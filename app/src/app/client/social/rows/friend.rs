use super::*;

impl SocialComponent {
    pub(super) fn friend_row(
        &self,
        row_id: usize,
        friend: &UiFriend,
        dimmed: bool,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let peer = friend.name.clone();
        let portrait = friend.portrait.as_ref().map_or_else(
            || img("images/icons/friend-placeholder.png"),
            |portrait| img(portrait.clone()),
        );
        let presence = presence_kind(friend.presence);
        let icon = chrome.ui_assets.presence_icon(presence);
        let mut row = div()
            .id(("social-friend", row_id))
            .relative()
            .h(px(52.0))
            .flex_shrink_0()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(0x12315eb8)))
            .active(|style| style.bg(rgba(0x1a4778e8)).opacity(0.82))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.social.open_conversation(peer.clone(), window, cx);
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
                    .text_color(rgb(0xd6e0f0))
                    .child(friend.name.clone()),
            )
            .child(
                ui_roster::presence_line(
                    icon,
                    presence.label(),
                    12.0,
                    6.0,
                    11.5,
                    rgb(0x7d8fa8).into(),
                )
                .absolute()
                .left(px(64.0))
                .right(px(8.0))
                .top(px(29.0))
                .h(px(18.0)),
            );
        if dimmed {
            row = row.opacity(0.46);
        }
        row.into_any_element()
    }
}
