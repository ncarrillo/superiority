#[allow(clippy::doc_markdown, clippy::must_use_candidate)]
pub mod generated {
    include!("generated/bgs_generated.rs");
}

mod client;
mod model;

pub use client::{ChallengeHandler, Client, Endpoint};
pub use model::{
    LogonSession, NativeHandoff, SecretBytes, build_front_request, default_logon_request, fourcc,
};
