use super::*;

impl SocialComponent {
    pub(super) fn whisper_row(
        &self,
        index: usize,
        peer: &str,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let portrait = self
            .friends
            .iter()
            .find(|friend| friend.name == peer)
            .and_then(|friend| friend.portrait.as_ref())
            .map_or_else(
                || img("images/icons/friend-placeholder.png"),
                |portrait| img(portrait.clone()),
            );
        let summary = self
            .conversations
            .get(peer)
            .and_then(|lines| lines.last())
            .map_or_else(
                || "No messages".to_owned(),
                |line| format!("{}{}", if line.outgoing { "You: " } else { "" }, line.body),
            );
        let unread = self.whisper_unread.get(peer).copied().unwrap_or(0);
        let conversation_peer = peer.to_owned();
        let mut row = div()
            .id(("social-whisper", index))
            .relative()
            .h(px(52.0))
            .flex_shrink_0()
            .cursor_pointer()
            .hover(|style| {
                style
                    .bg(rgba(0x28163ceb))
                    .border_1()
                    .border_color(rgba(0x993ddbd9))
                    .shadow_lg()
            })
            .active(|style| style.opacity(0.8))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.social
                    .open_conversation(conversation_peer.clone(), window, cx);
                cx.notify();
            }))
            .child(
                portrait
                    .absolute()
                    .left(px(12.0))
                    .top(px(7.0))
                    .size(px(38.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                img("images/nine-patch/portraits/frame.png")
                    .absolute()
                    .left(px(8.0))
                    .top(px(3.0))
                    .size(px(46.0))
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                div()
                    .absolute()
                    .left(px(64.0))
                    .right(px(8.0))
                    .top(px(7.0))
                    .h(px(19.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERNATIONAL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.0))
                    .text_color(rgb(0xebd6fc))
                    .child(peer.to_owned()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(64.0))
                    .right(px(8.0))
                    .top(px(29.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(11.5))
                    .text_color(rgb(0x76618c))
                    .child(summary),
            );
        if unread > 0 {
            row = row.child(
                div()
                    .absolute()
                    .left(px(4.0))
                    .top(px(1.0))
                    .w(px(20.0))
                    .h(px(17.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x1e0b33))
                    .border_1()
                    .border_color(rgb(0x3b264e))
                    .rounded(px(2.0))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(10.5))
                    .text_color(rgb(0xebd6fc))
                    .child(unread.min(99).to_string()),
            );
        }
        row.into_any_element()
    }
}
