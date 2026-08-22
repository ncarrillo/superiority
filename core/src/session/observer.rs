//! The tap an embedder can put on a session.
//!
//! Every product's worker reports to the same observer, each in its own
//! vocabulary: `StarCraft II` through [`ChatEvent`], Remastered through the
//! classic channel's [`ClassicChatEvent`] and whole-channel snapshots, Reforged
//! through its own. The arms mirror [`crate::connection::ClientEvent`]'s, and
//! for the same reason — none of these is a dialect of another.

use crate::{
    chat::{ChatChannel, ChatEvent},
    games::{
        scr::chat::{ChatChannel as ClassicChatChannel, ChatEvent as ClassicChatEvent},
        wc3::{ChatChannel as WarcraftChatChannel, ChatEvent as WarcraftChatEvent},
    },
    product::Product,
};

pub trait SessionObserverFactory: Send {
    /// `StarCraft II`'s session, which opens on a list of channels.
    fn begin_session(&self, channels: &[ChatChannel]) -> Box<dyn SessionObserver>;

    /// A classic session — Remastered's or Reforged's — which is in one channel
    /// at a time. `local_identity` is what the roster knows the signed-in
    /// account by (the BattleTag), so the observer can tell which member is
    /// the user; the classic edges do not mark it.
    fn begin_classic_session(
        &self,
        product: Product,
        local_identity: Option<String>,
    ) -> Box<dyn SessionObserver> {
        let _ = (product, local_identity);
        Box::new(NoObserver)
    }
}

pub trait SessionObserver {
    fn observe(&mut self, event: &ChatEvent);
    fn observe_left(&mut self, channel_index: u8);
    fn heartbeat(&mut self);
    fn reconcile(&mut self, snapshots: &[ChatEvent]);
    fn reject_channel(&mut self, channel: &ChatChannel);
    fn end_session(&mut self);
    fn resolve_pending_channels(&mut self);

    /// A Remastered chat event: talk, emote, whisper, broadcast, information,
    /// or error, against the channel the session is in.
    fn observe_classic(&mut self, event: &ClassicChatEvent) {
        let _ = event;
    }

    /// The Remastered channel as a whole — re-sent whenever the roster moves,
    /// because the classic channel describes its membership, not its changes.
    fn observe_classic_channel(&mut self, channel: &ClassicChatChannel) {
        let _ = channel;
    }

    /// A Reforged chat event.
    fn observe_warcraft(&mut self, event: &WarcraftChatEvent) {
        let _ = event;
    }

    /// The Reforged hall as a whole, re-sent whenever it changes.
    fn observe_warcraft_channel(&mut self, channel: &WarcraftChatChannel) {
        let _ = channel;
    }
}

pub struct NoObserver;

impl SessionObserverFactory for NoObserver {
    fn begin_session(&self, _channels: &[ChatChannel]) -> Box<dyn SessionObserver> {
        Box::new(NoObserver)
    }
}

impl SessionObserver for NoObserver {
    fn observe(&mut self, _event: &ChatEvent) {}
    fn observe_left(&mut self, _channel_index: u8) {}
    fn heartbeat(&mut self) {}
    fn reconcile(&mut self, _snapshots: &[ChatEvent]) {}
    fn reject_channel(&mut self, _channel: &ChatChannel) {}
    fn end_session(&mut self) {}
    fn resolve_pending_channels(&mut self) {}
}
