use super::*;
use gpui::Rgba;

/// What the connection is doing, said once. The picker's `StarCraft II` card and
/// the legacy dialog both report the handshake, and they used to word it
/// differently — same stage, two answers. This is the answer.
pub(in crate::app::client) struct ConnectionProgress {
    pub(in crate::app::client) title: &'static str,
    pub(in crate::app::client) detail: String,
    /// The same fact in a card's width: the dialog has a paragraph, the
    /// picker card has one line it shares with the step counter.
    pub(in crate::app::client) brief: String,
    pub(in crate::app::client) step: String,
    pub(in crate::app::client) title_color: Rgba,
    pub(in crate::app::client) progress_color: Rgba,
    pub(in crate::app::client) reveal_ids: (&'static str, &'static str),
}

impl ConnectionComponent {
    /// Starts one real connection attempt and clears any terminal state left
    /// by the previous one. Queued product sessions must do this before their
    /// worker can emit events, because `sign_out_requested` is also the guard
    /// that rejects late events from the session being signed out.
    pub(in crate::app::client) fn begin_attempt(&mut self) {
        self.sign_out_requested = false;
        self.signed_out = false;
        self.starting = true;
        self.error = None;
        self.fill = 0.0;
        self.floor = 0.0;
        self.ceiling = 0.0;
        self.progress_updated = Instant::now();
    }

    /// What the dialog and the card say about this connection.
    ///
    /// `product` is the game being connected to. Two of these lines name it,
    /// and they used to name `StarCraft II` outright — so Remastered's card
    /// said it was connecting to the wrong game.
    pub(in crate::app::client) fn progress(&self, product: Product) -> ConnectionProgress {
        let (title, detail, brief, step, title_color, progress_color, reveal_ids) =
            if let Some(error) = &self.error {
                (
                    "COULDN’T CONNECT",
                    compact_error(error),
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
                    "Getting ready".to_owned(),
                    String::new(),
                    rgb(0xd6e0f0),
                    rgb(0x33a8f0),
                    ("connection-title-start", "connection-detail-start"),
                )
            } else if self.signed_out {
                (
                    "SIGNED OUT",
                    "Sign in again whenever you're ready.".to_owned(),
                    "Signed out".to_owned(),
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
                        "Not connected".to_owned(),
                        String::new(),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-offline", "connection-detail-offline"),
                    ),
                    ConnectionStage::WebAuthentication => (
                        "SIGNING IN",
                        "Signing in to your Battle.net account.".to_owned(),
                        "Signing in".to_owned(),
                        format!("Step 1 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-web", "connection-detail-web"),
                    ),
                    ConnectionStage::GameUtilities => (
                        "FINDING YOUR GAME",
                        format!("Looking up your {} account.", product.name()),
                        "Finding your game".to_owned(),
                        format!("Step 2 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-game", "connection-detail-game"),
                    ),
                    ConnectionStage::NativeAuthentication => (
                        "CONNECTING",
                        format!("Connecting to {}.", product.name()),
                        "Connecting".to_owned(),
                        format!("Step 3 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-native", "connection-detail-native"),
                    ),
                    ConnectionStage::ChatBootstrap => (
                        "JOINING CHAT",
                        "Loading your channels and friends.".to_owned(),
                        "Joining chat".to_owned(),
                        format!("Step 4 of {CONNECTION_STEPS}"),
                        rgb(0xd6e0f0),
                        rgb(0x33a8f0),
                        ("connection-title-chat", "connection-detail-chat"),
                    ),
                    ConnectionStage::Connected => (
                        "CONNECTED",
                        "Chat is ready.".to_owned(),
                        "Connected".to_owned(),
                        String::new(),
                        rgb(0x47d185),
                        rgb(0x47d185),
                        ("connection-title-done", "connection-detail-done"),
                    ),
                }
            };
        ConnectionProgress {
            title,
            detail,
            brief,
            step,
            title_color,
            progress_color,
            reveal_ids,
        }
    }

    pub(in crate::app::client) fn overlay(
        &self,
        product: Product,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let ConnectionProgress {
            title,
            detail,
            brief: _,
            step,
            title_color,
            progress_color,
            reveal_ids,
        } = self.progress(product);
        let retry = self.error.is_some() || self.stage == ConnectionStage::Disconnected;
        let dressing = overlays::modal_variant(product);
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
            // the modern shell in the product's own dressing; a connection
            // that has failed wears the alarm variant instead
            .child(if retry {
                ui_shared_modal::error_frame(dressing, 540.0, 250.0, &chrome.modal_textures)
            } else {
                ui_shared_modal::frame(dressing, 540.0, 250.0, &chrome.modal_textures)
            })
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .right(px(18.0))
                    .top(px(30.0))
                    .child(ui_shared_modal::title(dressing, "BATTLE.NET CONNECTION")),
            )
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
                ui_buttons::button(
                    "connection-action",
                    dressing,
                    if retry {
                        ui_buttons::ButtonWeight::Primary
                    } else {
                        ui_buttons::ButtonWeight::Ghost
                    },
                    if retry {
                        ui_buttons::ButtonTone::Danger
                    } else {
                        ui_buttons::ButtonTone::Chrome
                    },
                    ui_buttons::ButtonLife::Ready,
                    ui_buttons::worded(dressing, if retry { "RECONNECT" } else { "CANCEL" }),
                )
                .w(px(132.0))
                .h(px(36.0))
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
            .child(if retry {
                ui_shared_modal::error_animated(
                    dressing,
                    panel,
                    self.dialog_closing,
                    platform::reduce_motion(),
                    540.0,
                    250.0,
                )
            } else {
                ui_shared_modal::animated(
                    dressing,
                    panel,
                    self.dialog_closing,
                    platform::reduce_motion(),
                    540.0,
                    250.0,
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_progress_names_the_game_it_is_connecting_to() {
        // these lines named StarCraft II outright, so Remastered's card said it
        // was connecting to a game it was not
        let component = ConnectionComponent {
            stage: ConnectionStage::NativeAuthentication,
            error: None,
            starting: false,
            signed_out: false,
            sign_out_requested: false,
            dialog_visible: true,
            dialog_closing: false,
            close_due: None,
            hide_due: None,
            fill: 0.0,
            floor: 0.0,
            ceiling: 1.0,
            progress_updated: Instant::now(),
        };
        for product in Product::ALL {
            let report = component.progress(product);
            assert!(
                report.detail.contains(product.name()),
                "{product:?} was told: {}",
                report.detail
            );
        }

        let finding = ConnectionComponent {
            stage: ConnectionStage::GameUtilities,
            ..component
        };
        assert!(
            finding
                .progress(Product::Remastered)
                .detail
                .contains(Product::Remastered.name())
        );
    }

    #[test]
    fn a_new_attempt_clears_the_sign_out_event_gate() {
        let mut component = ConnectionComponent {
            stage: ConnectionStage::Disconnected,
            error: Some("old failure".to_owned()),
            starting: false,
            signed_out: true,
            sign_out_requested: true,
            dialog_visible: true,
            dialog_closing: false,
            close_due: None,
            hide_due: None,
            fill: 1.0,
            floor: 0.75,
            ceiling: 1.0,
            progress_updated: Instant::now(),
        };

        component.begin_attempt();

        assert!(!component.sign_out_requested);
        assert!(!component.signed_out);
        assert!(component.starting);
        assert!(component.error.is_none());
        assert_eq!(component.fill, 0.0);
        assert_eq!(component.floor, 0.0);
        assert_eq!(component.ceiling, 0.0);
        assert_eq!(
            component.progress(Product::Remastered).title,
            "GETTING READY"
        );
    }
}
