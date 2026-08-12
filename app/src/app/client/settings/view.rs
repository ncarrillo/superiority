use super::*;

const SETTINGS_HEADER: ui_modal::HeaderLayout =
    ui_modal::HeaderLayout::new((84.0, 0.0, 776.0, 90.0), 272.0, 400.0, 20.5);

impl SettingsComponent {
    pub(in crate::app::client) fn modal(
        &self,
        chrome: &ChromeComponent,
        blocked_accounts: &[BlockedAccount],
        live_url: Option<String>,
        live_error: Option<String>,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let mut modal = div()
            .id("settings-modal")
            .relative()
            .w(px(944.0))
            .h(px(620.0))
            .font_family(FONT_INTERFACE)
            .text_color(rgb(0xd6e0f0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(chrome.modal_chrome(944.0, 620.0))
            .child(
                div()
                    .absolute()
                    .left(px(235.0))
                    .top(px(532.0))
                    .w(px(692.0))
                    .h(px(71.0))
                    .bg(rgba(0x02080dfc)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(234.0))
                    .top(px(94.0))
                    .w(px(694.0))
                    .h(px(510.0))
                    .border_1()
                    .border_color(rgba(0x144f78d9))
                    .rounded(px(2.0)),
            )
            .child(chrome.modal_header(SETTINGS_HEADER, "SETTINGS"));

        for (index, title) in ["Appearance", "Chat", "Privacy", "Live"]
            .into_iter()
            .enumerate()
        {
            let active = self.active_settings_page == index;
            let group = [
                "settings-nav-appearance",
                "settings-nav-chat",
                "settings-nav-privacy",
                "settings-nav-live",
            ][index];
            let idle_image = chrome
                .button_frames
                .as_ref()
                .and_then(|frames| frames.image(190.0, 64.0, false, false))
                .map_or_else(|| img("images/nine-patch/controls/button-idle.png"), img);
            let active_image = chrome
                .button_frames
                .as_ref()
                .and_then(|frames| frames.image(190.0, 64.0, false, true))
                .map_or_else(|| img("images/nine-patch/controls/button-active.png"), img);
            modal = modal.child(
                div()
                    .group(group)
                    .id(("settings-navigation", index))
                    .absolute()
                    .left(px(28.0))
                    .top(px(68.0 + index as f32 * 74.0))
                    .w(px(190.0))
                    .h(px(64.0))
                    .flex()
                    .items_center()
                    .justify_start()
                    .pl(px(18.0))
                    .cursor_pointer()
                    .hover(|style| style.shadow_lg())
                    .child(
                        idle_image
                            .absolute()
                            .left_0()
                            .top(px(-BUTTON_ART_VERTICAL_BLEED))
                            .w_full()
                            .h(px(64.0 + BUTTON_ART_VERTICAL_BLEED * 2.0))
                            .opacity(if active { 0.0 } else { 1.0 })
                            .object_fit(ObjectFit::Fill),
                    )
                    .child(
                        active_image
                            .absolute()
                            .left_0()
                            .top(px(-BUTTON_ART_VERTICAL_BLEED))
                            .w_full()
                            .h(px(64.0 + BUTTON_ART_VERTICAL_BLEED * 2.0))
                            .opacity(if active { 1.0 } else { 0.0 })
                            .group_hover(group, |style| style.opacity(1.0))
                            .group_active(group, |style| style.opacity(1.0))
                            .object_fit(ObjectFit::Fill),
                    )
                    .font_family(FONT_NAVIGATION)
                    .text_size(px(12.0))
                    .text_color(if active { rgb(0xd6e0f0) } else { rgb(0x6bc2f2) })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.settings.active_settings_page == index {
                            return;
                        }
                        this.settings.settings_page_transition = Some(SettingsPageTransition {
                            outgoing: this.settings.active_settings_page,
                            started: Instant::now(),
                        });
                        this.settings.active_settings_page = index;
                        this.settings.settings_tooltip = None;
                        cx.notify();
                    }))
                    .child(div().relative().child(title)),
            );
        }

        let now = Instant::now();
        if let Some(transition) = &self.settings_page_transition {
            let progress = ease_in_out(
                (now.saturating_duration_since(transition.started)
                    .as_secs_f32()
                    / SETTINGS_PAGE_CROSSFADE_DURATION.as_secs_f32())
                .clamp(0.0, 1.0),
            );
            modal = modal
                .child(self.settings_page(
                    transition.outgoing,
                    1.0 - progress,
                    chrome,
                    blocked_accounts,
                    live_url.clone(),
                    live_error.clone(),
                    cx,
                ))
                .child(self.settings_page(
                    self.active_settings_page,
                    progress,
                    chrome,
                    blocked_accounts,
                    live_url.clone(),
                    live_error.clone(),
                    cx,
                ));
        } else {
            modal = modal.child(self.settings_page(
                self.active_settings_page,
                1.0,
                chrome,
                blocked_accounts,
                live_url,
                live_error,
                cx,
            ));
        }

        modal = modal
            .child(
                div()
                    .absolute()
                    .left(px(254.0))
                    .top(px(561.0))
                    .w(px(500.0))
                    .h(px(16.0))
                    .text_size(px(10.5))
                    .text_color(rgb(0x7d8fa8))
                    .child("Changes apply immediately and are remembered across launches."),
            )
            .child(
                chrome
                    .action_button("settings-done", "DONE", 142.0, 42.0, true)
                    .absolute()
                    .left(px(774.0))
                    .top(px(549.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            )
            .child(
                ui_controls::close_button("settings-close")
                    .absolute()
                    .left(px(899.0))
                    .top(px(18.0))
                    .w(px(28.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_overlay(window, cx);
                    })),
            );
        if let Some(tooltip) = self.tooltip_card(&chrome.ui_assets) {
            modal = modal.child(tooltip);
        }
        modal
    }
}

impl SettingsComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        chrome: &ChromeComponent,
        overlays: &OverlayComponent,
        blocked_accounts: &[BlockedAccount],
        live_url: Option<String>,
        live_error: Option<String>,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let modal = self.modal(chrome, blocked_accounts, live_url, live_error, cx);
        let overlay = div()
            .id("settings-dismiss")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(overlays.dimmer())
            .child(ui_modal::animated(
                modal,
                overlays.closing,
                false,
                944.0,
                620.0,
                "settings-panel-open",
                "settings-panel-close",
                "settings-scan-open",
                "settings-scan-close",
            ));
        overlays.animated(
            overlay,
            "settings-overlay-open",
            "settings-overlay-close",
            false,
        )
    }
}
