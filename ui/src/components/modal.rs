use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Div, FontWeight, ImageSource, ObjectFit,
    SharedString, Stateful, div, ease_in_out, img, prelude::*, px, rgb, rgba,
};

use crate::{UiAssets, theme::FONT_NAVIGATION};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderLayout {
    pub band: (f32, f32, f32, f32),
    pub title_x: f32,
    pub title_width: f32,
    pub font_size: f32,
}

impl HeaderLayout {
    #[must_use]
    pub const fn new(
        band: (f32, f32, f32, f32),
        title_x: f32,
        title_width: f32,
        font_size: f32,
    ) -> Self {
        Self {
            band,
            title_x,
            title_width,
            font_size,
        }
    }
}

const HEADER_TITLE_TOP: f32 = 35.0;
const HEADER_TITLE_HEIGHT: f32 = 38.0;

#[must_use]
pub fn chrome(width: f32, height: f32, frame: Option<ImageSource>, assets: &UiAssets) -> Div {
    let along_y = height.min(436.0);
    let inset_y = (height - along_y) / 2.0;
    let along_x = width.min(436.0);
    let inset_x = (width - along_x) / 2.0;
    let hex_height = (width * 264.0 / 700.0).min(height);
    let mut chrome = div()
        .absolute()
        .top_0()
        .left_0()
        .w(px(width))
        .h(px(height))
        .bg(rgba(0x0107_0ffb))
        .shadow_lg()
        .child(
            img(assets.modal_hex.clone())
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(px(hex_height))
                .opacity(0.55)
                .object_fit(ObjectFit::Fill),
        )
        .child(
            img(assets.modal_glow_left.clone())
                .absolute()
                .left(px(-168.0))
                .top(px(inset_y))
                .w(px(168.0))
                .h(px(along_y))
                .opacity(0.68)
                .object_fit(ObjectFit::Fill),
        )
        .child(
            img(assets.modal_glow_right.clone())
                .absolute()
                .left(px(width))
                .top(px(inset_y))
                .w(px(168.0))
                .h(px(along_y))
                .opacity(0.68)
                .object_fit(ObjectFit::Fill),
        )
        .child(
            img(assets.modal_glow_top.clone())
                .absolute()
                .left(px(inset_x))
                .top(px(-168.0))
                .w(px(along_x))
                .h(px(168.0))
                .opacity(0.68)
                .object_fit(ObjectFit::Fill),
        )
        .child(
            img(assets.modal_glow_bottom.clone())
                .absolute()
                .left(px(inset_x))
                .top(px(height))
                .w(px(along_x))
                .h(px(168.0))
                .opacity(0.68)
                .object_fit(ObjectFit::Fill),
        );
    if let Some(frame) = frame {
        chrome = chrome.child(
            img(frame)
                .absolute()
                .top_0()
                .left_0()
                .w(px(width))
                .h(px(height))
                .object_fit(ObjectFit::Fill),
        );
    } else {
        chrome = chrome.border_2().border_color(rgb(0x001f_6ca3));
    }
    chrome
}

#[must_use]
pub fn header(layout: HeaderLayout, title: impl Into<SharedString>, assets: &UiAssets) -> Div {
    let (band_x, band_y, band_width, band_height) = layout.band;
    div()
        .absolute()
        .inset_0()
        .child(
            img(assets.modal_title_band.clone())
                .absolute()
                .left(px(band_x))
                .top(px(band_y))
                .w(px(band_width))
                .h(px(band_height))
                .opacity(0.7)
                .object_fit(ObjectFit::Fill),
        )
        .child(title_element(
            layout.title_x,
            layout.title_width,
            layout.font_size,
            rgb(0x00e6_f9ff),
            title.into(),
        ))
}

#[must_use]
pub fn warning_header(width: f32, title: impl Into<SharedString>) -> Div {
    title_element(80.0, width - 160.0, 26.0, rgb(0x00ff_fae8), title.into())
}

fn title_element(
    left: f32,
    width: f32,
    font_size: f32,
    color: impl Into<gpui::Hsla>,
    title: SharedString,
) -> Div {
    div()
        .absolute()
        .left(px(left))
        .top(px(HEADER_TITLE_TOP))
        .w(px(width))
        .h(px(HEADER_TITLE_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_NAVIGATION)
        .font_weight(FontWeight::BOLD)
        .text_size(px(font_size))
        .text_color(color.into())
        .child(title)
}

#[must_use]
pub fn scan(
    closing: bool,
    warning: bool,
    width: f32,
    height: f32,
    open_id: &'static str,
    close_id: &'static str,
) -> AnyElement {
    let glow = div()
        .absolute()
        .border_2()
        .border_color(if warning {
            rgba(0xff7a_0ee6)
        } else {
            rgb(0x0033_a8f0)
        })
        .bg(if warning {
            rgba(0xd157_032e)
        } else {
            rgba(0x006b_d12e)
        })
        .rounded(px(3.0));
    if closing {
        glow.with_animation(
            close_id,
            Animation::new(Duration::from_millis(260)),
            move |glow, delta| {
                let time_ms = delta * 260.0;
                let (glow_width, glow_height, opacity) = if time_ms <= 160.0 {
                    let stage = ease_in_out(stage(time_ms, 0.0, 160.0));
                    (
                        width + 16.0 + 4.0 * stage,
                        height + 16.0 - (height + 14.0) * stage,
                        stage,
                    )
                } else {
                    let stage = ease_in_out(stage(time_ms, 160.0, 100.0));
                    (width + 20.0 - (width - 8.0) * stage, 2.0, 1.0 - stage)
                };
                glow.left(px((width - glow_width) / 2.0))
                    .top(px((height - glow_height) / 2.0))
                    .w(px(glow_width))
                    .h(px(glow_height))
                    .opacity(opacity)
            },
        )
        .into_any_element()
    } else {
        glow.with_animation(
            open_id,
            Animation::new(Duration::from_millis(450)),
            move |glow, delta| {
                let time_ms = delta * 450.0;
                let (glow_width, glow_height, opacity) = if time_ms <= 50.0 {
                    (28.0, 2.0, stage(time_ms, 0.0, 50.0))
                } else if time_ms <= 210.0 {
                    let stage = ease_in_out(stage(time_ms, 50.0, 160.0));
                    (
                        28.0 + (width - 12.0) * stage,
                        2.0 + (height + 14.0) * stage,
                        1.0,
                    )
                } else {
                    let stage = ease_in_out(stage(time_ms, 210.0, 240.0));
                    (width + 16.0, height + 16.0, 1.0 - stage)
                };
                glow.left(px((width - glow_width) / 2.0))
                    .top(px((height - glow_height) / 2.0))
                    .w(px(glow_width))
                    .h(px(glow_height))
                    .opacity(opacity)
            },
        )
        .into_any_element()
    }
}

#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn animated(
    modal: Stateful<Div>,
    closing: bool,
    warning: bool,
    width: f32,
    height: f32,
    panel_open_id: &'static str,
    panel_close_id: &'static str,
    scan_open_id: &'static str,
    scan_close_id: &'static str,
) -> Div {
    let panel = if closing {
        modal
            .with_animation(
                panel_close_id,
                Animation::new(Duration::from_millis(260)),
                |modal, delta| {
                    let stage = ease_in_out(stage(delta * 260.0, 0.0, 160.0));
                    modal.opacity(1.0 - stage)
                },
            )
            .into_any_element()
    } else {
        modal
            .with_animation(
                panel_open_id,
                Animation::new(Duration::from_millis(450)),
                |modal, delta| {
                    let time_ms = delta * 450.0;
                    let opacity = if time_ms <= 50.0 {
                        0.0
                    } else if time_ms <= 210.0 {
                        0.18 * ease_in_out(stage(time_ms, 50.0, 160.0))
                    } else {
                        reveal_opacity(stage(time_ms, 210.0, 240.0))
                    };
                    modal.opacity(opacity)
                },
            )
            .into_any_element()
    };
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .child(panel)
        .child(scan(
            closing,
            warning,
            width,
            height,
            scan_open_id,
            scan_close_id,
        ))
}

#[must_use]
pub fn dimmer(closing: bool) -> AnyElement {
    let dimmer = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(rgba(0x0003_0552));
    if closing {
        dimmer
            .with_animation(
                "settings-dimmer-close",
                Animation::new(Duration::from_millis(140)).with_easing(ease_in_out),
                |dimmer, delta| dimmer.opacity(1.0 - delta),
            )
            .into_any_element()
    } else {
        dimmer
            .with_animation(
                "settings-dimmer-open",
                Animation::new(Duration::from_millis(160)).with_easing(ease_in_out),
                gpui::Styled::opacity,
            )
            .into_any_element()
    }
}

fn stage(time_ms: f32, start_ms: f32, duration_ms: f32) -> f32 {
    ((time_ms - start_ms) / duration_ms).clamp(0.0, 1.0)
}

fn reveal_opacity(delta: f32) -> f32 {
    const STOPS: &[(f32, f32)] = &[(0.00, 0.18), (0.10, 0.74), (0.17, 0.36), (1.00, 1.00)];
    for points in STOPS.windows(2) {
        let [(start_at, start_alpha), (end_at, end_alpha)] = points else {
            continue;
        };
        if delta <= *end_at {
            let local = ((delta - start_at) / (end_at - start_at)).clamp(0.0, 1.0);
            return start_alpha + (end_alpha - start_alpha) * ease_in_out(local);
        }
    }
    1.0
}
