use super::*;

/// bubble geometry. a whisper is short, so the bubble is sized to the words
/// rather than to the panel.
const BUBBLE_MAX_WIDTH: f32 = 258.0;
/// space above a message that starts a new group; consecutive messages from one
/// sender sit tight underneath instead.
const GROUP_GAP: f32 = 10.0;
const STACK_GAP: f32 = 3.0;

impl SocialComponent {
    /// the identity of the person you are whispering, drawn where the SOCIAL
    /// title sits when the list is showing. it replaces that title rather than
    /// stacking under it, so the conversation never carries two headers.
    pub(super) fn conversation_header(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let peer = self
            .conversation_peer
            .clone()
            .unwrap_or_else(|| "Conversation".to_owned());
        let friend = self.friends.iter().find(|friend| friend.name == peer);
        let user = friend.map_or_else(
            || UiFriend::unknown(&peer).roster_user(&chrome.ui_assets),
            |friend| friend.roster_user(&chrome.ui_assets),
        );
        let dot_color = user.presence.dot_color();
        let mut dot = div()
            .flex_shrink_0()
            .size(px(9.0))
            .rounded(px(4.5))
            .bg(rgb(dot_color));
        if user.presence.dot_glows() {
            dot = dot.shadow(vec![
                gpui::BoxShadow::new(px(0.0), px(0.0), rgba((dot_color << 8) | 0xcc).into())
                    .blur_radius(px(5.0)),
            ]);
        }
        div()
            .absolute()
            .left(px(26.0))
            .right(px(26.0))
            .top(px(40.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .font_family(FONT_INTERFACE)
            .child(
                div()
                    .id("conversation-back")
                    .w(px(18.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .cursor_pointer()
                    .text_size(px(15.0))
                    .text_color(rgb(NOTICE))
                    .hover(|style| style.text_color(rgb(0x00e6_f9ff)))
                    .active(|style| style.opacity(0.64))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this
                            .social
                            .close_conversation(&this.focus_handle, window, cx)
                        {
                            cx.notify();
                        }
                    }))
                    .child("\u{2039}"),
            )
            .child(ui_roster::framed_portrait(
                user.portrait.as_ref(),
                &chrome.ui_assets,
                ui_roster::PORTRAIT_FRAME,
                ui_roster::PORTRAIT_FACE,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(13.0))
                    .text_color(rgb(0x00e6_f9ff))
                    .child(peer),
            )
            .child(dot)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(-6.0))
                    .h(px(1.0))
                    .bg(rgba(BORDER_STRUCTURAL)),
            )
    }

    pub(super) fn conversation_pane(
        &self,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let peer = self
            .conversation_peer
            .clone()
            .unwrap_or_else(|| "Conversation".to_owned());
        let history = self.conversations.get(&peer).cloned().unwrap_or_default();
        let mut messages = Vec::new();
        if history.is_empty() {
            messages.push(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .py(px(10.0))
                    .text_size(px(12.0))
                    .text_color(rgb(MUTED))
                    .child(format!("No messages with {peer} yet."))
                    .into_any_element(),
            );
        } else {
            let mut last_marker: Option<&str> = None;
            let mut last_speaker = None;
            for (index, line) in history.iter().enumerate() {
                // the time marker is the only thing separating one exchange
                // from the next; the sides already say who is speaking.
                let marked = last_marker != Some(line.timestamp.as_str());
                if marked {
                    last_marker = Some(line.timestamp.as_str());
                    messages.push(time_marker(&line.timestamp));
                }
                let started_group = marked || last_speaker != Some(line.outgoing);
                last_speaker = Some(line.outgoing);
                messages.push(bubble(index, line, started_group));
            }
        }
        let input_color = if self.conversation_input.is_empty() {
            rgb(0x005e_8291)
        } else {
            rgb(TEXT)
        };

        let messages = div()
            .id("conversation-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.conversation_scroll)
            .child(
                // the thread hangs off the bottom of the viewport so a short
                // conversation sits against the input rather than floating in
                // the middle of an empty panel.
                div()
                    .w_full()
                    .min_h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .children(messages),
            );
        let conversation = div()
            .id("conversation-viewport")
            .absolute()
            .left(px(SOCIAL_CONVERSATION_GUTTER))
            .right(px(SOCIAL_CONVERSATION_GUTTER))
            .top(px(6.0))
            .bottom(px(72.0))
            .child(messages)
            .vertical_scrollbar_for(&self.conversation_scroll, window, cx);

        div()
            .absolute()
            .left(px(400.0))
            .top_0()
            .w(px(400.0))
            .h(px(SOCIAL_BODY_HEIGHT))
            .font_family(FONT_INTERFACE)
            .child(conversation)
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
                    .bg(rgba(0x030b_13f7))
                    .border_1()
                    .border_color(if self.conversation_focused {
                        rgb(BORDER_FOCUSED)
                    } else {
                        rgba(BORDER_STRUCTURAL)
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

/// one message. yours sit right in channel blue, theirs left in whisper purple —
/// which is what removes the need for a name on every line.
fn bubble(index: usize, line: &ConversationLine, started_group: bool) -> AnyElement {
    let (text, media) = ui_chat::split_media(&line.body);
    let (fill, border, ink) = if line.outgoing {
        (rgba(0x1231_5e8c), rgba(0x33a8_f059), rgb(0x00e6_f9ff))
    } else {
        (rgba(0x5028_784d), rgba(0xc084_e859), rgb(0x00e8_ddf2))
    };
    let mut group = div()
        .id(("conversation-line", index))
        .w_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(STACK_GAP))
        .pt(px(if started_group { GROUP_GAP } else { STACK_GAP }));
    group = if line.outgoing {
        group.items_end()
    } else {
        group.items_start()
    };
    if !text.is_empty() {
        group = group.child(
            div()
                .max_w(px(BUBBLE_MAX_WIDTH))
                .px(px(11.0))
                .py(px(7.0))
                .bg(fill)
                .border_1()
                .border_color(border)
                .font_family(FONT_INTERNATIONAL)
                .text_size(px(12.5))
                .line_height(px(18.0))
                .text_color(ink)
                .child(ui_chat::display_message_body(&text).0),
        );
    }
    for link in media {
        group = group.child(
            img(link)
                .max_w(px(BUBBLE_MAX_WIDTH))
                .max_h(px(240.0))
                .object_fit(ObjectFit::Contain)
                .rounded(px(2.0)),
        );
    }
    group.into_any_element()
}

fn time_marker(time: &str) -> AnyElement {
    div()
        .w_full()
        .flex_shrink_0()
        .flex()
        .justify_center()
        .pt(px(GROUP_GAP))
        .text_size(px(9.5))
        .text_color(rgb(MUTED))
        .child(time.to_owned())
        .into_any_element()
}
