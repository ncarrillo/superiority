use super::*;

impl SocialComponent {
    /// a whisper in the list. unread reads as a purple wash, a one-line preview,
    /// and a count badge; once read the row keeps only the name and the time,
    /// because there is nothing left to act on.
    pub(in crate::app::client) fn whisper_row(
        &self,
        index: usize,
        peer: &str,
        variant: ui_shared_modal::ModalVariant,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let friend = self
            .friends
            .iter()
            .find(|friend| friend.name == peer)
            .map_or_else(|| UiFriend::unknown(peer), Clone::clone);
        let skin = SocialSkin::for_variant(variant);
        let last = self.conversations.get(peer).and_then(|lines| lines.last());
        let time = last.map(|line| line.timestamp.clone()).unwrap_or_default();
        let preview = last.map(|line| {
            let (text, media) = ui_chat::split_media(&line.body);
            let body = if text.trim().is_empty() && !media.is_empty() {
                "Sent an image".to_owned()
            } else {
                text
            };
            format!("{}{body}", if line.outgoing { "You: " } else { "" })
        });
        let unread = self.whisper_unread.get(peer).copied().unwrap_or(0);
        let conversation_peer = peer.to_owned();

        let name_block = whisper_name_block(&friend.name, &time, unread, preview, skin);

        let mut row = div()
            .id(("social-whisper", index))
            .h(px(SOCIAL_WHISPER_ROW_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(SOCIAL_ROW_INSET))
            .cursor_pointer()
            .hover(move |style| style.bg(rgba(skin.whisper_wash)))
            .active(|style| style.opacity(0.8))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.session
                    .social
                    .open_conversation(conversation_peer.clone(), window, cx);
                cx.notify();
            }))
            .child(social_portrait(&friend, variant, chrome))
            .child(name_block);
        if unread > 0 {
            row = row.bg(rgba(skin.whisper_wash)).child(
                div()
                    .flex_shrink_0()
                    .min_w(px(16.0))
                    .h(px(16.0))
                    .px(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(rgb(skin.whisper))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(9.5))
                    .text_color(rgb(skin.text))
                    .child(unread.min(99).to_string()),
            );
        }
        row.into_any_element()
    }
}

fn whisper_name_block(
    name: &str,
    time: &str,
    unread: usize,
    preview: Option<String>,
    skin: SocialSkin,
) -> Div {
    let mut block = div().flex_1().min_w_0().flex().flex_col().child(
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(skin.body_font)
                    .text_size(px(12.5))
                    .text_color(if unread > 0 {
                        rgb(skin.bright)
                    } else {
                        rgb(skin.text)
                    })
                    .child(name.to_owned()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(9.5))
                    .text_color(rgb(skin.muted))
                    .child(time.to_owned()),
            ),
    );
    if unread > 0
        && let Some(preview) = preview
    {
        block = block.child(
            div()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(skin.body_font)
                .text_size(px(11.0))
                .text_color(rgb(skin.whisper))
                .child(preview),
        );
    }
    block
}
