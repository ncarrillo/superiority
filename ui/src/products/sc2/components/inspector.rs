use gpui::{BoxShadow, Div, ElementId, SharedString, Stateful, div, prelude::*, px, rgb, rgba};

use crate::products::sc2::theme::{
    BORDER_FOCUSED, BORDER_STRUCTURAL, FONT_INTERFACE, MUTED, NOTICE, PANEL_BACKGROUND,
    PANEL_BORDER, focus_glow,
};

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

/// the 48px band that opens every inspector pane. compose it with
/// [`header_title`] and [`header_detail`] when a pane needs more than a plain
/// title and one trailing string.
#[must_use]
pub fn header_shell() -> Div {
    div()
        .h(px(48.0))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(9.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(rgb(PANEL_BORDER))
        .bg(rgb(PANEL_BACKGROUND))
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::BOLD)
        .text_size(px(14.5))
        .text_color(rgb(0x009f_b8cf))
}

#[must_use]
pub fn header_title(title: impl Into<SharedString>) -> Div {
    div().flex_shrink_0().child(title.into())
}

/// the trailing slot of a header band. children override the colour when a
/// pane needs more than one tone in there.
#[must_use]
pub fn header_detail() -> Div {
    div()
        .ml_auto()
        .pl(px(16.0))
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_right()
        .text_color(rgb(0x0042_b8eb))
}

#[must_use]
pub fn header(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> Div {
    header_shell()
        .child(header_title(title))
        .child(header_detail().child(detail.into()))
}

/// the search box that sits under a pane header. the pane keeps ownership of
/// the text input; this is only the frame it lives in.
#[must_use]
pub fn search_field(focused: bool) -> Div {
    div()
        .h(px(27.0))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(7.0))
        .px(px(8.0))
        .border_1()
        .border_color(if focused {
            rgb(BORDER_FOCUSED)
        } else {
            rgba(BORDER_STRUCTURAL)
        })
        .bg(rgba(0x030a_13e6))
        .when(focused, |field| field.shadow(focus_glow()))
}

/// one toggle in a filter chip strip. selected chips wear the tier-2 stroke;
/// the rest stay quiet enough to read as a legend.
#[must_use]
pub fn filter_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(18.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px(px(7.0))
        .border_1()
        .cursor_pointer()
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_size(px(10.0))
        .border_color(if selected {
            rgb(BORDER_FOCUSED)
        } else {
            rgba(BORDER_STRUCTURAL)
        })
        .bg(if selected {
            rgba(0x1231_5e99)
        } else {
            rgba(0x0000_0000)
        })
        .text_color(if selected { rgb(0xe6f9ff) } else { rgb(MUTED) })
        .when(!selected, |chip| {
            chip.hover(|style| {
                style
                    .border_color(rgba(0x2d88_ac99))
                    .text_color(rgb(0xa9b8cc))
            })
        })
        .active(|style| style.opacity(0.7))
        .child(label.into())
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

/// the record rail row. selection is the tier-2 treatment: a left accent bar,
/// a fill, and an inset glow — no per-row tint competing with it.
#[must_use]
pub fn rail_row(id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .flex_shrink_0()
        .relative()
        .cursor_pointer()
        .border_b_1()
        .border_color(rgba(0x133e_5b59))
        .when(selected, |row| {
            row.bg(rgba(0x1231_5e80))
                .shadow(vec![
                    BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f01f).into())
                        .blur_radius(px(12.0))
                        .inset(),
                ])
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(2.0))
                        .bg(rgb(NOTICE)),
                )
        })
        .when(!selected, |row| {
            row.hover(|style| style.bg(rgba(0x1231_5e4d)))
        })
        .active(|style| style.opacity(0.92))
}
