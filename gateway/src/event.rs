use std::{
    net::SocketAddr,
    sync::mpsc::{SendError, Sender},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityKind {
    System,
    Legacy,
    Upstream,
    Warning,
    Error,
}

#[derive(Debug)]
pub enum Event {
    Status(String),
    Listening(SocketAddr),
    Upstream {
        peer: SocketAddr,
        toon: String,
        channel: Option<String>,
        roster: Vec<String>,
        friends: usize,
    },
    LegacyConnected(SocketAddr),
    LegacyDisconnected(SocketAddr),
    Activity {
        kind: ActivityKind,
        message: String,
    },
    Finished(Option<String>),
}

#[derive(Clone)]
pub struct Reporter(Sender<Event>);

impl Reporter {
    pub fn new(sender: Sender<Event>) -> Self {
        Self(sender)
    }

    pub fn send(&self, event: Event) -> Result<(), SendError<Event>> {
        self.0.send(event)
    }

    pub fn status(&self, status: impl Into<String>) {
        let _ = self.send(Event::Status(status.into()));
    }

    pub fn activity(&self, kind: ActivityKind, message: impl Into<String>) {
        let _ = self.send(Event::Activity {
            kind,
            message: message.into(),
        });
    }
}
