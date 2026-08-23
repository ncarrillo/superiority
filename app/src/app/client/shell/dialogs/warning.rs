use super::*;

impl WarningComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        variant: ui_shared_modal::ModalVariant,
        textures: &ui_shared_modal::ModalTextures,
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
        // the modern alarm shell: the legacy hex-and-glow PNG stack becomes
        // the shared error dressing — the old orange dialog reborn in vector
        let card = div()
            .id("warning-modal")
            .relative()
            .w(px(620.0))
            .h(px(310.0))
            .font_family(FONT_INTERFACE)
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(ui_shared_modal::error_frame(
                variant, 620.0, 310.0, textures,
            ))
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .right(px(18.0))
                    .top(px(34.0))
                    .child(ui_shared_modal::error_title(variant, &title)),
            );
        let mut card = card.child(
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
                        .text_color(rgb(0x00ed_c78c))
                        .child(detail),
                ),
        );
        if disconnected {
            card = card
                .child(
                    ui_buttons::button(
                        "disconnect-quit",
                        variant,
                        ui_buttons::ButtonWeight::Ghost,
                        ui_buttons::ButtonTone::Danger,
                        ui_buttons::ButtonLife::Ready,
                        ui_buttons::worded(variant, "QUIT"),
                    )
                    .w(px(185.0))
                    .h(px(50.0))
                    .absolute()
                    .left(px(117.0))
                    .top(px(215.0))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.quit_after_disconnect(cx);
                    })),
                )
                .child(
                    ui_buttons::button(
                        "disconnect-reconnect",
                        variant,
                        ui_buttons::ButtonWeight::Primary,
                        ui_buttons::ButtonTone::Danger,
                        ui_buttons::ButtonLife::Ready,
                        ui_buttons::worded(variant, "RECONNECT"),
                    )
                    .w(px(185.0))
                    .h(px(50.0))
                    .absolute()
                    .left(px(318.0))
                    .top(px(215.0))
                    .on_click(cx.listener(|this, _, _, cx| this.reconnect(cx))),
                );
        } else {
            card = card.child(
                ui_buttons::button(
                    "channel-error-close",
                    variant,
                    ui_buttons::ButtonWeight::Primary,
                    ui_buttons::ButtonTone::Danger,
                    ui_buttons::ButtonLife::Ready,
                    ui_buttons::worded(variant, "CLOSE"),
                )
                .w(px(185.0))
                .h(px(50.0))
                .absolute()
                .left(px(217.5))
                .top(px(215.0))
                .on_click(cx.listener(|this, _, _, cx| this.begin_warning_close(cx))),
            );
        }
        let dimmer = div().absolute().inset_0().bg(rgba(0x0003_05ad));
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
                    gpui::Styled::opacity,
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
            .child(ui_shared_modal::error_animated(
                variant,
                card,
                self.warning_closing,
                platform::reduce_motion(),
                620.0,
                310.0,
            ))
    }
}
