use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Div, ElementId, FontWeight, ImageSource, ObjectFit,
    SharedString, Stateful, div, ease_in_out, img, prelude::*, px, rgb, rgba,
};

use crate::theme::FONT_NAVIGATION;

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
        .absolute()
        .left(px(18.0))
        .top(px(10.0))
        .size(px(22.0))
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
        .font_family(crate::theme::FONT_INTERFACE)
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
        .bg(rgba(0x0106_0dfc))
        .border_2()
        .border_color(rgb(0x0033_a8f0))
        .rounded(px(1.0))
        .shadow_lg()
        .child(
            img(fill)
                .absolute()
                .left(px(3.0))
                .top(px(3.0))
                .w(px((width - 6.0).max(0.0)))
                .h(px((height - 6.0).max(0.0)))
                .opacity(0.82)
                .object_fit(ObjectFit::Fill),
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
