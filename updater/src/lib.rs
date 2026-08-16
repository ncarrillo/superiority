//! signed application updates for macos and windows.
//!
//! the parser preserves the legacy appcast shape during migration.

mod appcast;
mod download;
mod engine;
mod error;
mod install;
mod platform;
mod protocol;
mod publishing;

pub use appcast::{Appcast, Artifact, Platform, Release, SUPERIORITY_NAMESPACE, compare_versions};
pub use engine::{Client, Config};
pub use error::{Error, Result};
pub use install::{InstallPlan, PreparedUpdate};
pub use platform::{
    installation_needs_authorization, launch_elevated_worker, relaunch, show_progress,
    wait_for_process,
};
pub use protocol::Event;
pub use publishing::{
    add_platform_artifact, preserve_platform_artifacts, publish_macos_release,
    publish_platform_release,
};
