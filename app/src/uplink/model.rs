//! the wire model for Live sharing: what a [`ChatEvent`] becomes on its way
//! to the ingest Worker, and the batching/backoff machinery the uplink worker
//! runs on. Projection is also where privacy is enforced — only selected
//! channels project, social data needs its own opt-in, and several variants
//! (blocked accounts among them) never leave the machine at all.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    chat::{ChatChannel, ChatEvent, ChatUser, channel_title, strip_character_code},
    native::presence::PresenceState,
};

/// Events per POST; matches the ingest Worker's cap.
pub const MAX_BATCH_EVENTS: usize = 64;
/// A partial batch flushes quickly enough for presence to feel live.
pub const FLUSH_AFTER: Duration = Duration::from_millis(500);
/// Worker-side buffer bound; beyond it the oldest event drops.
pub const MAX_PENDING_EVENTS: usize = 4096;
/// Message bodies truncate to the server's validation cap.
pub const MAX_BODY_CHARS: usize = 4000;

#[derive(Clone, Debug, Serialize)]
pub struct SessionMeta {
    pub id: String,
    pub client_version: &'static str,
    pub started_at: u64,
}

#[derive(Serialize)]
pub struct Envelope<'a> {
    pub v: u32,
    pub session: &'a SessionMeta,
    pub events: &'a [EventDto],
}

#[derive(Clone, Debug, Serialize)]
pub struct EventDto {
    pub seq: u64,
    pub ts: u64,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChannelRef {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A portrait's cell in the app's own atlas sheets: `atlas-{t:02}.png`,
/// cell `o` (portraits.rs: 6 cells per row, 152pt each). Meaningless without
/// the atlases, which the viewer serves from the same shipped files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PortraitRef {
    pub t: u16,
    pub o: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UserRef {
    pub handle: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clan_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portrait: Option<PortraitRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_local: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joined_order: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    SessionStarted,
    SyncStarted,
    SessionSynced,
    Joined {
        channel: ChannelRef,
    },
    /// The local user left the channel; the viewer stops listing it.
    Left {
        channel: ChannelRef,
    },
    Roster {
        channel: ChannelRef,
        complete: bool,
        count: u32,
        users: Vec<UserRef>,
    },
    MemberJoined {
        channel: ChannelRef,
        user: UserRef,
    },
    MemberLeft {
        channel: ChannelRef,
        user: UserRef,
    },
    /// Presence, name, clan, or portrait changes for members already listed —
    /// the session enriches its roster continuously and these keep the
    /// viewer's in step. Never a transcript line.
    RosterDelta {
        channel: ChannelRef,
        users: Vec<UserRef>,
    },
    Message {
        channel: ChannelRef,
        sender: UserRef,
        body: String,
    },
    Dropped {
        count: u64,
    },
    /// Sent on the connection keepalive tick so the feed reads live while the
    /// app is connected, even through long silences. Carries nothing and
    /// writes nothing — it only refreshes the session's last-seen time.
    Heartbeat,
    SessionEnded,
}

/// The identity string shared with `save_open_channels` persistence and the
/// backend's channel keys. UI and uplink must never disagree on this format.
#[must_use]
pub fn channel_identity(channel: &ChatChannel) -> String {
    match channel {
        ChatChannel::Public(identifier) => format!("public:{identifier}"),
        ChatChannel::Private(name) => format!("private:{name}"),
        ChatChannel::Club(club_id) => format!("club:{club_id}"),
        ChatChannel::Party => "party".into(),
    }
}

fn channel_ref(
    channel: &ChatChannel,
    club_names: &BTreeMap<u32, String>,
    public_names: &BTreeMap<u16, String>,
) -> ChannelRef {
    ChannelRef {
        key: channel_identity(channel),
        name: match channel {
            // The learned group name when the session has one; otherwise no
            // claim — the viewer falls back to "Group {id}" like a fresh tab.
            ChatChannel::Club(club_id) => club_names.get(club_id).cloned(),
            // use the catalog learned from battle.net before the offline fallback.
            ChatChannel::Public(identifier) => public_names
                .get(identifier)
                .cloned()
                .or_else(|| Some(channel_title(channel))),
            ChatChannel::Private(_) | ChatChannel::Party => Some(channel_title(channel)),
        },
    }
}

#[must_use]
pub fn presence_str(state: PresenceState) -> &'static str {
    match state {
        PresenceState::Available => "available",
        PresenceState::Away => "away",
        PresenceState::Busy => "busy",
        PresenceState::InGame => "in_game",
        PresenceState::Offline => "offline",
        PresenceState::Unknown => "unknown",
    }
}

fn user_ref(
    user: &ChatUser,
    with_presence: bool,
    is_local: Option<bool>,
    joined_order: Option<u64>,
) -> UserRef {
    UserRef {
        handle: user.handle,
        // The app never shows the #code (visible_name strips it); neither
        // does the wire.
        name: user
            .name
            .as_deref()
            .map(|name| strip_character_code(name).to_owned()),
        clan_tag: user.clan_tag.clone(),
        presence: with_presence.then(|| presence_str(user.presence)),
        portrait: user.avatar.map(|entry| PortraitRef {
            t: entry.table_id,
            o: entry.offset,
        }),
        is_local,
        joined_order,
    }
}

fn truncated(body: &str) -> String {
    if body.chars().count() <= MAX_BODY_CHARS {
        body.to_owned()
    } else {
        body.chars().take(MAX_BODY_CHARS).collect()
    }
}

/// What the user's settings permit the projection to emit. Mirrors
/// [`super::config::UplinkConfig`] as a per-event snapshot.
#[derive(Clone, Copy)]
pub struct ProjectionGates<'a> {
    /// The master switch; off projects nothing.
    pub enabled: bool,
    /// Optional channel allow-list; `None` shares every channel (the current
    /// global-switch behaviour — per-channel selection is plumbed but has no
    /// UI yet).
    pub shared_channels: Option<&'a BTreeSet<String>>,
}

/// Turns [`ChatEvent`]s into wire events, holding the per-session state that
/// requires seeing events in order: the channel-index map (fed by `Joined`,
/// overwritten on index reuse), the per-channel roster mirror that turns the
/// session's repeated snapshots into deltas, the learned group names, and the
/// current roster mirror.
#[derive(Default)]
pub struct Projector {
    channels: BTreeMap<u8, ChatChannel>,
    local_handles: BTreeMap<u8, u32>,
    roster_sent: BTreeSet<u8>,
    /// What the viewer knows of each channel's members, keyed by handle —
    /// diffing successive snapshots against this is what carries presence,
    /// name, and portrait resolution to the wire.
    roster_state: BTreeMap<u8, BTreeMap<u32, UserRef>>,
    roster_order: BTreeMap<u8, BTreeMap<u32, u64>>,
    next_roster_order: BTreeMap<u8, u64>,
    /// Group names learned from `GroupSummary`, as the app's tabs learn them.
    club_names: BTreeMap<u32, String>,
    public_names: BTreeMap<u16, String>,
}

impl Projector {
    #[must_use]
    pub fn with_club_names(club_names: BTreeMap<u32, String>) -> Self {
        Self {
            club_names,
            ..Self::default()
        }
    }

    /// Projects one event, or `None` when it must not be sent. Structural
    /// bookkeeping (the index map) always runs, even while disabled, so a
    /// mid-session toggle starts from a correct map.
    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per ChatEvent variant, as handle_chat does"
    )]
    pub fn project(&mut self, event: &ChatEvent, gates: ProjectionGates<'_>) -> Option<EventKind> {
        let ProjectionGates {
            enabled,
            shared_channels: shared,
        } = gates;
        if let ChatEvent::Joined {
            channel_index,
            channel,
            local_member_handle,
            ..
        } = event
        {
            self.channels.insert(*channel_index, channel.clone());
            self.local_handles
                .insert(*channel_index, *local_member_handle);
            self.roster_sent.remove(channel_index);
            self.roster_state.remove(channel_index);
            self.roster_order.remove(channel_index);
            self.next_roster_order.remove(channel_index);
        }
        // group names are structural too: learned even while disabled, so a
        // later enable names the tabs correctly from the start.
        if let ChatEvent::GroupSummary {
            club_id,
            name: Some(name),
            ..
        } = event
        {
            self.club_names.insert(*club_id, name.clone());
        }
        if let ChatEvent::PublicChannelCatalog(channels) = event {
            self.public_names = channels
                .iter()
                .map(|channel| (channel.identifier, channel.name.clone()))
                .collect();
        }
        if !enabled {
            return None;
        }
        match event {
            ChatEvent::Joined { channel, .. } => self
                .shared_channel_ref(channel, shared)
                .map(|channel| EventKind::Joined { channel }),
            ChatEvent::Roster(snapshot) => {
                if !snapshot.initial_complete {
                    return None;
                }
                let channel = self.shared_index(snapshot.channel_index, shared)?;
                let mirror = snapshot
                    .users
                    .iter()
                    .map(|user| {
                        (
                            user.handle,
                            self.roster_user_ref(snapshot.channel_index, user),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                if self.roster_sent.contains(&snapshot.channel_index) {
                    // The session re-emits a full snapshot on every membership
                    // or presence record; the viewer only needs what changed.
                    let known = self.roster_state.entry(snapshot.channel_index).or_default();
                    let users: Vec<UserRef> = mirror
                        .values()
                        .filter(|user| known.get(&user.handle) != Some(user))
                        .cloned()
                        .collect();
                    *known = mirror;
                    if users.is_empty() {
                        return None;
                    }
                    Some(EventKind::RosterDelta { channel, users })
                } else {
                    self.roster_sent.insert(snapshot.channel_index);
                    let count = u32::try_from(snapshot.users.len()).unwrap_or(u32::MAX);
                    let users = mirror.values().cloned().collect();
                    self.roster_state.insert(snapshot.channel_index, mirror);
                    Some(EventKind::Roster {
                        channel,
                        complete: true,
                        count,
                        users,
                    })
                }
            }
            ChatEvent::MemberJoined {
                channel_index,
                user,
            } => {
                let projected = self.roster_user_ref(*channel_index, user);
                self.roster_state
                    .entry(*channel_index)
                    .or_default()
                    .insert(user.handle, projected.clone());
                self.shared_index(*channel_index, shared)
                    .map(|channel| EventKind::MemberJoined {
                        channel,
                        user: projected,
                    })
            }
            ChatEvent::MemberLeft {
                channel_index,
                user,
                ..
            } => {
                let projected = self
                    .roster_state
                    .get(channel_index)
                    .and_then(|known| known.get(&user.handle))
                    .cloned()
                    .unwrap_or_else(|| self.roster_user_ref(*channel_index, user));
                self.roster_state
                    .entry(*channel_index)
                    .or_default()
                    .remove(&user.handle);
                self.roster_order
                    .entry(*channel_index)
                    .or_default()
                    .remove(&user.handle);
                self.shared_index(*channel_index, shared)
                    .map(|channel| EventKind::MemberLeft {
                        channel,
                        user: projected,
                    })
            }
            // Being removed from a channel means it is no longer ours to
            // share: forget it and tell the viewer it closed, exactly as a
            // deliberate leave does.
            ChatEvent::Removed { channel_index, .. } => self
                .leave(*channel_index, enabled, shared)
                .map(|channel| EventKind::Left { channel }),
            // A group summary that names a shared open channel refreshes the
            // viewer's tab title (the server keeps the latest non-empty name).
            ChatEvent::GroupSummary {
                club_id,
                name: Some(_),
                ..
            } => {
                let named = self
                    .channels
                    .values()
                    .find(|channel| matches!(channel, ChatChannel::Club(id) if id == club_id))?
                    .clone();
                self.shared_channel_ref(&named, shared)
                    .map(|channel| EventKind::Joined { channel })
            }
            ChatEvent::Message {
                channel_index,
                sender,
                body,
            } => self
                .shared_index(*channel_index, shared)
                .map(|channel| EventKind::Message {
                    channel,
                    sender: user_ref(sender, false, None, None),
                    body: truncated(body),
                }),
            // everything else stays on the machine: catalog and ux noise
            // (ConferenceDirectory, Activity, JoinRejected, WhisperFailed,
            // Group*) and BlockedAccounts, which no opt-in covers.
            ChatEvent::PublicChannelCatalog(_)
            | ChatEvent::ConferenceDirectory { .. }
            | ChatEvent::JoinRejected { .. }
            | ChatEvent::BlockedAccounts(_)
            | ChatEvent::Activity { .. }
            | ChatEvent::Whisper { .. }
            | ChatEvent::Friends(_)
            | ChatEvent::WhisperFailed { .. }
            | ChatEvent::GroupInvitation { .. }
            | ChatEvent::PartyInvitation { .. }
            | ChatEvent::GroupSummary { .. }
            | ChatEvent::GroupSearch { .. } => None,
        }
    }

    /// The local user left a channel (a command, not a chat event — the
    /// caller hooks it in the command loop). Always forgets the index, which
    /// the service recycles; returns the channel to announce when sharing.
    pub fn leave(
        &mut self,
        index: u8,
        enabled: bool,
        shared: Option<&BTreeSet<String>>,
    ) -> Option<ChannelRef> {
        let announced = if enabled {
            self.shared_index(index, shared)
        } else {
            None
        };
        self.channels.remove(&index);
        self.local_handles.remove(&index);
        self.roster_sent.remove(&index);
        self.roster_state.remove(&index);
        self.roster_order.remove(&index);
        self.next_roster_order.remove(&index);
        announced
    }

    fn roster_user_ref(&mut self, channel_index: u8, user: &ChatUser) -> UserRef {
        let joined_order = self
            .roster_order
            .get(&channel_index)
            .and_then(|orders| orders.get(&user.handle))
            .copied()
            .unwrap_or_else(|| {
                let next = self.next_roster_order.entry(channel_index).or_default();
                let joined_order = *next;
                *next = next.saturating_add(1);
                self.roster_order
                    .entry(channel_index)
                    .or_default()
                    .insert(user.handle, joined_order);
                joined_order
            });
        user_ref(
            user,
            true,
            Some(self.local_handles.get(&channel_index) == Some(&user.handle)),
            Some(joined_order),
        )
    }

    fn shared_index(&self, index: u8, shared: Option<&BTreeSet<String>>) -> Option<ChannelRef> {
        let channel = self.channels.get(&index)?;
        self.shared_channel_ref(channel, shared)
    }

    fn shared_channel_ref(
        &self,
        channel: &ChatChannel,
        shared: Option<&BTreeSet<String>>,
    ) -> Option<ChannelRef> {
        match shared {
            None => Some(channel_ref(channel, &self.club_names, &self.public_names)),
            Some(selection) => selection
                .contains(&channel_identity(channel))
                .then(|| channel_ref(channel, &self.club_names, &self.public_names)),
        }
    }

    #[must_use]
    pub fn reconciliation_events(&self, shared: Option<&BTreeSet<String>>) -> Vec<EventKind> {
        self.channels
            .iter()
            .filter(|(index, _)| self.roster_sent.contains(index))
            .filter_map(|(index, channel)| {
                let channel = self.shared_channel_ref(channel, shared)?;
                let users = self
                    .roster_state
                    .get(index)?
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                Some(EventKind::Roster {
                    channel,
                    complete: true,
                    count: u32::try_from(users.len()).unwrap_or(u32::MAX),
                    users,
                })
            })
            .collect()
    }
}

/// The uplink worker's outbound buffer: flush at [`MAX_BATCH_EVENTS`] or
/// [`FLUSH_AFTER`], drop the oldest past [`MAX_PENDING_EVENTS`].
#[derive(Default)]
pub struct Batcher {
    pending: VecDeque<EventDto>,
    first_buffered: Option<Instant>,
}

impl Batcher {
    /// Buffers an event; returns how many old events were dropped to make room.
    pub fn push(&mut self, event: EventDto, now: Instant) -> u64 {
        let mut dropped = 0;
        while self.pending.len() >= MAX_PENDING_EVENTS {
            self.pending.pop_front();
            dropped += 1;
        }
        if self.pending.is_empty() {
            self.first_buffered = Some(now);
        }
        self.pending.push_back(event);
        dropped
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// When the next time-based flush is due, if anything is buffered.
    #[must_use]
    pub fn flush_deadline(&self) -> Option<Instant> {
        self.first_buffered.map(|start| start + FLUSH_AFTER)
    }

    /// Takes a batch if one is due, either by size or by age.
    pub fn take_batch(&mut self, now: Instant) -> Option<Vec<EventDto>> {
        let due_by_size = self.pending.len() >= MAX_BATCH_EVENTS;
        let due_by_age = self
            .flush_deadline()
            .is_some_and(|deadline| now >= deadline);
        let completes_sync = self
            .pending
            .iter()
            .any(|event| matches!(event.kind, EventKind::SessionSynced));
        if !due_by_size && !due_by_age && !completes_sync {
            return None;
        }
        let count = self.ready_count()?;
        Some(self.take_count(count))
    }

    /// Takes whatever is buffered immediately (shutdown flush).
    pub fn take_now(&mut self) -> Vec<EventDto> {
        let count = self.ready_count().unwrap_or(0);
        self.take_count(count)
    }

    pub fn discard_batch(&mut self) -> Vec<EventDto> {
        let count = self.pending.len().min(MAX_BATCH_EVENTS);
        self.take_count(count)
    }

    fn ready_count(&self) -> Option<usize> {
        let limit = self.pending.len().min(MAX_BATCH_EVENTS);
        let sync_start = self
            .pending
            .iter()
            .take(limit)
            .position(|event| matches!(event.kind, EventKind::SyncStarted));
        let Some(sync_start) = sync_start else {
            return (limit > 0).then_some(limit);
        };
        let sync_end = self
            .pending
            .iter()
            .skip(sync_start)
            .position(|event| matches!(event.kind, EventKind::SessionSynced))
            .map(|offset| sync_start + offset);
        match sync_end {
            Some(end) if end < limit => Some(limit),
            Some(_) | None if sync_start > 0 => Some(sync_start),
            Some(_) | None => None,
        }
    }

    fn take_count(&mut self, count: usize) -> Vec<EventDto> {
        let batch: Vec<EventDto> = self.pending.drain(..count).collect();
        self.first_buffered = if self.pending.is_empty() {
            None
        } else {
            Some(Instant::now())
        };
        batch
    }

    /// Puts a failed batch back at the front, oldest first.
    pub fn restore(&mut self, batch: Vec<EventDto>) {
        for event in batch.into_iter().rev() {
            self.pending.push_front(event);
        }
        if self.first_buffered.is_none() && !self.pending.is_empty() {
            self.first_buffered = Some(Instant::now());
        }
    }
}

/// Exponential backoff for retryable POST failures: 1s base, doubling to a
/// 60s cap. Jitter is sampled by the caller so this stays deterministic.
#[derive(Default)]
pub struct Backoff {
    failures: u32,
}

impl Backoff {
    pub fn note_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
    }

    pub fn reset(&mut self) {
        self.failures = 0;
    }

    #[must_use]
    pub fn is_backing_off(&self) -> bool {
        self.failures > 0
    }

    /// The full (un-jittered) delay for the current failure count.
    #[must_use]
    pub fn delay(&self) -> Duration {
        if self.failures == 0 {
            return Duration::ZERO;
        }
        let exponent = self.failures.saturating_sub(1).min(6);
        Duration::from_secs(1 << exponent).min(Duration::from_secs(60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::RosterSnapshot;

    fn user(handle: u32, name: &str) -> ChatUser {
        ChatUser {
            handle,
            presence_id: None,
            name: Some(name.to_owned()),
            clan_tag: None,
            avatar: None,
            presence: PresenceState::Available,
        }
    }

    fn joined(index: u8, channel: ChatChannel) -> ChatEvent {
        ChatEvent::Joined {
            channel_index: index,
            channel,
            local_member_handle: 1,
            shard_index: None,
        }
    }

    fn shared(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|key| (*key).to_owned()).collect()
    }

    fn gates(enabled: bool, shared: Option<&BTreeSet<String>>) -> ProjectionGates<'_> {
        ProjectionGates {
            enabled,
            shared_channels: shared,
        }
    }

    #[test]
    fn identity_matches_the_saved_channels_format() {
        assert_eq!(channel_identity(&ChatChannel::Public(1033)), "public:1033");
        assert_eq!(
            channel_identity(&ChatChannel::Private("Op Test".into())),
            "private:Op Test"
        );
        assert_eq!(channel_identity(&ChatChannel::Club(5322)), "club:5322");
        assert_eq!(channel_identity(&ChatChannel::Party), "party");
    }

    #[test]
    fn cached_group_names_resolve_the_first_join() {
        let mut projector =
            Projector::with_club_names(BTreeMap::from([(98908, "general Arcade".to_owned())]));

        match projector.project(&joined(0, ChatChannel::Club(98908)), gates(true, None)) {
            Some(EventKind::Joined { channel }) => {
                assert_eq!(channel.key, "club:98908");
                assert_eq!(channel.name.as_deref(), Some("general Arcade"));
            }
            other => panic!("expected a named club join, got {other:?}"),
        }
    }

    #[test]
    fn party_channels_keep_the_party_identity_and_title() {
        let mut projector = Projector::default();

        match projector.project(&joined(u8::MAX, ChatChannel::Party), gates(true, None)) {
            Some(EventKind::Joined { channel }) => {
                assert_eq!(channel.key, "party");
                assert_eq!(channel.name.as_deref(), Some("Party"));
            }
            other => panic!("expected a party join, got {other:?}"),
        }
    }

    #[test]
    fn only_shared_channels_project() {
        let mut projector = Projector::default();
        let share = shared(&["public:1033"]);
        assert!(
            projector
                .project(
                    &joined(0, ChatChannel::Public(1033)),
                    gates(true, Some(&share))
                )
                .is_some()
        );
        assert!(
            projector
                .project(
                    &joined(1, ChatChannel::Public(52)),
                    gates(true, Some(&share))
                )
                .is_none()
        );

        let on_shared = ChatEvent::Message {
            channel_index: 0,
            sender: user(7, "Overmind"),
            body: "hi".into(),
        };
        let on_unshared = ChatEvent::Message {
            channel_index: 1,
            sender: user(7, "Overmind"),
            body: "hi".into(),
        };
        assert!(
            projector
                .project(&on_shared, gates(true, Some(&share)))
                .is_some()
        );
        assert!(
            projector
                .project(&on_unshared, gates(true, Some(&share)))
                .is_none()
        );
    }

    #[test]
    fn repeated_snapshots_become_deltas_carrying_presence_changes() {
        let mut projector = Projector::default();
        let all = gates(true, None);
        projector.project(&joined(0, ChatChannel::Public(1033)), all);

        let snapshot = |presence: PresenceState| {
            ChatEvent::Roster(RosterSnapshot {
                channel_index: 0,
                initial_complete: true,
                users: vec![
                    ChatUser {
                        handle: 7,
                        presence_id: None,
                        name: Some("Overmind".into()),
                        clan_tag: None,
                        avatar: None,
                        presence,
                    },
                    user(8, "Kerrigan"),
                ],
            })
        };

        // First complete snapshot: the full roster.
        assert!(matches!(
            projector.project(&snapshot(PresenceState::Unknown), all),
            Some(EventKind::Roster { .. })
        ));
        // Same snapshot again: nothing changed, nothing sent.
        assert!(
            projector
                .project(&snapshot(PresenceState::Unknown), all)
                .is_none()
        );
        // Overmind's presence resolves: exactly one delta, exactly one user.
        match projector.project(&snapshot(PresenceState::Available), all) {
            Some(EventKind::RosterDelta { users, .. }) => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].handle, 7);
                assert_eq!(users[0].presence, Some("available"));
            }
            other => panic!("expected a roster delta, got {other:?}"),
        }
        // And the delta is remembered — replaying it is silent.
        assert!(
            projector
                .project(&snapshot(PresenceState::Available), all)
                .is_none()
        );
    }

    #[test]
    fn being_removed_closes_the_channel_and_a_trailing_roster_cannot_reopen_it() {
        let mut projector = Projector::default();
        let all = gates(true, None);
        projector.project(&joined(0, ChatChannel::Public(1033)), all);

        // The ban: a Removed event closes the channel for the viewer.
        let removed = ChatEvent::Removed {
            channel_index: 0,
            reason: Some(315),
        };
        match projector.project(&removed, all) {
            Some(EventKind::Left { channel }) => assert_eq!(channel.key, "public:1033"),
            other => panic!("expected the channel to close, got {other:?}"),
        }

        // The session emits a roster snapshot right after the removal; it must
        // not resurrect the channel the viewer just dropped.
        let snapshot = ChatEvent::Roster(RosterSnapshot {
            channel_index: 0,
            initial_complete: true,
            users: vec![user(7, "Overmind")],
        });
        assert!(
            projector.project(&snapshot, all).is_none(),
            "a roster for a channel we were removed from must not reopen it"
        );
    }

    #[test]
    fn leaving_a_channel_announces_and_forgets_the_index() {
        let mut projector = Projector::default();
        let all = gates(true, None);
        projector.project(&joined(0, ChatChannel::Public(1033)), all);

        let channel = projector
            .leave(0, true, None)
            .expect("shared channel announces");
        assert_eq!(channel.key, "public:1033");
        // The index is forgotten: traffic on it no longer resolves.
        let message = ChatEvent::Message {
            channel_index: 0,
            sender: user(7, "Overmind"),
            body: "ghost".into(),
        };
        assert!(projector.project(&message, all).is_none());
        // And a second leave has nothing to announce.
        assert!(projector.leave(0, true, None).is_none());
    }

    #[test]
    fn group_summaries_name_club_channels() {
        let mut projector = Projector::default();
        let all = gates(true, None);
        projector.project(&joined(0, ChatChannel::Club(5322)), all);

        let summary = ChatEvent::GroupSummary {
            club_id: 5322,
            name: Some("Night Owls".into()),
            kind: 1,
            category: 1,
            private: false,
            member: true,
        };
        match projector.project(&summary, all) {
            Some(EventKind::Joined { channel }) => {
                assert_eq!(channel.key, "club:5322");
                assert_eq!(channel.name.as_deref(), Some("Night Owls"));
            }
            other => panic!("expected a named joined refresh, got {other:?}"),
        }
    }

    #[test]
    fn names_lose_their_character_codes() {
        let mut projector = Projector::default();
        let all = gates(true, None);
        projector.project(
            &ChatEvent::PublicChannelCatalog(vec![crate::chat::PublicChannel {
                identifier: 1028,
                name: "General".into(),
            }]),
            all,
        );
        projector.project(&joined(0, ChatChannel::Public(1028)), all);
        let message = ChatEvent::Message {
            channel_index: 0,
            sender: user(848, "Chalcuchimac#848"),
            body: "hi".into(),
        };
        match projector.project(&message, all) {
            Some(EventKind::Message {
                sender, channel, ..
            }) => {
                assert_eq!(sender.name.as_deref(), Some("Chalcuchimac"));
                assert_eq!(channel.name.as_deref(), Some("General"));
            }
            other => panic!("expected a message, got {other:?}"),
        }
    }

    #[test]
    fn no_selection_shares_every_channel() {
        let mut projector = Projector::default();
        projector.project(&joined(0, ChatChannel::Public(1033)), gates(true, None));
        projector.project(&joined(1, ChatChannel::Club(5322)), gates(true, None));
        for index in [0, 1] {
            let message = ChatEvent::Message {
                channel_index: index,
                sender: user(7, "Overmind"),
                body: "hi".into(),
            };
            assert!(projector.project(&message, gates(true, None)).is_some());
        }
    }

    #[test]
    fn disabled_still_learns_the_index_map() {
        let mut projector = Projector::default();
        let share = shared(&["public:1033"]);
        assert!(
            projector
                .project(
                    &joined(0, ChatChannel::Public(1033)),
                    gates(false, Some(&share))
                )
                .is_none()
        );
        // Enable mid-session: the index resolves because Joined was observed.
        let message = ChatEvent::Message {
            channel_index: 0,
            sender: user(7, "Overmind"),
            body: "hi".into(),
        };
        assert!(
            projector
                .project(&message, gates(true, Some(&share)))
                .is_some()
        );
    }

    #[test]
    fn roster_projects_once_per_join_epoch() {
        let mut projector = Projector::default();
        let share = shared(&["public:1033"]);
        let channel = ChatChannel::Public(1033);
        projector.project(&joined(0, channel.clone()), gates(true, Some(&share)));

        let snapshot = |complete: bool| {
            ChatEvent::Roster(RosterSnapshot {
                channel_index: 0,
                initial_complete: complete,
                users: vec![user(7, "Overmind")],
            })
        };
        assert!(
            projector
                .project(&snapshot(false), gates(true, Some(&share)))
                .is_none()
        );
        assert!(
            projector
                .project(&snapshot(true), gates(true, Some(&share)))
                .is_some()
        );
        assert!(
            projector
                .project(&snapshot(true), gates(true, Some(&share)))
                .is_none()
        );
        // Rejoining the channel resets the throttle.
        projector.project(&joined(0, channel), gates(true, Some(&share)));
        assert!(
            projector
                .project(&snapshot(true), gates(true, Some(&share)))
                .is_some()
        );
    }

    #[test]
    fn noise_variants_never_project() {
        let mut projector = Projector::default();
        let share = shared(&["public:1033"]);
        let events = [
            ChatEvent::PublicChannelCatalog(vec![]),
            ChatEvent::ConferenceDirectory {
                identifiers: vec![1],
                complete: true,
            },
            ChatEvent::JoinRejected {
                channel: None,
                reason: Some(1),
            },
            ChatEvent::BlockedAccounts(vec![]),
            ChatEvent::Activity { route: (None, 0) },
            ChatEvent::WhisperFailed {
                peer: "x".into(),
                reason: "y".into(),
            },
            ChatEvent::GroupInvitation { club_id: 1 },
            ChatEvent::GroupSearch { club_ids: vec![] },
        ];
        for event in &events {
            assert!(
                projector
                    .project(event, gates(true, Some(&share)))
                    .is_none()
            );
        }
    }

    #[test]
    fn envelope_serializes_the_documented_json() {
        let session = SessionMeta {
            id: "00112233445566778899aabbccddeeff".into(),
            client_version: "0.1.19",
            started_at: 1_754_700_251_000,
        };
        let events = vec![EventDto {
            seq: 41,
            ts: 1_754_700_302_113,
            kind: EventKind::Message {
                channel: ChannelRef {
                    key: "public:1033".into(),
                    name: None,
                },
                sender: UserRef {
                    handle: 12_345,
                    name: Some("Radiant".into()),
                    clan_tag: Some("SLIM".into()),
                    presence: None,
                    portrait: None,
                    is_local: None,
                    joined_order: None,
                },
                body: "gl hf".into(),
            },
        }];
        let envelope = Envelope {
            v: 1,
            session: &session,
            events: &events,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert_eq!(
            json,
            "{\"v\":1,\
             \"session\":{\"id\":\"00112233445566778899aabbccddeeff\",\
             \"client_version\":\"0.1.19\",\"started_at\":1754700251000},\
             \"events\":[{\"seq\":41,\"ts\":1754700302113,\"kind\":\"message\",\
             \"channel\":{\"key\":\"public:1033\"},\
             \"sender\":{\"handle\":12345,\"name\":\"Radiant\",\"clan_tag\":\"SLIM\"},\
             \"body\":\"gl hf\"}]}"
        );
    }

    #[test]
    fn batcher_flushes_by_size_and_age_and_drops_oldest() {
        let start = Instant::now();
        let mut batcher = Batcher::default();
        let dto = |seq: u64| EventDto {
            seq,
            ts: 1,
            kind: EventKind::SessionStarted,
        };

        for seq in 0..MAX_BATCH_EVENTS as u64 - 1 {
            assert_eq!(batcher.push(dto(seq), start), 0);
        }
        assert!(batcher.take_batch(start).is_none(), "not due yet");
        batcher.push(dto(63), start);
        let batch = batcher.take_batch(start).expect("due by size");
        assert_eq!(batch.len(), MAX_BATCH_EVENTS);

        batcher.push(dto(64), start);
        assert!(batcher.take_batch(start).is_none());
        let batch = batcher.take_batch(start + FLUSH_AFTER).expect("due by age");
        assert_eq!(batch.len(), 1);

        let mut dropped = 0;
        for seq in 0..(MAX_PENDING_EVENTS as u64 + 10) {
            dropped += batcher.push(dto(seq), start);
        }
        assert_eq!(dropped, 10);
    }

    #[test]
    fn batcher_never_splits_an_authoritative_sync() {
        let start = Instant::now();
        let mut batcher = Batcher::default();
        let event = |seq, kind| EventDto { seq, ts: seq, kind };
        batcher.push(event(1, EventKind::Heartbeat), start);
        batcher.push(event(2, EventKind::SyncStarted), start);
        batcher.push(
            event(
                3,
                EventKind::Roster {
                    channel: ChannelRef {
                        key: "public:1033".into(),
                        name: None,
                    },
                    complete: true,
                    count: 0,
                    users: Vec::new(),
                },
            ),
            start,
        );
        assert_eq!(
            batcher.take_batch(start + FLUSH_AFTER).unwrap().len(),
            1,
            "events before a pending sync may flush"
        );
        assert!(batcher.take_batch(start + FLUSH_AFTER).is_none());
        batcher.push(event(4, EventKind::SessionSynced), start);
        let sync = batcher.take_batch(start + FLUSH_AFTER).unwrap();
        assert!(matches!(
            sync.first().map(|event| &event.kind),
            Some(EventKind::SyncStarted)
        ));
        assert!(matches!(
            sync.last().map(|event| &event.kind),
            Some(EventKind::SessionSynced)
        ));
    }

    #[test]
    fn restore_preserves_order() {
        let start = Instant::now();
        let mut batcher = Batcher::default();
        let dto = |seq: u64| EventDto {
            seq,
            ts: 1,
            kind: EventKind::SessionStarted,
        };
        batcher.push(dto(1), start);
        batcher.push(dto(2), start);
        let batch = batcher.take_now();
        batcher.push(dto(3), start);
        batcher.restore(batch);
        let all = batcher.take_now();
        let sequence: Vec<u64> = all.iter().map(|event| event.seq).collect();
        assert_eq!(sequence, vec![1, 2, 3]);
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let mut backoff = Backoff::default();
        assert_eq!(backoff.delay(), Duration::ZERO);
        backoff.note_failure();
        assert_eq!(backoff.delay(), Duration::from_secs(1));
        backoff.note_failure();
        assert_eq!(backoff.delay(), Duration::from_secs(2));
        for _ in 0..10 {
            backoff.note_failure();
        }
        assert_eq!(backoff.delay(), Duration::from_secs(60));
        backoff.reset();
        assert_eq!(backoff.delay(), Duration::ZERO);
    }
}
