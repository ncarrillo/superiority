use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, px, rgb, rgba};

use crate::theme::{FONT_INTERFACE, PANEL_BACKGROUND, PANEL_BORDER};

#[must_use]
pub fn toolbar_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(32.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if active {
            rgb(0x0033_a8f0)
        } else {
            rgb(PANEL_BORDER)
        })
        .bg(if active {
            rgba(0x164f_7850)
        } else {
            rgba(0x0713_1cdd)
        })
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_size(px(11.0))
        .text_color(if active {
            rgb(0x00d6_e0f0)
        } else {
            rgb(0x0078_91a6)
        })
        .cursor_pointer()
        .hover(|style| {
            style
                .border_color(rgb(0x0033_a8f0))
                .text_color(rgb(0x00d6_e0f0))
        })
        .active(|style| style.opacity(0.72))
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
pub fn header(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> Div {
    div()
        .h(px(42.0))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .px(px(14.0))
        .border_b_1()
        .border_color(rgb(PANEL_BORDER))
        .bg(rgb(PANEL_BACKGROUND))
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::BOLD)
        .text_size(px(12.5))
        .text_color(rgb(0x009f_b8cf))
        .child(title.into())
        .child(
            div()
                .ml_auto()
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
            row.bg(rgba(0x164f_7845)).child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.0))
                    .bg(rgb(0x0039_baff)),
            )
        })
        .hover(|style| style.bg(rgba(0x164f_782b)))
        .active(|style| style.bg(rgba(0x164f_7860)).opacity(0.84))
}
