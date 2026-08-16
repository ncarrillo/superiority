use super::*;

mod actions;

pub(super) struct ComposerComponent {
    pub(super) composer_focused: bool,
    pub(super) composer: ui_text_input::TextInput,
}

impl ComposerComponent {
    pub(super) fn view(
        &self,
        window: &Window,
        has_channel: bool,
        online_friends: usize,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let focused = self.composer_focused && self.composer.is_focused(window);
        let placeholder = if has_channel {
            "Press Enter to chat"
        } else {
            "Use + to join a channel"
        };
        self.composer.set_placeholder(placeholder);
        let text_color = if self.composer.is_empty() {
            rgb(0x5e8291)
        } else {
            rgb(0xd6e0f0)
        };
        let border = if focused {
            rgb(0x33a8f0)
        } else {
            rgb(0x133e5b)
        };

        div()
            .flex()
            .gap(px(8.0))
            .h(px(COMPOSER_HEIGHT))
            .flex_shrink_0()
            .child(
                div()
                    .id("composer")
                    .flex()
                    .items_center()
                    .flex_1()
                    .px(px(12.0))
                    .bg(rgb(0x080d13))
                    .border_1()
                    .border_color(border)
                    .rounded(px(2.0))
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(13.5))
                    .text_color(text_color)
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_hover(cx.listener(|this, hovered, window, cx| {
                        this.set_composer_pointer_focus(*hovered, window, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            if this.channels.active().is_some() {
                                this.overlays.active = None;
                                this.chat.transcript.selection.clear();
                                this.composer.composer_focused = true;
                                this.roster.roster.focused = false;
                                this.composer.composer.focus(window, cx);
                                cx.notify();
                            }
                        }),
                    )
                    .child(self.composer.element()),
            )
            .child(
                div()
                    .relative()
                    .w(px(56.0))
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("friends")
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(0x101a2a))
                            .border_1()
                            .border_color(rgb(0x2c425d))
                            .rounded(px(2.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x16273e)).border_color(rgb(0x3e6e9e)))
                            .active(|style| style.bg(rgb(0x1d3a5c)).border_color(rgb(0x4e8fc8)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.composer.composer_focused = false;
                                this.roster.roster.focused = false;
                                cx.stop_propagation();
                                if this.overlays.active == Some(Overlay::Friends)
                                    && !this.overlays.closing
                                {
                                    this.dismiss_overlay(window, cx);
                                } else {
                                    this.social.social_detail_open = false;
                                    this.social.social_pane_transition = None;
                                    this.social.conversation_peer = None;
                                    this.social.conversation_input.clear();
                                    this.social.conversation_focused = false;
                                    this.overlays.active = Some(Overlay::Friends);
                                    this.overlays.closing = false;
                                    cx.notify();
                                }
                            }))
                            .child(
                                img("images/icons/friends.png")
                                    .size(px(28.0))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .right(px(-5.0))
                            .top(px(-8.0))
                            .min_w(px(19.0))
                            .h(px(18.0))
                            .px(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(0x4579b7))
                            .border_1()
                            .border_color(rgb(0x78b8ec))
                            .rounded(px(2.0))
                            .text_size(px(10.5))
                            .text_color(rgb(0xe6f9ff))
                            .child(online_friends.to_string()),
                    ),
            )
    }
}
