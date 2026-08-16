use super::*;

impl SettingsComponent {
    pub(super) fn chat_settings_page(
        &self,
        mut page: Stateful<Div>,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        page = page.child(
            div()
                .absolute()
                .left(px(22.0))
                .top(px(100.0))
                .w(px(400.0))
                .h(px(30.0))
                .flex()
                .items_center()
                .font_weight(FontWeight::BOLD)
                .text_size(px(20.0))
                .child("Chat"),
        );
        for (index, (title, checked)) in [
            ("Show timestamps", self.show_timestamps),
            ("Join / leave notifications", self.show_membership),
        ]
        .into_iter()
        .enumerate()
        {
            let tooltip = if index == 0 {
                SETTINGS_TOOLTIP_TIMESTAMPS
            } else {
                SETTINGS_TOOLTIP_MEMBERSHIP
            };
            let checkbox = self
                .settings_checkbox_visual(("chat-setting-checkbox", index), index, checked, chrome)
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.settings.set_tooltip(tooltip, *hovered);
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if index == 0 {
                        let checked = !this.settings.show_timestamps;
                        this.settings.begin_checkbox_animation(
                            CHAT_SETTING_TIMESTAMPS,
                            this.settings.show_timestamps,
                            checked,
                        );
                        this.settings.show_timestamps = checked;
                        preferences::save_show_timestamps(this.settings.show_timestamps);
                    } else {
                        let checked = !this.settings.show_membership;
                        this.settings.begin_checkbox_animation(
                            CHAT_SETTING_MEMBERSHIP,
                            this.settings.show_membership,
                            checked,
                        );
                        this.settings.show_membership = checked;
                        preferences::save_show_membership(this.settings.show_membership);
                        this.settings.push_live_config(&this.runtime.uplink);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }));
            page = page.child(
                div()
                    .id(("chat-setting", index))
                    .absolute()
                    .left(px(22.0))
                    .top(px(149.0 + index as f32 * 48.0))
                    .w(px(650.0))
                    .h(px(44.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .pl(px(18.0))
                    .child(checkbox)
                    .child(
                        div()
                            .w(px(470.0))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.0))
                            .text_color(rgb(0x6bc2f2))
                            .child(title),
                    ),
            );
        }
        page
    }
}
