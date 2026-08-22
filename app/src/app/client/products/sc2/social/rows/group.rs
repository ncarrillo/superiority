use super::*;

impl SocialComponent {
    /// a section rule in the social list: a caret, the label, what the section
    /// is worth in one phrase, and a hairline running out to the edge.
    /// refinement N replaced the boxed, bordered header with this so the list
    /// reads as one column of people rather than a stack of cards.
    pub(in crate::app::client) fn section_header(
        &self,
        group: usize,
        title: &'static str,
        detail: Option<String>,
        whisper: bool,
        variant: ui_shared_modal::ModalVariant,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let skin = SocialSkin::for_variant(variant);
        let caret = if self.social_collapsed[group] {
            "\u{25b8}"
        } else {
            "\u{25be}"
        };
        let (caret_color, label_color, rule_color, wash) = if whisper {
            (
                rgb(skin.whisper),
                rgb(skin.whisper),
                rgba(skin.whisper_rule),
                rgba(skin.whisper_wash),
            )
        } else {
            (
                rgb(skin.accent),
                rgb(skin.accent),
                rgba(skin.structural),
                rgba(skin.hover),
            )
        };
        let mut header = div()
            .id(("social-group", group))
            .h(px(SOCIAL_SECTION_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .font_family(skin.interface_font)
            .gap(px(8.0))
            .px(px(SOCIAL_ROW_INSET))
            .cursor_pointer()
            .hover(move |style| style.bg(wash))
            .active(|style| style.opacity(0.64))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session.social.social_collapsed[group] =
                    !this.session.social.social_collapsed[group];
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
                    .text_color(rgb(skin.muted))
                    .child(detail),
            );
        }
        header
            .child(div().flex_1().h(px(1.0)).bg(rule_color))
            .into_any_element()
    }
}
