use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Div, ElementId, FontWeight, ImageSource, ObjectFit,
    SharedString, Stateful, div, ease_in_out, img, prelude::*, px, rgb, rgba,
};

use crate::products::sc2::theme::FONT_NAVIGATION;

const ACTION_BUTTON_ART_BLEED: f32 = 6.0;

#[derive(Clone)]
pub struct ActionButtonImages {
    pub idle: ImageSource,
    pub active: ImageSource,
}

#[must_use]
pub fn action_button(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    width: f32,
    height: f32,
    warning: bool,
    images: ActionButtonImages,
) -> Stateful<Div> {
    let id = id.into();
    div()
        .group(id.clone())
        .id(id.clone())
        .relative()
        .w(px(width))
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .font_family(FONT_NAVIGATION)
        .font_weight(FontWeight::BOLD)
        .text_size(px(if warning { 12.0 } else { 10.5 }))
        .text_color(if warning {
            rgb(0x00ff_e8ab)
        } else {
            rgb(0x00d6_e0f0)
        })
        .hover(gpui::Styled::shadow_lg)
        .child(button_image(images.idle, height).opacity(1.0))
        .child(
            button_image(images.active, height)
                .opacity(0.0)
                .group_hover(id.clone(), |style| style.opacity(1.0))
                .group_active(id, |style| style.opacity(1.0)),
        )
        .child(div().relative().child(title.into()))
}

/// the same frame with nothing behind it: the art sits back, the label goes
/// muted, and there is no hover swap to promise a press that would do nothing.
#[must_use]
pub fn disabled_action_button(
    title: impl Into<SharedString>,
    width: f32,
    height: f32,
    idle: ImageSource,
) -> Div {
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_NAVIGATION)
        .font_weight(FontWeight::BOLD)
        .text_size(px(10.5))
        .text_color(rgb(0x0055_6272))
        .child(button_image(idle, height).opacity(0.35))
        .child(div().relative().child(title.into()))
}

fn button_image(source: ImageSource, height: f32) -> gpui::Img {
    img(source)
        .absolute()
        .left_0()
        .top(px(-ACTION_BUTTON_ART_BLEED))
        .w_full()
        .h(px(height + ACTION_BUTTON_ART_BLEED * 2.0))
        .object_fit(ObjectFit::Fill)
}

#[must_use]
pub fn checkbox(id: impl Into<ElementId>, amount: f32, mark: ImageSource) -> Stateful<Div> {
    let amount = amount.clamp(0.0, 1.0);
    let mark_size = 14.0 + amount * 6.0;
    div()
        .id(id)
        .size(px(22.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(rgb(0x0001_060d))
        .border(px(1.5))
        .border_color(rgb(0x001f_7bd6))
        .rounded(px(2.5))
        .cursor_pointer()
        .hover(|style| {
            style
                .border(px(1.5))
                .border_color(rgb(0x0033_a8f0))
                .shadow_lg()
        })
        .active(|style| style.opacity(0.76))
        .child(
            img(mark)
                .size(px(mark_size))
                .opacity(amount)
                .object_fit(ObjectFit::Contain),
        )
}

#[must_use]
pub fn close_button(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .font_family(crate::products::sc2::theme::FONT_INTERFACE)
        .font_weight(FontWeight::BOLD)
        .text_size(px(18.0))
        .text_color(rgb(0x00d6_e0f0))
        .cursor_pointer()
        .hover(|style| style.text_color(rgb(0x00ff_ffff)))
        .active(|style| style.opacity(0.64))
        .child("×")
}

#[must_use]
pub fn tooltip_shell(width: f32, height: f32, fill: ImageSource) -> Div {
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .rounded(px(1.0))
        .child(tooltip_frame(fill))
}

/// the frame every floating surface in the app wears: a dark fill, the SC2
/// tooltip texture stretched over it, and one thin blue rule inside the edge.
///
/// it sizes itself to whatever it is dropped into, so a surface whose height is
/// its content — the /join popup — wears the same frame as the fixed-size
/// tooltips without repeating the numbers.
#[must_use]
pub fn tooltip_frame(fill: ImageSource) -> Div {
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .rounded(px(1.0))
        .child(div().absolute().inset(px(3.0)).bg(rgba(0x0106_0dfc)))
        .child(
            div().absolute().inset(px(3.0)).overflow_hidden().child(
                img(fill)
                    .size_full()
                    .opacity(0.82)
                    .object_fit(ObjectFit::Fill),
            ),
        )
        .child(
            div()
                .absolute()
                .inset(px(4.0))
                .border_1()
                .border_color(rgba(0x144c_75e6)),
        )
}

#[must_use]
pub fn animated_tooltip(
    tooltip: Div,
    animation_id: &'static str,
    left: f32,
    top: f32,
    from_x: f32,
) -> AnyElement {
    tooltip
        .with_animation(
            animation_id,
            Animation::new(Duration::from_millis(140)),
            move |tooltip, delta| {
                let travel = ease_in_out(delta);
                let opacity = 0.12 + (delta / (120.0 / 140.0)).min(1.0) * 0.88;
                tooltip
                    .left(px(left + from_x * (1.0 - travel)))
                    .top(px(top + 3.0 * (1.0 - travel)))
                    .opacity(opacity)
            },
        )
        .into_any_element()
}
