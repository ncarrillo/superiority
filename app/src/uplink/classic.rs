//! the tap for a classic-channel product — Remastered or Reforged — and the
//! roster-diff shared by their projectors.
//!
//! A classic session speaks its own chat vocabulary (see [`super::scr`] and
//! [`super::wc3`]), but the send machinery, the announce/heartbeat/end
//! lifecycle, and the way a re-sent whole roster becomes wire events are the
//! same as `StarCraft II`'s. Those are shared here rather than written a second
//! time, so a change to how the uplink batches or sequences reaches every
//! product.

use std::collections::BTreeMap;
use std::sync::mpsc::SyncSender;

use superiority_core::games::scr::chat::{ChatChannel as ScrChannel, ChatEvent as ScrChatEvent};
use superiority_core::games::wc3::{
    ChatChannel as WarcraftChannel, ChatEvent as WarcraftChatEvent,
};
use superiority_core::observer::SessionObserver;
use superiority_core::product::Product;

use super::model::{ChannelRef, EventKind, SessionMeta, UserRef};
use super::scr::ScrProjector;
use super::wc3::Wc3Projector;
use super::{CLIENT_VERSION, TapMessage, UplinkControl, announce_session, emit_event, now_ms};

/// FNV-1a (32-bit) over some bytes. The classic edges do not hand out a numeric
/// member handle the wire wants, so a stable one is derived: Remastered hashes
/// the lowercase name, Reforged its account handle's bytes. Stable across a
/// reconnect, which a per-session counter would not be.
#[must_use]
pub(super) fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// truncates a body to the server's cap by characters, not bytes.
#[must_use]
pub(super) fn truncate(body: String) -> String {
    if body.chars().count() <= super::model::MAX_BODY_CHARS {
        body
    } else {
        body.chars().take(super::model::MAX_BODY_CHARS).collect()
    }
}

/// diffs a whole-channel snapshot against what the viewer was last told: a
/// complete [`EventKind::Roster`] the first time and whenever the member set
/// changes (so a departure is applied — a delta only upserts), a
/// [`EventKind::RosterDelta`] when only existing members' attributes moved, and
/// nothing when nothing changed.
#[must_use]
pub(super) fn roster_events(
    channel: ChannelRef,
    members: BTreeMap<u32, UserRef>,
    state: &mut BTreeMap<u32, UserRef>,
    sent: &mut bool,
) -> Vec<EventKind> {
    let membership_changed =
        members.len() != state.len() || members.keys().any(|handle| !state.contains_key(handle));
    if !*sent || membership_changed {
        *sent = true;
        let users = members.values().cloned().collect::<Vec<_>>();
        *state = members;
        return vec![EventKind::Roster {
            channel,
            complete: true,
            count: u32::try_from(users.len()).unwrap_or(u32::MAX),
            users,
        }];
    }
    let changed = members
        .values()
        .filter(|user| state.get(&user.handle) != Some(*user))
        .cloned()
        .collect::<Vec<_>>();
    *state = members;
    if changed.is_empty() {
        Vec::new()
    } else {
        vec![EventKind::RosterDelta {
            channel,
            users: changed,
        }]
    }
}

enum ClassicProjector {
    Scr(ScrProjector),
    Wc3(Wc3Projector),
}

impl ClassicProjector {
    fn resend(&self) -> Vec<EventKind> {
        match self {
            Self::Scr(projector) => projector.resend(),
            Self::Wc3(projector) => projector.resend(),
        }
    }

    fn reset_roster(&mut self) {
        match self {
            Self::Scr(projector) => projector.reset_roster(),
            Self::Wc3(projector) => projector.reset_roster(),
        }
    }
}

/// the Live tap on a classic product's session. Shares the SC2 tap's send
/// plumbing through the free functions in [`super`]; owns only the classic
/// projector and the announce state.
pub struct ClassicSessionTap {
    sender: SyncSender<TapMessage>,
    control: UplinkControl,
    meta: SessionMeta,
    projector: ClassicProjector,
    announced: bool,
    next_seq: u64,
    pending_dropped: u64,
    ended: bool,
}

impl ClassicSessionTap {
    #[must_use]
    pub(super) fn new(
        sender: SyncSender<TapMessage>,
        control: UplinkControl,
        product: Product,
        local_identity: Option<String>,
    ) -> Self {
        let projector = match product {
            Product::Remastered => ClassicProjector::Scr(ScrProjector::new(local_identity)),
            Product::Warcraft3 => ClassicProjector::Wc3(Wc3Projector::new()),
            // the SC2 path never takes a classic tap; it has its own.
            Product::StarCraft2 => ClassicProjector::Wc3(Wc3Projector::new()),
        };
        Self {
            sender,
            control,
            meta: SessionMeta {
                id: format!("{:032x}", rand::random::<u128>()),
                product: product.slug(),
                client_version: CLIENT_VERSION,
                started_at: now_ms(),
            },
            projector,
            announced: false,
            next_seq: 1,
            pending_dropped: 0,
            ended: false,
        }
    }

    fn enabled(&self) -> bool {
        matches!(self.control.config.read(), Ok(config) if config.enabled)
    }

    fn announce(&mut self) -> bool {
        announce_session(
            &self.sender,
            &self.control,
            &self.meta,
            &mut self.announced,
            &mut self.next_seq,
            &mut self.pending_dropped,
        )
    }

    fn emit(&mut self, kind: EventKind) {
        emit_event(
            &self.sender,
            &self.control,
            &self.meta,
            &mut self.next_seq,
            &mut self.pending_dropped,
            kind,
        );
    }

    fn announce_and_emit(&mut self, kind: EventKind) {
        if self.announce() {
            self.emit(kind);
        }
    }

    fn emit_all(&mut self, events: Vec<EventKind>) {
        for event in events {
            self.announce_and_emit(event);
        }
    }

    fn end(&mut self) {
        if self.announced && !self.ended {
            self.emit(EventKind::SessionEnded);
            self.ended = true;
        }
    }
}

impl SessionObserver for ClassicSessionTap {
    // the SC2 tap's surface: a classic session sees none of it.
    fn observe(&mut self, _event: &superiority_core::chat::ChatEvent) {}
    fn observe_left(&mut self, _channel_index: u8) {}
    fn reconcile(&mut self, _snapshots: &[superiority_core::chat::ChatEvent]) {}
    fn reject_channel(&mut self, _channel: &superiority_core::chat::ChatChannel) {}
    fn resolve_pending_channels(&mut self) {}

    fn heartbeat(&mut self) {
        if !self.enabled() {
            self.end();
            return;
        }
        if self.ended {
            // Live came back on: re-announce and resend the roster whole so the
            // viewer starts this session from a complete member list.
            self.ended = false;
            self.projector.reset_roster();
            self.emit(EventKind::SessionStarted);
            let events = self.projector.resend();
            self.emit_all(events);
        } else if self.announced {
            self.emit(EventKind::Heartbeat);
        }
    }

    fn end_session(&mut self) {
        self.end();
    }

    fn observe_classic(&mut self, event: &ScrChatEvent) {
        if !self.enabled() {
            return;
        }
        let kind = match &self.projector {
            ClassicProjector::Scr(projector) => projector.message_event(event),
            ClassicProjector::Wc3(_) => None,
        };
        if let Some(kind) = kind {
            self.announce_and_emit(kind);
        }
    }

    fn observe_classic_channel(&mut self, channel: &ScrChannel) {
        if !self.enabled() {
            return;
        }
        let events = match &mut self.projector {
            ClassicProjector::Scr(projector) => projector.channel_events(channel),
            ClassicProjector::Wc3(_) => Vec::new(),
        };
        self.emit_all(events);
    }

    fn observe_warcraft(&mut self, event: &WarcraftChatEvent) {
        if !self.enabled() {
            return;
        }
        let events = match &mut self.projector {
            ClassicProjector::Wc3(projector) => projector.event_kinds(event),
            ClassicProjector::Scr(_) => Vec::new(),
        };
        self.emit_all(events);
    }

    fn observe_warcraft_channel(&mut self, channel: &WarcraftChannel) {
        if !self.enabled() {
            return;
        }
        let events = match &mut self.projector {
            ClassicProjector::Wc3(projector) => projector.channel_events(channel),
            ClassicProjector::Scr(_) => Vec::new(),
        };
        self.emit_all(events);
    }
}

impl Drop for ClassicSessionTap {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use superiority_core::games::scr::chat::{ChatChannel, ChatUser};

    use super::*;

    fn member(name: &str) -> ChatUser {
        ChatUser {
            name: name.into(),
            flags: Some(0),
            is_operator: false,
            avatar: None,
            attributes: Vec::new(),
        }
    }

    fn channel(users: Vec<ChatUser>) -> ChatChannel {
        ChatChannel {
            channel_id: 9,
            name: "Public Chat 1".into(),
            display_name: Some("Public Chat 1".into()),
            is_public: true,
            users,
        }
    }

    #[test]
    fn a_classic_tap_is_silent_until_live_is_enabled_then_tags_its_own_session() {
        let (sender, receiver) = mpsc::sync_channel(64);
        let control = UplinkControl::new();
        let mut tap = ClassicSessionTap::new(
            sender,
            control.clone(),
            Product::Remastered,
            Some("Commander#1234".into()),
        );

        // disabled: nothing leaves the machine.
        tap.observe_classic_channel(&channel(vec![member("Darko")]));
        assert!(
            receiver.try_recv().is_err(),
            "a disabled tap must be silent"
        );

        // enabled: the session announces as `scr`, then the roster, and every
        // event is tagged with this session's id and no other.
        control.update_config(|config| config.enabled = true);
        tap.observe_classic_channel(&channel(vec![member("Darko"), member("Kerrigan")]));

        let TapMessage::Session(meta) = receiver.try_recv().expect("a session announcement") else {
            panic!("expected the session to announce first");
        };
        assert_eq!(meta.product, "scr");

        let mut saw_roster = false;
        while let Ok(message) = receiver.try_recv() {
            let TapMessage::Event { session, dto } = message else {
                panic!("only one session announcement");
            };
            assert_eq!(session, meta.id, "every event carries this session's id");
            if matches!(dto.kind, EventKind::Roster { .. }) {
                saw_roster = true;
            }
        }
        assert!(saw_roster, "the first snapshot carries a complete roster");
    }
}
