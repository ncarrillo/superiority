use super::*;

impl SocialComponent {
    /// a whisper in the list. unread reads as a purple wash, a one-line preview,
    /// and a count badge; once read the row keeps only the name and the time,
    /// because there is nothing left to act on.
    pub(super) fn whisper_row(
        &self,
        index: usize,
        peer: &str,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let user = self
            .friends
            .iter()
            .find(|friend| friend.name == peer)
            .map_or_else(
                || UiFriend::unknown(peer).roster_user(&chrome.ui_assets),
                |friend| friend.roster_user(&chrome.ui_assets),
            );
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

        let name_block = whisper_name_block(&user.name, &time, unread, preview);

        let mut row = div()
            .id(("social-whisper", index))
            .h(px(SOCIAL_WHISPER_ROW_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(SOCIAL_ROW_INSET))
            .cursor_pointer()
            .hover(|style| style.bg(rgba(0x5028_784d)))
            .active(|style| style.opacity(0.8))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.social
                    .open_conversation(conversation_peer.clone(), window, cx);
                cx.notify();
            }))
            .child(ui_roster::framed_portrait(
                user.portrait.as_ref(),
                &chrome.ui_assets,
                ui_roster::PORTRAIT_FRAME,
                ui_roster::PORTRAIT_FACE,
            ))
            .child(name_block);
        if unread > 0 {
            row = row.bg(rgba(0x5028_782e)).child(
                div()
                    .flex_shrink_0()
                    .min_w(px(16.0))
                    .h(px(16.0))
                    .px(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(rgb(WHISPER_ACCENT))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(9.5))
                    .text_color(rgb(0x001a_0a26))
                    .child(unread.min(99).to_string()),
            );
        }
        row.into_any_element()
    }
}

fn whisper_name_block(name: &str, time: &str, unread: usize, preview: Option<String>) -> Div {
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
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(12.5))
                    .text_color(if unread > 0 {
                        rgb(0x00e6_f9ff)
                    } else {
                        rgb(TEXT)
                    })
                    .child(name.to_owned()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(9.5))
                    .text_color(rgb(MUTED))
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
                .font_family(FONT_INTERNATIONAL)
                .text_size(px(11.0))
                .text_color(rgb(0x00d0_b3e8))
                .child(preview),
        );
    }
    block
}
