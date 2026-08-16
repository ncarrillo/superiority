use super::*;

impl WarningComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let (title, detail, disconnected) = match self.warning_dialog.as_ref() {
            Some(WarningDialog::Disconnected { detail }) => {
                ("ERROR".to_owned(), detail.clone(), true)
            }
            Some(WarningDialog::Channel { title, detail, .. }) => {
                (title.clone(), detail.clone(), false)
            }
            None => (String::new(), String::new(), false),
        };
        let mut card = div()
            .id("warning-modal")
            .relative()
            .w(px(620.0))
            .h(px(310.0))
            .font_family(FONT_INTERFACE)
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .top(px(18.0))
                    .w(px(584.0))
                    .h(px(274.0))
                    .bg(rgba(0x1b0f04fc)),
            );
        if disconnected {
            card = card
                .child(
                    img("images/dialogs/warning-glow-left.png")
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(310.0))
                        .h(px(310.0))
                        .opacity(0.42)
                        .object_fit(ObjectFit::Fill),
                )
                .child(
                    img("images/dialogs/warning-glow-right.png")
                        .absolute()
                        .left(px(310.0))
                        .top_0()
                        .w(px(310.0))
                        .h(px(310.0))
                        .opacity(0.42)
                        .object_fit(ObjectFit::Fill),
                )
                .child(
                    img("images/dialogs/warning-hex-top.png")
                        .absolute()
                        .left(px(4.0))
                        .top(px(30.0))
                        .w(px(612.0))
                        .h(px(145.0))
                        .opacity(0.3)
                        .object_fit(ObjectFit::Fill),
                )
                .child(
                    img("images/dialogs/warning-hex-bottom.png")
                        .absolute()
                        .left(px(4.0))
                        .top(px(143.0))
                        .w(px(612.0))
                        .h(px(145.0))
                        .opacity(0.24)
                        .object_fit(ObjectFit::Fill),
                );
        }
        card = card.child(ui_modal::warning_header(620.0, title)).child(
            div()
                .absolute()
                .left(px(34.0))
                .top(px(if disconnected { 122.0 } else { 110.0 }))
                .w(px(552.0))
                .h(px(if disconnected { 48.0 } else { 72.0 }))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .whitespace_normal()
                        .text_center()
                        .text_size(px(19.5))
                        .line_height(px(24.0))
                        .text_color(rgb(0xedc78c))
                        .child(detail),
                ),
        );
        if disconnected {
            card = card
                .child(
                    chrome
                        .action_button("disconnect-quit", "QUIT", 185.0, 50.0, true)
                        .absolute()
                        .left(px(117.0))
                        .top(px(215.0))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.quit_after_disconnect(cx);
                        })),
                )
                .child(
                    chrome
                        .action_button("disconnect-reconnect", "RECONNECT", 185.0, 50.0, true)
                        .absolute()
                        .left(px(318.0))
                        .top(px(215.0))
                        .on_click(cx.listener(|this, _, _, cx| this.reconnect(cx))),
                );
        } else {
            card = card.child(
                chrome
                    .action_button("channel-error-close", "CLOSE", 185.0, 50.0, true)
                    .absolute()
                    .left(px(217.5))
                    .top(px(215.0))
                    .on_click(cx.listener(|this, _, _, cx| this.begin_warning_close(cx))),
            );
        }
        card = card.child(
            img("images/dialogs/warning-frame.png")
                .absolute()
                .inset_0()
                .object_fit(ObjectFit::Fill),
        );
        let dimmer = div().absolute().inset_0().bg(rgba(0x000305ad));
        let dimmer = if self.warning_closing {
            dimmer
                .with_animation(
                    "warning-dimmer-close",
                    Animation::new(Duration::from_millis(140)),
                    |dimmer, delta| dimmer.opacity(1.0 - delta),
                )
                .into_any_element()
        } else {
            dimmer
                .with_animation(
                    "warning-dimmer-open",
                    Animation::new(Duration::from_millis(160)),
                    |dimmer, delta| dimmer.opacity(delta),
                )
                .into_any_element()
        };
        div()
            .id("warning-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(dimmer)
            .child(ui_modal::animated(
                card,
                self.warning_closing,
                true,
                620.0,
                310.0,
                "warning-panel-open",
                "warning-panel-close",
                "warning-scan-open",
                "warning-scan-close",
            ))
    }
}
