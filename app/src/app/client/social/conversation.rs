use super::*;

impl SocialComponent {
    pub(super) fn conversation_pane(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let peer = self
            .conversation_peer
            .clone()
            .unwrap_or_else(|| "Conversation".to_owned());
        let friend = self.friends.iter().find(|friend| friend.name == peer);
        let portrait = friend
            .and_then(|friend| friend.portrait.clone())
            .map_or_else(|| img("images/icons/friend-placeholder.png"), img);
        let presence = friend.map_or(PresenceState::Unknown, |friend| friend.presence);
        let presence = presence_kind(presence);
        let presence_copy = presence.label();
        let presence_icon = chrome.ui_assets.presence_icon(presence);
        let history = self.conversations.get(&peer).cloned().unwrap_or_default();
        let mut messages = Vec::new();
        if history.is_empty() {
            messages.push(
                div()
                    .h(px(34.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(rgb(0x7d8fa8))
                    .child(format!("No messages with {peer} yet."))
                    .into_any_element(),
            );
        } else {
            let mut last_speaker = None;
            for (index, line) in history.iter().enumerate() {
                let who = if line.outgoing { "You" } else { peer.as_str() };
                let speaker_changed = last_speaker != Some(line.outgoing);
                last_speaker = Some(line.outgoing);
                let (text, media) = ui_chat::split_media(&line.body);
                let mut message = div()
                    .id(("conversation-line", index))
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .pt(px(if speaker_changed { 12.0 } else { 2.0 }))
                    .pb(px(2.0));
                if line.outgoing {
                    message = message.items_end();
                } else {
                    message = message.items_start();
                }
                if speaker_changed {
                    message = message.child(
                        div()
                            .font_family(FONT_INTERFACE)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.5))
                            .text_color(if line.outgoing {
                                rgb(0x33a8f0)
                            } else {
                                rgb(0xb277e8)
                            })
                            .child(format!("{who}   {}", line.timestamp)),
                    );
                }
                if !text.is_empty() {
                    message = message.child(
                        div()
                            .max_w(px(330.0))
                            .font_family(FONT_INTERNATIONAL)
                            .text_size(px(13.0))
                            .line_height(px(18.0))
                            .text_color(rgb(0xd6e0f0))
                            .child(ui_chat::display_message_body(&text).0),
                    );
                }
                for link in media {
                    message = message.child(
                        img(link)
                            .max_w(px(240.0))
                            .max_h(px(240.0))
                            .object_fit(ObjectFit::Contain)
                            .rounded(px(2.0)),
                    );
                }
                messages.push(message.into_any_element());
            }
        }
        let input_color = if self.conversation_input.is_empty() {
            rgb(0x5e8291)
        } else {
            rgb(0xd6e0f0)
        };

        div()
            .absolute()
            .left(px(400.0))
            .top_0()
            .w(px(400.0))
            .h(px(SOCIAL_BODY_HEIGHT))
            .font_family(FONT_INTERFACE)
            .child(
                div()
                    .id("conversation-back")
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER - 4.0))
                    .top(px(8.0))
                    .w(px(110.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(13.0))
                    .text_color(rgb(0x76618c))
                    .hover(|style| style.text_color(rgb(0xffffff)))
                    .active(|style| style.opacity(0.64))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this
                            .social
                            .close_conversation(&this.focus_handle, window, cx)
                        {
                            cx.notify();
                        }
                    }))
                    .child("‹  Friends"),
            )
            .child(
                portrait
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER + 2.0))
                    .top(px(44.0))
                    .size(px(42.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                img("images/nine-patch/portraits/frame.png")
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER - 2.0))
                    .top(px(40.0))
                    .size(px(50.0))
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                div()
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER + 50.0))
                    .right(px(SOCIAL_CONVERSATION_GUTTER))
                    .top(px(43.0))
                    .h(px(22.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERNATIONAL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(rgb(0xd6e0f0))
                    .child(peer.clone()),
            )
            .child(
                img(presence_icon)
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER + 50.0))
                    .top(px(70.0))
                    .size(px(12.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                div()
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER + 68.0))
                    .right(px(SOCIAL_CONVERSATION_GUTTER))
                    .top(px(67.0))
                    .h(px(18.0))
                    .text_size(px(11.5))
                    .text_color(rgb(0x7d8fa8))
                    .child(presence_copy),
            )
            .child(
                div()
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER))
                    .right(px(SOCIAL_CONVERSATION_GUTTER))
                    .top(px(98.0))
                    .h(px(1.0))
                    .bg(rgb(0x174f78)),
            )
            .child(
                div()
                    .id("conversation-scroll")
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER))
                    .right(px(SOCIAL_CONVERSATION_GUTTER))
                    .top(px(104.0))
                    .bottom(px(72.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.conversation_scroll)
                    .children(messages),
            )
            .child(
                div()
                    .id("conversation-input")
                    .absolute()
                    .left(px(SOCIAL_CONVERSATION_GUTTER))
                    .right(px(SOCIAL_CONVERSATION_GUTTER))
                    .bottom(px(26.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .bg(rgba(0x030b13f7))
                    .border_1()
                    .border_color(if self.conversation_focused {
                        rgb(0x33a8f0)
                    } else {
                        rgb(0x174f78)
                    })
                    .rounded(px(2.0))
                    .cursor(gpui::CursorStyle::IBeam)
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(13.0))
                    .text_color(input_color)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.social.conversation_focused = true;
                        this.social.conversation_input.focus(window, cx);
                        cx.notify();
                    }))
                    .child(self.conversation_input.element()),
            )
    }
}
