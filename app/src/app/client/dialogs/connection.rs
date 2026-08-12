use super::*;

const CONNECTION_HEADER: ui_modal::HeaderLayout =
    ui_modal::HeaderLayout::new((24.0, 12.0, 492.0, 57.0), 54.0, 432.0, 15.5);

impl ConnectionComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let (title, detail, step, title_color, progress_color, reveal_ids) =
            if let Some(error) = &self.error {
                (
                    "COULDN’T CONNECT",
                    compact_error(error),
                    String::new(),
                    rgb(0xf2705c),
                    rgb(0xf2705c),
                    ("connection-title-error", "connection-detail-error"),
                )
            } else if self.starting {
                (
                    "GETTING READY",
                    "Starting the connection.".to_owned(),
                    String::new(),
                    rgb(0xd6e0f0),
                    rgb(0x33a8f0),
                    ("connection-title-start", "connection-detail-start"),
                )
            } else if self.signed_out {
                (
                    "SIGNED OUT",
                    "Sign in again whenever you're ready.".to_owned(),
                    String::new(),
                    rgb(0xd6e0f0),
                    rgb(0x33a8f0),
                    (
                        "connection-title-signed-out",
                        "connection-detail-signed-out",
                    ),
                )
            } else {
                match self.stage {
                    ConnectionStage::Disconnected => (
                        "NOT CONNECTED",
                        "You're offline. Reconnect whenever you're ready.".to_owned(),
                        String::new(),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-offline", "connection-detail-offline"),
                    ),
                    ConnectionStage::WebAuthentication => (
                        "SIGNING IN",
                        "Signing in to your Battle.net account.".to_owned(),
                        format!("Step 1 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-web", "connection-detail-web"),
                    ),
                    ConnectionStage::GameUtilities => (
                        "FINDING YOUR GAME",
                        "Looking up your StarCraft II account.".to_owned(),
                        format!("Step 2 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-game", "connection-detail-game"),
                    ),
                    ConnectionStage::NativeAuthentication => (
                        "CONNECTING",
                        "Connecting to StarCraft II.".to_owned(),
                        format!("Step 3 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-native", "connection-detail-native"),
                    ),
                    ConnectionStage::ChatBootstrap => (
                        "JOINING CHAT",
                        "Loading your channels and friends.".to_owned(),
                        format!("Step 4 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-chat", "connection-detail-chat"),
                    ),
                    ConnectionStage::Connected => (
                        "CONNECTED",
                        "Chat is ready.".to_owned(),
                        String::new(),
                        rgb(0x47d185),
                        rgb(0x47d185),
                        ("connection-title-done", "connection-detail-done"),
                    ),
                }
            };
        let retry = self.error.is_some() || self.stage == ConnectionStage::Disconnected;
        let title = div()
            .id(reveal_ids.0)
            .absolute()
            .left(px(42.0))
            .top(px(84.0))
            .w(px(400.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .font_weight(FontWeight::BOLD)
            .text_size(px(17.0))
            .text_color(title_color)
            .child(title)
            .with_animation(
                reveal_ids.0,
                Animation::new(Duration::from_millis(180)),
                |title, delta| title.opacity(0.55 + delta * 0.45),
            );
        let detail = div()
            .id(reveal_ids.1)
            .absolute()
            .left(px(42.0))
            .top(px(112.0))
            .w(px(420.0))
            .h(px(18.0))
            .flex()
            .items_center()
            .text_size(px(12.0))
            .text_color(rgb(0x7d8fa8))
            .child(detail)
            .with_animation(
                reveal_ids.1,
                Animation::new(Duration::from_millis(180)),
                |detail, delta| detail.opacity(0.62 + delta * 0.38),
            );
        let mut panel = div()
            .id("connection-modal")
            .relative()
            .w(px(540.0))
            .h(px(250.0))
            .font_family(FONT_INTERFACE)
            .text_color(rgb(0xd6e0f0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(chrome.modal_chrome(540.0, 250.0))
            .child(chrome.modal_header(CONNECTION_HEADER, "BATTLE.NET CONNECTION"))
            .child(title)
            .child(detail)
            .child(
                div()
                    .absolute()
                    .left(px(42.0))
                    .top(px(158.0))
                    .w(px(CONNECTION_RAIL))
                    .h(px(4.0))
                    .bg(rgb(0x0a1a24)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(42.0))
                    .top(px(158.0))
                    .w(px(CONNECTION_RAIL * self.fill.clamp(0.0, 1.0)))
                    .h(px(4.0))
                    .bg(progress_color),
            )
            .child(
                div()
                    .absolute()
                    .left(px(338.0))
                    .top(px(168.0))
                    .w(px(160.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_size(px(11.5))
                    .text_color(rgb(0x7d8fa8))
                    .child(step),
            );
        if self.stage != ConnectionStage::Connected {
            panel = panel.child(
                chrome
                    .action_button(
                        "connection-action",
                        if retry { "RECONNECT" } else { "CANCEL" },
                        132.0,
                        36.0,
                        false,
                    )
                    .absolute()
                    .left(px(366.0))
                    .top(px(194.0))
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_connection(cx))),
            );
        }
        let dimmer = div().absolute().inset_0().bg(rgba(0x000305ad));
        let dimmer = if self.dialog_closing {
            dimmer
                .with_animation(
                    "connection-dimmer-close",
                    Animation::new(Duration::from_millis(140)),
                    |dimmer, delta| dimmer.opacity(1.0 - delta),
                )
                .into_any_element()
        } else {
            dimmer
                .with_animation(
                    "connection-dimmer-open",
                    Animation::new(Duration::from_millis(160)),
                    |dimmer, delta| dimmer.opacity(delta),
                )
                .into_any_element()
        };
        div()
            .id("live-connection")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(dimmer)
            .child(ui_modal::animated(
                panel,
                self.dialog_closing,
                false,
                540.0,
                250.0,
                "connection-panel-open",
                "connection-panel-close",
                "connection-scan-open",
                "connection-scan-close",
            ))
    }
}
