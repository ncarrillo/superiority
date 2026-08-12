use gpui::{Div, ElementId, Rgba, SharedString, Stateful, div, prelude::*, px, rgb, rgba};

use crate::theme::{FONT_INTERFACE, PANEL_BACKGROUND, PANEL_BORDER};

#[must_use]
pub fn toolbar_icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: impl IntoElement,
    active: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(30.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(7.0))
        .rounded(px(3.0))
        .border_1()
        .border_color(if active { rgb(0x2d88ac) } else { rgb(0x17384d) })
        .bg(if active {
            rgba(0x164f_7870)
        } else {
            rgba(0x0713_1c90)
        })
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_size(px(12.0))
        .text_color(if active { rgb(0xd6efff) } else { rgb(0x8aa2b5) })
        .cursor_pointer()
        .hover(|style| {
            style
                .border_color(rgb(0x3aa8d4))
                .bg(rgba(0x164f_7850))
                .text_color(rgb(0xd6efff))
        })
        .active(|style| style.bg(rgba(0x164f_78a0)).opacity(0.8))
        .child(icon)
        .child(label.into())
}

#[must_use]
pub fn pane() -> Div {
    div()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(0x0004_0b12))
        .border_color(rgb(PANEL_BORDER))
}

#[must_use]
pub fn focus_outline() -> Div {
    div()
        .absolute()
        .inset_0()
        .border_1()
        .border_color(rgba(0x39ba_ffb8))
}

#[must_use]
pub fn header(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> Div {
    div()
        .h(px(48.0))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .px(px(16.0))
        .border_b_1()
        .border_color(rgb(PANEL_BORDER))
        .bg(rgb(PANEL_BACKGROUND))
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::BOLD)
        .text_size(px(14.5))
        .text_color(rgb(0x009f_b8cf))
        .child(div().flex_shrink_0().child(title.into()))
        .child(
            div()
                .ml_auto()
                .pl(px(16.0))
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_right()
                .text_color(rgb(0x0042_b8eb))
                .child(detail.into()),
        )
}

#[must_use]
pub fn selectable_row(id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .flex_shrink_0()
        .relative()
        .cursor_pointer()
        .border_b_1()
        .border_color(rgb(0x0010_2a3b))
        .when(selected, |row| {
            row.border_color(rgb(0x17384d)).child(
                div()
                    .absolute()
                    .left(px(4.0))
                    .right(px(4.0))
                    .top(px(3.0))
                    .bottom(px(3.0))
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(rgba(0x39baff8c))
                    .bg(rgba(0x105f_8a78)),
            )
        })
        .when(!selected, |row| {
            row.hover(|style| style.bg(rgba(0x164f_7848)))
        })
        .active(|style| style.bg(rgba(0x1a79_a5a0)).opacity(0.92))
}

#[must_use]
pub fn tinted_selectable_row(
    id: impl Into<ElementId>,
    selected: bool,
    tint: Rgba,
) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .flex_shrink_0()
        .relative()
        .cursor_pointer()
        .border_b_1()
        .border_color(tint.alpha(0.2))
        .bg(tint.alpha(if selected { 0.12 } else { 0.035 }))
        .when(selected, |row| {
            row.border_color(tint.alpha(0.55)).child(
                div()
                    .absolute()
                    .left(px(4.0))
                    .right(px(4.0))
                    .top(px(3.0))
                    .bottom(px(3.0))
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(tint.alpha(0.88))
                    .bg(tint.alpha(0.14)),
            )
        })
        .when(!selected, |row| {
            row.hover(move |style| style.border_color(tint.alpha(0.38)).bg(tint.alpha(0.09)))
        })
        .active(move |style| style.bg(tint.alpha(0.17)).opacity(0.95))
}
