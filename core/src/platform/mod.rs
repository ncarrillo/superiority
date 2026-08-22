//! Battle.net itself: the parts that are not tied to any one game.
//!
//! A product's own protocol lives under [`crate::games`]. What is here is what
//! every product shares — the socket it connects over, the service that signs
//! it in, and the credential cache that keeps it signed in.

pub mod auth;
pub mod bgs;
pub mod wire;
