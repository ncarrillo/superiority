use super::*;

mod chat_events;
mod client_events;
mod controller;
mod dialogs;
mod lifecycle;
mod polling;
mod product;
mod reconnect;
mod roster;
mod updates;

#[cfg(test)]
pub(in crate::app::client) use roster::adopt_identity;

const CHAT_LEAVE_BANNED: u16 = 315;
const CONNECTION_CONNECTED_HOLD: Duration = Duration::from_millis(900);
const STARTUP_UPDATE_TIMEOUT: Duration = Duration::from_secs(12);

pub(in crate::app::client) use product::{ProductSession, ProductUiState, Sc2SessionUi};

pub(in crate::app::client) struct ClientRuntime {
    pub(in crate::app::client) app_menu_events: Receiver<AppMenuCommand>,
    pub(in crate::app::client) _app_menu_target: NativeAppMenuTarget,
    pub(in crate::app::client) live_mode: bool,
    /// The picker now reports the handshake on the `StarCraft II` card, so the
    /// dialog that used to report it is kept but not shown. `SUPERIORITY_LEGACY_CONNECT=1`
    /// puts it back, for as long as it is worth keeping around.
    pub(in crate::app::client) legacy_connect_dialog: bool,
    pub(in crate::app::client) authenticator: Option<WebAuthenticatorHandle>,
    pub(in crate::app::client) uplink: uplink::UplinkControl,
    /// The Live publisher, kept so every product's worker taps the same feed.
    pub(in crate::app::client) publisher: uplink::Publisher,
    pub(in crate::app::client) live_auth_notified: bool,
    /// The first (SC2/front) session establishes the one Battle.net identity
    /// every product credential must resolve to.
    pub(in crate::app::client) authoritative_account_id: Option<u64>,
    pub(in crate::app::client) authoritative_battle_tag: Option<String>,
    /// Region returned by the authoritative Battle.net logon. This belongs to
    /// the account strip on the game picker; it must not drift with whichever
    /// product session happens to be focused when the picker is reopened.
    pub(in crate::app::client) authoritative_region: Option<u32>,
    /// Products licensed to that identity. This is both the picker visibility
    /// set and the allow-list for background connection work.
    pub(in crate::app::client) provisioned: BTreeSet<Product>,
    /// Products whose worker is running but whose `Connect` has not been sent.
    ///
    /// Every product's session exists from startup, but they sign in one at a
    /// time: Battle.net's web sign-in is interactive and the app can only
    /// present one at a time, so sending every `Connect` at once had the second
    /// request replace the first and the first come back "cancelled". The queue
    /// advances when the one in flight either signs in or fails.
    pub(in crate::app::client) connect_queue: VecDeque<Product>,
    /// The product whose sign-in is in flight, if any.
    pub(in crate::app::client) connecting: Option<Product>,
}

impl ClientRuntime {
    pub(in crate::app::client) fn copy_live_link(&self, cx: &mut App) {
        if let Some(url) = self.uplink.stats.feed_url() {
            cx.write_to_clipboard(ClipboardItem::new_string(url));
        }
    }
}
