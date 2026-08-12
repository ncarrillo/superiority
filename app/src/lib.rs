pub use sc2_core::{
    Error, Result, auth, bgs, bsn, chat, connection, error, metadata, native, observer, wire,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod app;
pub mod uplink;
