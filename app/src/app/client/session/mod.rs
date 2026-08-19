use super::*;

mod chat_events;
mod client_events;
mod controller;
mod dialogs;
mod lifecycle;
mod polling;
mod reconnect;
mod roster;
mod updates;

#[cfg(test)]
pub(in crate::app::client) use roster::adopt_identity;

const CHAT_LEAVE_BANNED: u16 = 315;
const CONNECTION_CONNECTED_HOLD: Duration = Duration::from_millis(900);
const STARTUP_UPDATE_TIMEOUT: Duration = Duration::from_secs(12);

pub(in crate::app::client) struct ClientRuntime {
    pub(in crate::app::client) app_menu_events: Receiver<AppMenuCommand>,
    pub(in crate::app::client) _app_menu_target: NativeAppMenuTarget,
    pub(in crate::app::client) live_mode: bool,
    pub(in crate::app::client) commands: Option<Sender<ClientCommand>>,
    pub(in crate::app::client) events: Option<Receiver<ClientEvent>>,
    pub(in crate::app::client) authenticator: Option<WebAuthenticatorHandle>,
    pub(in crate::app::client) uplink: uplink::UplinkControl,
    pub(in crate::app::client) live_auth_notified: bool,
}

impl ClientRuntime {
    pub(in crate::app::client) fn copy_live_link(&self, cx: &mut App) {
        if let Some(url) = self.uplink.stats.feed_url() {
            cx.write_to_clipboard(ClipboardItem::new_string(url));
        }
    }
}
