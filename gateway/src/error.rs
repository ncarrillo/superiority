use std::net::SocketAddr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Configuration(String),
    #[error("listener {0} is not loopback; pass --allow-remote to expose the gateway")]
    RemoteListenerNotAllowed(SocketAddr),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] superiority_core::Error),
    #[error("could not install the shutdown handler: {0}")]
    ShutdownHandler(#[from] ctrlc::Error),
    #[error("gateway worker panicked")]
    WorkerPanicked,
}

pub type Result<T> = std::result::Result<T, Error>;
