use super::*;

impl SettingsComponent {
    pub(super) fn live_settings_page(
        &self,
        mut page: Stateful<Div>,
        chrome: &ChromeComponent,
        live_url: Option<String>,
        live_error: Option<String>,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let live_link_target = if self.live_enabled && live_error.is_none() {
            live_url.clone()
        } else {
            None
        };
        let live_link = if self.live_enabled {
            if let Some(error) = live_error {
                error
            } else {
                live_url
                    .clone()
                    .unwrap_or_else(|| "Generating link…".to_owned())
            }
        } else {
            "Enable Live to get your link.".to_owned()
        };
        let mut live_link_view = div()
            .id("live-link")
            .absolute()
            .left(px(22.0))
            .top(px(295.0))
            .w(px(506.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .overflow_hidden()
            .whitespace_nowrap()
            .font_weight(FontWeight::BOLD)
            .text_size(px(14.0))
            .child(live_link);
        if let Some(url) = live_link_target {
            live_link_view = live_link_view
                .cursor_pointer()
                .underline()
                .text_color(rgb(0x33a8f0))
                .hover(|style| style.text_color(rgb(0x85d1ff)))
                .active(|style| style.opacity(0.68))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(&url);
                });
        }
        let mut copy_button = div()
            .id("live-copy")
            .absolute()
            .left(px(534.0))
            .top(px(290.0))
            .w(px(80.0))
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .font_family(FONT_NAVIGATION)
            .text_size(px(11.0))
            .text_color(rgb(0x33a8f0))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(0x85d1ff)).shadow_lg())
            .active(|style| style.opacity(0.62))
            .child("COPY");
        if live_url.is_some() {
            copy_button = copy_button.on_click(cx.listener(|this, _, _, cx| {
                this.runtime.copy_live_link(cx);
                cx.stop_propagation();
            }));
        } else {
            copy_button = copy_button.opacity(0.46).cursor_default();
        }
        let live_checkbox = self
            .settings_checkbox_visual(
                "live-setting-checkbox",
                LIVE_SETTING_ENABLED,
                self.live_enabled,
                chrome,
            )
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.settings
                    .set_tooltip(SETTINGS_TOOLTIP_LIVE_ENABLED, *hovered);
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings.toggle_live(&this.runtime.uplink);
                cx.stop_propagation();
                cx.notify();
            }));
        page = page
            .child(
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
                    .child("Live"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(143.0))
                    .w(px(640.0))
                    .text_size(px(12.5))
                    .child("Live streams your open channels to a web page anyone with your link can watch."),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(165.0))
                    .w(px(640.0))
                    .text_size(px(11.2))
                    .text_color(rgb(0x7d8fa8))
                    .child("Messages, joins and leaves, and the member list appear there seconds after they happen. Turn Live off and the stream stops."),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(203.0))
                    .w(px(650.0))
                    .h(px(44.0))
                    .child(live_checkbox)
                    .child(
                        div()
                            .absolute()
                            .left(px(52.0))
                            .top(px(15.0))
                            .w(px(470.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.0))
                            .text_color(rgb(0x6bc2f2))
                            .child("Enable Live"),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(273.0))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(11.5))
                    .text_color(rgb(0x6bc2f2))
                    .child("Your live link"),
            )
            .child(live_link_view)
            .child(copy_button);
        page
    }
}
