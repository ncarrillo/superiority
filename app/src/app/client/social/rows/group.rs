use super::*;

impl SocialComponent {
    /// a section rule in the social list: a caret, the label, what the section
    /// is worth in one phrase, and a hairline running out to the edge.
    /// refinement N replaced the boxed, bordered header with this so the list
    /// reads as one column of people rather than a stack of cards.
    pub(super) fn section_header(
        &self,
        group: usize,
        title: &'static str,
        detail: Option<String>,
        whisper: bool,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let caret = if self.social_collapsed[group] {
            "\u{25b8}"
        } else {
            "\u{25be}"
        };
        let (caret_color, label_color, rule_color, wash) = if whisper {
            (
                rgb(WHISPER_ACCENT),
                rgb(WHISPER_ACCENT),
                rgba(0x783c_a066),
                rgba(0x5028_7833),
            )
        } else {
            (
                rgb(NOTICE),
                rgb(0x00a9_b8cc),
                rgba(0x133e_5b80),
                rgba(0x1231_5e33),
            )
        };
        let mut header = div()
            .id(("social-group", group))
            .h(px(SOCIAL_SECTION_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(SOCIAL_ROW_INSET))
            .cursor_pointer()
            .hover(move |style| style.bg(wash))
            .active(|style| style.opacity(0.64))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.social.social_collapsed[group] = !this.social.social_collapsed[group];
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(9.0))
                    .text_color(caret_color)
                    .child(caret),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(10.0))
                    .text_color(label_color)
                    .child(title),
            );
        if let Some(detail) = detail {
            header = header.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(10.0))
                    .text_color(rgb(MUTED))
                    .child(detail),
            );
        }
        header
            .child(div().flex_1().h(px(1.0)).bg(rule_color))
            .into_any_element()
    }
}
