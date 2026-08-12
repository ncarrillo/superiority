use crate::chat::{ChatChannel, ChatEvent};

pub trait SessionObserverFactory: Send {
    fn begin_session(&self, channels: &[ChatChannel]) -> Box<dyn SessionObserver>;
}

pub trait SessionObserver {
    fn observe(&mut self, event: &ChatEvent);
    fn observe_left(&mut self, channel_index: u8);
    fn heartbeat(&mut self);
    fn reconcile(&mut self, snapshots: &[ChatEvent]);
    fn reject_channel(&mut self, channel: &ChatChannel);
    fn end_session(&mut self);
    fn resolve_pending_channels(&mut self);
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
