//! what the app is allowed to see of core.
//!
//! Deliberately narrow. This used to re-export all eleven of core's modules
//! flat, which is why `crate::native::` — `StarCraft II`'s wire protocol — used
//! to resolve anywhere in the UI without anyone noticing. `chat`, `connection`, and
//! `product` are the app-facing surface; anything reaching past them into one
//! game's protocol has to name `superiority_core::` in full, at the site, where it can
//! be seen.
pub use superiority_core::{Error, Product, Result, chat, connection, product};

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod app;
pub mod uplink;
