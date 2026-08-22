//! One module per game protocol.
//!
//! Battle.net's account layer is shared (see [`crate::platform`]); what a game
//! says once it is signed in is not. Each product speaks its own thing, so each
//! gets its own module rather than a set of flags on a shared one.

pub mod sc2;
pub mod scr;
pub mod wc3;
