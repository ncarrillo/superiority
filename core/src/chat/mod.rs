mod cache_catalog;
mod error_catalog;
mod portrait_catalog;
mod public_channels;
mod session;

/// download a Battle.net catalog body (e.g. the public-channel list) from the
/// depot, given the `CacheStreamItems` the server pushed. this is the exact fetch
/// the client performs — the depot is public and content-addressed. used to
/// pre-fetch and cache catalogs so a Sunken server can serve them.
pub fn download_catalog(
    response: &crate::native::model::CacheStreamItems,
    label: &str,
) -> crate::Result<Vec<u8>> {
    cache_catalog::load(response, label)
}

pub use public_channels::PublicChannel;
pub use session::{
    BlockedAccount, ChatChannel, ChatEvent, ChatFriend, ChatUser, GENERAL_PUBLIC_CHANNEL,
    JOIN_LOCALE, LiveChat,
    RosterSnapshot, channel_title, strip_character_code,
};
