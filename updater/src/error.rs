use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the update operation was cancelled")]
    Cancelled,
    #[error("the update feed is invalid: {0}")]
    InvalidAppcast(String),
    #[error("the update artifact is invalid: {0}")]
    InvalidArtifact(String),
    #[error("the update signature is invalid: {0}")]
    InvalidSignature(String),
    #[error("the updater is in the wrong state: {0}")]
    InvalidState(&'static str),
    #[error("network request failed: {0}")]
    Network(String),
    #[error("no update artifact is available for {0}")]
    UnsupportedPlatform(String),
    #[error("failed to find the application inside {0}")]
    ApplicationNotFound(PathBuf),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("installation failed: {0}")]
    Installation(String),
    #[error("administrator authorization was cancelled")]
    AuthorizationCancelled,
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
