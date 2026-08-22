//! `StarCraft II`.
//!
//! `native` is the wire protocol and `native::schema` the types generated from
//! the client's own metadata blob; `bsn` and `metadata` are the bit-codec
//! engine that reads them, which is SC2-only despite looking general —
//! Remastered's classic channel carries protobuf-lite instead, and has no use
//! for any of it. `chat` is what the app talks to.

pub mod bsn;
pub mod chat;
pub mod metadata;
pub mod native;
