#![expect(
    clippy::struct_excessive_bools,
    reason = "generated structures preserve the protocol schema"
)]

pub mod account;
pub mod achievement;
pub mod authentication;
pub mod cache;
pub mod chat;
pub mod club;
pub mod comsat;
pub mod conference;
pub mod connection;
pub mod datagramconnection;
pub mod defines;
pub mod frame;
pub mod friends;
pub mod gameaccount;
pub mod ip4;
pub mod league;
pub mod matchmaker;
pub mod party;
pub mod presence;
pub mod profile;
pub mod realm;
pub mod regulator;
pub mod s2game;
pub mod s2map;
pub mod s2master;
pub mod starcraft2;
pub mod statistics;
pub mod toon;
pub mod version;
pub(crate) mod wire;
