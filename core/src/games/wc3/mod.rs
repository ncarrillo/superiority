//! Warcraft III: Reforged.
//!
//! Reforged uses a JSON BGS v2 account channel and then hands off to a classic
//! RPC channel. The second channel shares SC:R's descriptor-free RPC framing
//! and check-value transform, but its authentication body, chat service, and
//! product seed are WC3-specific.

mod account;
mod classic;
mod identity;
mod protocol;
mod schema;
pub mod session;

pub use classic::{
    ChatChannel, ChatEvent, ChatFriend, ChatMember, ChatPresence, ClanInfo, ClanMember,
    ClanMembership, ClanSnapshot, FriendPresence, GameListing, PublicChannel,
};
