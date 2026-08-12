use super::*;

impl SocialComponent {
    pub(super) fn group_header(
        &self,
        group: usize,
        title: &'static str,
        count: usize,
        whisper: bool,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let caret = if self.social_collapsed[group] {
            "▸"
        } else {
            "▾"
        };
        div()
            .id(("social-group", group))
            .h(px(26.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(if whisper {
                rgb(0x0f0915)
            } else {
                rgba(0x051326f0)
            })
            .border_1()
            .border_color(if whisper {
                rgb(0x3b264e)
            } else {
                rgb(0x174f78)
            })
            .rounded(px(2.0))
            .cursor_pointer()
            .hover(move |style| {
                style
                    .bg(if whisper {
                        rgba(0x28163ceb)
                    } else {
                        rgba(0x0b2b4be8)
                    })
                    .shadow_lg()
            })
            .active(|style| style.opacity(0.78))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.social.social_collapsed[group] = !this.social.social_collapsed[group];
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .ml(px(10.0))
                    .w(px(16.0))
                    .text_size(px(11.0))
                    .text_color(if whisper {
                        rgb(0x76618c)
                    } else {
                        rgb(0x7d8fa8)
                    })
                    .child(caret),
            )
            .child(
                div()
                    .ml(px(4.0))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.0))
                    .text_color(rgb(0xd6e0f0))
                    .child(title),
            )
            .child(
                div()
                    .ml_auto()
                    .mr(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(13.0))
                    .text_color(rgb(0xd6e0f0))
                    .child(format!("({count})")),
            )
            .into_any_element()
    }
}
