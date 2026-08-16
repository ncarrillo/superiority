use super::*;

mod actions;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::app::client) enum Overlay {
    Account,
    Friends,
    Join,
    Settings,
}

pub(in crate::app::client) const MODAL_CLOSE_DURATION: Duration = Duration::from_millis(260);

pub(super) struct OverlayComponent {
    pub(super) active: Option<Overlay>,
    pub(super) closing: bool,
    pub(super) epoch: u64,
}

impl OverlayComponent {
    pub(super) fn dimmer(&self) -> AnyElement {
        ui_modal::dimmer(self.closing)
    }

    pub(super) fn animated(
        &self,
        overlay: Stateful<Div>,
        open_id: &'static str,
        close_id: &'static str,
        account: bool,
    ) -> AnyElement {
        if self.closing && account {
            return overlay
                .with_animation(
                    close_id,
                    Animation::new(Duration::from_millis(130)).with_easing(ease_in_out),
                    |overlay, delta| overlay.opacity(1.0 - delta),
                )
                .into_any_element();
        }
        if account {
            overlay
                .with_animation(
                    open_id,
                    Animation::new(Duration::from_millis(140)).with_easing(ease_in_out),
                    |overlay, delta| overlay.opacity(delta),
                )
                .into_any_element()
        } else {
            overlay.into_any_element()
        }
    }
}

impl OverlayComponent {
    pub(in crate::app::client) fn account(
        &self,
        identity: Option<(&UiUser, &str)>,
        connection_stage: &ConnectionStage,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let account_name = identity.map_or_else(
            || "YOUR BATTLE.NET ACCOUNT".to_owned(),
            |(user, _)| user.name.clone(),
        );
        let account_detail = identity.map_or_else(
            || match connection_stage {
                ConnectionStage::Connected => "Presence unknown  ·  Battle.net".to_owned(),
                ConnectionStage::Disconnected => "Offline".to_owned(),
                _ => "Connecting…".to_owned(),
            },
            |(user, channel)| format!("{}  ·  {channel}", user.presence.label()),
        );
        let account_detail_color =
            identity.map_or(rgb(0x7d8fa8), |(user, _)| user.presence.text_color());
        let account_portrait = identity.and_then(|(user, _)| user.portrait.clone());
        let account_menu = div()
            .id("account-menu")
            .absolute()
            .top(px(51.0))
            .w(px(292.0))
            .h(px(196.0))
            .bg(rgba(0x050a0ffc))
            .border_1()
            .border_color(rgb(0x33a8f0))
            .rounded(px(3.0))
            .shadow_lg()
            .font_family(FONT_INTERFACE)
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                account_portrait
                    .map_or_else(|| img("images/icons/account-placeholder.png"), img)
                    .absolute()
                    .left(px(20.0))
                    .top(px(13.0))
                    .size(px(44.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                img("images/nine-patch/portraits/frame.png")
                    .absolute()
                    .left(px(16.0))
                    .top(px(9.0))
                    .size(px(52.0))
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.0))
                    .top(px(11.0))
                    .w(px(194.0))
                    .h(px(23.0))
                    .flex()
                    .items_center()
                    .font_family(FONT_INTERNATIONAL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(rgb(0xd6e0f0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(account_name),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.0))
                    .top(px(36.0))
                    .w(px(194.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(account_detail_color)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(account_detail),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .top(px(69.0))
                    .w(px(256.0))
                    .h(px(1.0))
                    .bg(rgb(0x174f78)),
            )
            .child(
                chrome
                    .action_button("account-settings", "SETTINGS", 260.0, 42.0, false)
                    .absolute()
                    .left(px(16.0))
                    .top(px(84.0))
                    .text_size(px(12.0))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_settings(cx);
                    })),
            )
            .child(
                chrome
                    .action_button("account-sign-out", "SIGN OUT", 260.0, 42.0, true)
                    .absolute()
                    .left(px(16.0))
                    .top(px(138.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.sign_out(window, cx);
                    })),
            )
            .child(
                div()
                    .absolute()
                    .left(px(10.0))
                    .top(px(193.0))
                    .w(px(272.0))
                    .h(px(2.0))
                    .bg(rgb(0x33a8f0)),
            );
        #[cfg(target_os = "windows")]
        let account_menu = account_menu.left_0();
        #[cfg(target_os = "macos")]
        let account_menu = account_menu.right_0();
        let overlay = div()
            .id("account-menu-dismiss")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_overlay(window, cx);
            }))
            .child(account_menu);
        self.animated(
            overlay,
            "account-overlay-open",
            "account-overlay-close",
            true,
        )
    }
}
