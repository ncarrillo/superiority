mod client;
mod preferences;
mod preview;
mod protocol_viewer;
mod update;
mod web_auth;

// the connection orchestration moved into the core crate; re-exported here so
// the rest of `app` and `main` keep using `app::spawn_client` and friends.
pub use crate::connection::{ClientCommand, ClientEvent, ConnectionStage, spawn_client};
pub use client::run;
pub use protocol_viewer::run as run_protocol_viewer;
