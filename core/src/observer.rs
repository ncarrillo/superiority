use crate::chat::ChatEvent;

pub trait SessionObserverFactory: Send {
    fn begin_session(&self) -> Box<dyn SessionObserver>;
}

pub trait SessionObserver {
    fn observe(&mut self, event: &ChatEvent);
    fn observe_left(&mut self, channel_index: u8);
    fn heartbeat(&mut self);
}

pub struct NoObserver;

impl SessionObserverFactory for NoObserver {
    fn begin_session(&self) -> Box<dyn SessionObserver> {
        Box::new(NoObserver)
    }
}

impl SessionObserver for NoObserver {
    fn observe(&mut self, _event: &ChatEvent) {}
    fn observe_left(&mut self, _channel_index: u8) {}
    fn heartbeat(&mut self) {}
}
