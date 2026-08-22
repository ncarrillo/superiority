use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bgs protocol error: {0}")]
    BgsWire(String),
    #[error("bsn protocol error: {0}")]
    BsnWire(String),
    /// Remastered's classic channel: its descriptor-free protobuf, its RPC
    /// framing, and the check-value envelope around both.
    #[error("classic protocol error: {0}")]
    ClassicWire(String),
    #[error("metadata error: {0}")]
    Metadata(String),
    #[error("authentication error: {0}")]
    Authentication(String),
    #[error("native protocol error: {0}")]
    Native(String),
    #[error("unsupported native record route slot={slot} command={command}")]
    UnmappedNativeRoute { slot: u8, command: u8 },
    #[error("native server rejected the connection: error={error_code}")]
    NativeServerRejected { error_code: i128 },
    #[error("native Battle.net resume was rejected (error={error_code} wait={wait})")]
    NativeResumeRejected { error_code: i128, wait: i128 },
    #[error("incomplete frame: {0}")]
    IncompleteFrame(String),
    #[error("server rejected the request: {0}")]
    Server(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    WebSocket(#[from] tungstenite::Error),
    #[error(transparent)]
    ProtobufDecode(#[from] prost::DecodeError),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
