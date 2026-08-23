//! reusable GPUI for Superiority's native and browser hosts.
//!
//! Product-neutral mechanisms live in [`foundation`] and [`patterns`]. A
//! renderer or presentation model that speaks a game's vocabulary is named
//! under [`products`] even when more than one host consumes it.

pub mod foundation;
pub mod patterns;
pub mod products;
