// derive output refers to this crate by its library name.
extern crate self as sc2_core;

pub mod auth;
pub mod bgs;
pub mod bsn;
pub mod chat;
pub mod connection;
pub mod error;
pub mod metadata;
pub mod native;
pub mod observer;
pub mod wire;

pub use error::{Error, Result};
