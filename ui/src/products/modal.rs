//! The shared modal frame — one shell, dressed per realm.
//!
//! `Canvas.dc.html`'s crisp rebuild of the dialog chrome: a filled metal
//! band with hard 1px rims and the bloom layered *under* the vector so the
//! edge itself never blurs. Three dressings: StarCraft II's notched plate
//! with machined tab inserts and energy fields, Remastered's double-ruled
//! redline over a pure black plate, and Reforged's stone band with gold
//! corner studs. This shell will eventually replace the nine-patch legacy
//! modal in `sc2::components::modal`; it deliberately leaves it untouched —
//! the one thing it borrows is the legacy scan-beam reveal, which is the
//! StarCraft II open the doc's holo-projector motion was describing anyway.
//!
//! What gpui cannot say here: the title's text-shadow glow (no text
//! shadows), and the doc's monospace voice for Remastered (no mono face is
//! bundled; the shipped product speaks Eurostile, so this does too).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, Background, BoxShadow, Div, FontWeight, ImageSource, ObjectFit,
    Path, PathBuilder, Pixels, Point, RenderImage, Stateful, canvas, div, ease_in_out, img,
    linear_color_stop, linear_gradient, point, prelude::*, px, rgb, rgba,
};
use image::{Frame, Rgba, RgbaImage};

use crate::foundation::animation::cubic_bezier;
use crate::foundation::scrollbar::ScrollbarColors;
use crate::products::sc2::components::modal as sc2_modal;
use crate::products::sc2::theme::FONT_NAVIGATION;
use crate::products::scr::theme::FONT_INTERFACE as SCR_FONT;

/// Reforged speaks in Friz Quadrata, its own UI face, embedded from the client.
const WC3_FONT: &str = "Friz Quadrata TT";

/// Which realm the shell is dressed as.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalVariant {
    Sc2,
    Remastered,
    Reforged,
}

impl ModalVariant {
    pub const ALL: [Self; 3] = [Self::Sc2, Self::Remastered, Self::Reforged];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sc2 => "SC2",
            Self::Remastered => "SC:R",
            Self::Reforged => "WC3:R",
        }
    }

    /// The realm's scrollbar: StarCraft II's web-blue thumb, the console's
    /// rust that lights to its accent, Reforged's bronze that lights to gold.
    /// Every scrolling surface inside a realm draws its bar from here.
    #[must_use]
    pub fn scrollbar(self) -> ScrollbarColors {
        match self {
            Self::Sc2 => ScrollbarColors::default(),
            Self::Remastered => ScrollbarColors {
                thumb: rgba(0xc93a_2c99).into(),
                thumb_hover: rgba(0xff8a_78cc).into(),
                thumb_active: rgba(0xffd0_c8ff).into(),
                ..ScrollbarColors::default()
            },
            Self::Reforged => ScrollbarColors {
                thumb: rgba(0x8a6d_3bb3).into(),
                thumb_hover: rgba(0xe8c8_74d9).into(),
                thumb_active: rgba(0xffe9_a8ff).into(),
                ..ScrollbarColors::default()
            },
        }
    }
}

// ---------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------

/// The chrome, absolutely positioned to fill the panel: bloom under vector,
/// band, plate, and the realm's own furniture. Content goes over it in a
/// [`content`] container.
#[must_use]
pub fn frame(variant: ModalVariant, width: f32, height: f32, textures: &ModalTextures) -> Div {
    match variant {
        ModalVariant::Sc2 => sc2_frame(width, height, textures, false),
        ModalVariant::Remastered => scr_frame(height, textures, false),
        ModalVariant::Reforged => wc3_frame(width, height, textures, false),
    }
}

/// The same shell gone wrong, per realm: StarCraft II's chrome flips whole
/// to the legacy alert orange with a recessed notch where the machined tab
/// was; Remastered's plate is already red, so red cannot mean error there —
/// the frame stays and the caution channel is amber, under a field of
/// interference scanlines; Reforged's torchlight shifts ember-red from
/// above, an omen rather than an alert.
#[must_use]
pub fn error_frame(
    variant: ModalVariant,
    width: f32,
    height: f32,
    textures: &ModalTextures,
) -> Div {
    match variant {
        ModalVariant::Sc2 => sc2_frame(width, height, textures, true),
        ModalVariant::Remastered => scr_frame(height, textures, true),
        ModalVariant::Reforged => wc3_frame(width, height, textures, true),
    }
}

/// How the alarm names itself, in the realm's own voice.
#[must_use]
pub fn error_title(variant: ModalVariant, text: &str) -> Div {
    let titled = match variant {
        ModalVariant::Sc2 => tracked(text, 10.0, 7.0)
            .font_family(FONT_NAVIGATION)
            .text_size(px(22.0))
            .text_color(rgb(0x00e6_f9ff)),
        ModalVariant::Remastered => tracked(text, 8.0, 5.5)
            .font_family(SCR_FONT)
            .text_size(px(19.0))
            .text_color(rgb(0x00ff_cf6a)),
        ModalVariant::Reforged => tracked(text, 3.0, 6.0)
            .font_family(WC3_FONT)
            .text_size(px(22.0))
            .text_color(rgb(0x00e8_c874)),
    };
    div()
        .flex()
        .justify_center()
        .child(titled.font_weight(FontWeight::BOLD))
}

/// The line under the alarm's name: SC2's status code, Remastered's
/// terminal chatter. Reforged draws [`error_divider`] instead.
#[must_use]
pub fn error_subtitle(variant: ModalVariant, text: &str) -> Div {
    let (colour, size) = match variant {
        ModalVariant::Sc2 => (0x00f0_a030, 9.0),
        ModalVariant::Remastered => (0x00c8_9040, 10.0),
        ModalVariant::Reforged => (0x009c_8a6e, 10.0),
    };
    div().flex().justify_center().child(
        tracked(text, 2.0, 3.0)
            .text_size(px(size))
            .text_color(rgb(colour)),
    )
}

/// Reforged's gold hairline, fading out both ways under the omen's name.
/// gpui gradients carry two stops, so the fade is two half-runs meeting in
/// the middle.
#[must_use]
pub fn error_divider() -> Div {
    let half = |from: u32, to: u32| {
        div().w(px(100.0)).h(px(1.0)).bg(linear_gradient(
            90.0,
            linear_color_stop(rgba(from), 0.0),
            linear_color_stop(rgba(to), 1.0),
        ))
    };
    div()
        .flex()
        .justify_center()
        .child(half(0x8a6d_3b00, 0x8a6d_3bff))
        .child(half(0x8a6d_3bff, 0x8a6d_3b00))
}

/// Remastered's hazard stripes — the one bar of paint on the console that
/// means CAUTION before any words do.
#[must_use]
pub fn hazard_bar(textures: &ModalTextures, width: f32) -> Div {
    div().h(px(8.0)).w(px(width)).opacity(0.85).child(
        img(ImageSource::from(textures.hazard(width)))
            .size_full()
            .object_fit(ObjectFit::Fill),
    )
}

/// Where the realm keeps its content: the plate's inset, plus the doc's own
/// inner padding. The caller fills it.
#[must_use]
pub fn content(variant: ModalVariant) -> Div {
    let (left, top, right, bottom) = match variant {
        ModalVariant::Sc2 => (18.0, 22.0, 18.0, 16.0),
        ModalVariant::Remastered => (16.0, 16.0, 16.0, 14.0),
        ModalVariant::Reforged => (16.0, 16.0, 16.0, 16.0),
    };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .right(px(right))
        .bottom(px(bottom))
        .flex()
        .flex_col()
        .pt(px(10.0))
        .px(px(8.0))
}

/// The title, set in the realm's own voice and centred the way the doc sets
/// it. The doc also glows the glyphs; gpui has no text shadow, so the letters
/// carry the colour and the frame carries the light.
#[must_use]
pub fn title(variant: ModalVariant, text: &str) -> Div {
    let titled = match variant {
        ModalVariant::Sc2 => tracked(text, 4.0, 4.2)
            .font_family(FONT_NAVIGATION)
            .text_size(px(15.0))
            .text_color(rgb(0x00e6_f9ff)),
        // the terminal pass retired the phosphor green everywhere on the
        // console: the title speaks in the chrome red like the rest of it
        ModalVariant::Remastered => tracked(text, 4.0, 3.9)
            .font_family(SCR_FONT)
            .text_size(px(14.0))
            .text_color(rgb(0x00ff_8a78)),
        ModalVariant::Reforged => tracked(text, 3.0, 4.5)
            .font_family(WC3_FONT)
            .text_size(px(16.0))
            .text_color(rgb(0x00e8_c874)),
    };
    div()
        .flex()
        .justify_center()
        .child(titled.font_weight(FontWeight::BOLD))
}

/// The ✕, styled but not wired: the caller gives it an id and a click.
#[must_use]
pub fn close_glyph(variant: ModalVariant) -> Div {
    let (rest, lit) = match variant {
        ModalVariant::Sc2 => (0x008f_a4bc, 0x00e6_f9ff),
        ModalVariant::Remastered => (0x008a_7d6e, 0x00ff_8a78),
        ModalVariant::Reforged => (0x009c_8a6e, 0x00ff_e9a8),
    };
    div()
        .absolute()
        .right(px(8.0))
        .top(px(if variant == ModalVariant::Sc2 {
            10.0
        } else {
            8.0
        }))
        .text_size(px(13.0))
        .text_color(rgb(rest))
        .cursor_pointer()
        .hover(move |glyph| glyph.text_color(rgb(lit)))
        .child("✕")
}

/// The doc's letter-spacing, which gpui does not have: each letter is its own
/// element and the tracking is the gap between them. Sound for the caps and
/// small words a modal title sets; a shaped script would need the real thing.
fn tracked(text: &str, tracking: f32, space: f32) -> Div {
    let letters: Vec<char> = text.chars().collect();
    div()
        .flex()
        .gap(px(tracking))
        .children(letters.into_iter().map(move |letter| {
            if letter == ' ' {
                div().w(px(space))
            } else {
                div().child(letter.to_string())
            }
        }))
}

/// The bloom behind the vector — a separate rounded shadow layer, so the
/// edge itself never blurs.
fn bloom(radius: f32, near: (f32, u32), far: (f32, u32)) -> Div {
    div().absolute().inset_0().rounded(px(radius)).shadow(vec![
        BoxShadow::new(px(0.0), px(0.0), rgba(near.1).into()).blur_radius(px(near.0)),
        BoxShadow::new(px(0.0), px(0.0), rgba(far.1).into()).blur_radius(px(far.0)),
    ])
}

/// The metal band's vertical sheen: bright at both rims, dark through the
/// middle. gpui gradients carry two stops, so the doc's five become four
/// stacked strips filling the band box — the plate covers the middle, and
/// only the ring shows. The end strips wear the box's corner radius
/// themselves: `overflow_hidden` clips to the rectangle, not to the corner,
/// and an unrounded strip pokes a square of sheen past the rim's curve.
fn band_strips(
    band_height: f32,
    radius: f32,
    knee: f32,
    bright: u32,
    near: u32,
    mid: u32,
) -> Vec<Div> {
    [
        (0.0, knee, bright, near),
        (knee, 0.5, near, mid),
        (0.5, 1.0 - knee, mid, near),
        (1.0 - knee, 1.0, near, bright),
    ]
    .into_iter()
    .map(|(from, to, top, bottom)| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(from * band_height))
            // a half-pixel of overlap keeps float rounding from opening a
            // seam between strips
            .h(px((to - from) * band_height + 0.5))
            .when(radius > 0.0 && from <= f32::EPSILON, |strip| {
                strip.rounded_t(px(radius))
            })
            .when(radius > 0.0 && to >= 1.0 - f32::EPSILON, |strip| {
                strip.rounded_b(px(radius))
            })
            .bg(linear_gradient(
                180.0,
                linear_color_stop(rgb(top), 0.0),
                linear_color_stop(rgb(bottom), 1.0),
            ))
    })
    .collect()
}

/// StarCraft II: the notched plate. A steel band between hard rims, tab
/// inserts top and bottom, and an energy field washing in from each rim.
/// The alarm flips the whole chrome to the legacy alert orange — same
/// geometry, re-toned — and swaps the machined tabs for a recessed notch:
/// the old hex dialog reborn.
fn sc2_frame(width: f32, height: f32, textures: &ModalTextures, alarm: bool) -> Div {
    let field_width = width - 30.0;
    let top_field = textures.sc2_field(field_width, true, alarm);
    // the alarm carries one field, off the top rim only
    let bottom_field = (!alarm).then(|| textures.sc2_field(field_width, false, alarm));
    let (bloom_near, bloom_far) = if alarm {
        (0xf0a0_304d, 0xf0a0_3014)
    } else {
        (0x2f66_cc47, 0x2f66_cc17)
    };
    let (knee, bright, near, mid) = if alarm {
        (0.12, 0x00f0_a030, 0x00b8_6e18, 0x005e_3410)
    } else {
        (0.06, 0x0043_83dc, 0x0033_6cc6, 0x0026_4f9e)
    };
    let (plate, plate_edge) = if alarm {
        (0x000a_0602, 0xf0a0_3080)
    } else {
        (0x0005_090f, 0x5a8c_d873)
    };
    div()
        .absolute()
        .inset_0()
        .child(bloom(14.0, (26.0, bloom_near), (70.0, bloom_far)))
        .child(
            div()
                .absolute()
                .inset(px(6.0))
                .rounded(px(16.0))
                .overflow_hidden()
                .children(band_strips(height - 12.0, 16.0, knee, bright, near, mid)),
        )
        .child(
            div()
                .absolute()
                .inset(px(11.0))
                .rounded(px(11.0))
                .bg(rgb(plate))
                .border_1()
                .border_color(rgba(plate_edge)),
        )
        .child(
            img(ImageSource::from(top_field))
                .absolute()
                .left(px(15.0))
                .top(px(15.0))
                .w(px(field_width))
                .h(px(FIELD_HEIGHT))
                .object_fit(ObjectFit::Fill),
        )
        .children(bottom_field.map(|bottom_field| {
            img(ImageSource::from(bottom_field))
                .absolute()
                .left(px(15.0))
                .bottom(px(15.0))
                .w(px(field_width))
                .h(px(FIELD_HEIGHT))
                .object_fit(ObjectFit::Fill)
        }))
        .child(sc2_decorations(width, height, alarm))
}

/// The 1.5px outer rim and the machined tab inserts, drawn as real vectors:
/// lyon strokes the rim and fills the trapezoids, so they stay crisp at any
/// scale and any size. The alarm draws neither — its mark is a recessed
/// notch cut into the top band, carrying a bright alert line and two
/// diagonals where the metal was cut.
fn sc2_decorations(width: f32, height: f32, alarm: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let origin = bounds.origin;
            let centre = width / 2.0;
            if alarm {
                // the cut: a bite out of the band, painted the page's own
                // dark so the dimmer reads through where metal was removed
                let mut carve = PathBuilder::fill();
                carve.add_polygon(
                    &[
                        point(origin.x + px(centre - 80.0), origin.y + px(5.0)),
                        point(origin.x + px(centre + 80.0), origin.y + px(5.0)),
                        point(origin.x + px(centre + 70.0), origin.y + px(16.0)),
                        point(origin.x + px(centre - 70.0), origin.y + px(16.0)),
                    ],
                    true,
                );
                if let Ok(cut) = carve.build() {
                    window.paint_path(cut, rgb(0x0004_060a));
                }
                let mut floor = PathBuilder::stroke(px(1.5));
                floor.move_to(point(origin.x + px(centre - 70.0), origin.y + px(16.0)));
                floor.line_to(point(origin.x + px(centre + 70.0), origin.y + px(16.0)));
                if let Ok(line) = floor.build() {
                    window.paint_path(line, rgb(0x00ff_c25e));
                }
                for side in [-1.0f32, 1.0] {
                    let mut cut_edge = PathBuilder::stroke(px(2.0));
                    cut_edge.move_to(point(
                        origin.x + px(side.mul_add(80.0, centre)),
                        origin.y + px(6.0),
                    ));
                    cut_edge.line_to(point(
                        origin.x + px(side.mul_add(70.0, centre)),
                        origin.y + px(16.0),
                    ));
                    if let Ok(edge) = cut_edge.build() {
                        window.paint_path(edge, rgb(0x00ff_c25e));
                    }
                }
                return;
            }
            if let Some(rim) =
                rounded_rect_stroke(origin, 6.0, width - 12.0, height - 12.0, 16.0, 1.5)
            {
                window.paint_path(rim, rgb(0x006f_9ce2));
            }
            // the top insert lights from the rim downward, the bottom one
            // from the rim upward: both wear the light where the metal meets it
            tab(origin, centre, 6.0, 15.0, window, tab_gradient(false));
            tab(
                origin,
                centre,
                height - 6.0,
                height - 15.0,
                window,
                tab_gradient(true),
            );
            if let Some(edge) = tab_outline(origin, centre, 6.0, 15.0, 1.0) {
                window.paint_path(edge, rgb(0x0079_a6e6));
            }
            if let Some(edge) = tab_outline(origin, centre, height - 6.0, height - 15.0, 1.0) {
                window.paint_path(edge, rgba(0x79a6_e6bf));
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

fn tab_gradient(flipped: bool) -> Background {
    let (rim, plate) = (rgb(0x006f_9ce2), rgb(0x0033_6cc6));
    if flipped {
        linear_gradient(
            180.0,
            linear_color_stop(plate, 0.0),
            linear_color_stop(rim, 1.0),
        )
    } else {
        linear_gradient(
            180.0,
            linear_color_stop(rim, 0.0),
            linear_color_stop(plate, 1.0),
        )
    }
}

/// One machined insert: 136 wide at the rim, 112 where it meets the plate.
fn tab_points(origin: Point<Pixels>, centre: f32, rim_y: f32, plate_y: f32) -> [Point<Pixels>; 4] {
    [
        point(origin.x + px(centre - 68.0), origin.y + px(rim_y)),
        point(origin.x + px(centre + 68.0), origin.y + px(rim_y)),
        point(origin.x + px(centre + 56.0), origin.y + px(plate_y)),
        point(origin.x + px(centre - 56.0), origin.y + px(plate_y)),
    ]
}

fn tab(
    origin: Point<Pixels>,
    centre: f32,
    rim_y: f32,
    plate_y: f32,
    window: &mut gpui::Window,
    fill: Background,
) {
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&tab_points(origin, centre, rim_y, plate_y), true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, fill);
    }
}

fn tab_outline(
    origin: Point<Pixels>,
    centre: f32,
    rim_y: f32,
    plate_y: f32,
    weight: f32,
) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::stroke(px(weight));
    builder.add_polygon(&tab_points(origin, centre, rim_y, plate_y), true);
    builder.build().ok()
}

/// A rounded rectangle outline for the rims, stroked at the doc's weight.
fn rounded_rect_stroke(
    origin: Point<Pixels>,
    inset: f32,
    width: f32,
    height: f32,
    radius: f32,
    weight: f32,
) -> Option<Path<Pixels>> {
    let (x, y) = (origin.x + px(inset), origin.y + px(inset));
    let (w, h, r) = (width, height, radius);
    let radii = point(px(r), px(r));
    let mut builder = PathBuilder::stroke(px(weight));
    builder.move_to(point(x + px(r), y));
    builder.line_to(point(x + px(w - r), y));
    builder.arc_to(radii, px(0.0), false, true, point(x + px(w), y + px(r)));
    builder.line_to(point(x + px(w), y + px(h - r)));
    builder.arc_to(radii, px(0.0), false, true, point(x + px(w - r), y + px(h)));
    builder.line_to(point(x + px(r), y + px(h)));
    builder.arc_to(radii, px(0.0), false, true, point(x, y + px(h - r)));
    builder.line_to(point(x, y + px(r)));
    builder.arc_to(radii, px(0.0), false, true, point(x + px(r), y));
    builder.close();
    builder.build().ok()
}

/// Remastered: the redline. A 2px edge with the same metal sheen, a pure
/// black plate, and the signature 1px inner rail — hard corners, no sheen
/// fields, and a glow so faint it barely admits to being one.
fn scr_frame(height: f32, textures: &ModalTextures, alarm: bool) -> Div {
    // the plate is already red, so red cannot mean error here: the frame
    // stays the frame, the plate warms a step, and a field of interference
    // scanlines says the picture itself has gone wrong
    let scanlines = alarm.then(|| textures.scanlines(height - 28.0));
    div()
        .absolute()
        .inset_0()
        .child(bloom(2.0, (24.0, 0x9b1a_1240), (60.0, 0x9b1a_1214)))
        .child(
            div()
                .absolute()
                .inset(px(8.0))
                .rounded(px(2.0))
                .overflow_hidden()
                .children(band_strips(
                    height - 16.0,
                    2.0,
                    0.12,
                    0x00c9_3a2c,
                    0x009b_1a12,
                    0x006b_140d,
                )),
        )
        .child(div().absolute().inset(px(10.0)).bg(rgb(if alarm {
            0x000a_0505
        } else {
            0x0002_0202
        })))
        .child(
            div()
                .absolute()
                .inset(px(16.0))
                .border_1()
                .border_color(rgba(0x9b1a_1280)),
        )
        .children(scanlines.map(|scanlines| {
            img(ImageSource::from(scanlines))
                .absolute()
                .inset(px(14.0))
                .object_fit(ObjectFit::Fill)
        }))
}

/// Reforged: the stone band. Aged-bronze sheen between gold rims, gold
/// corner studs with recessed centres where StarCraft II wears tabs, and a
/// torchlight wash pooling into shadow at the base.
fn wc3_frame(width: f32, height: f32, textures: &ModalTextures, alarm: bool) -> Div {
    let wash = textures.wc3_wash(width - 32.0, height - 32.0, alarm);
    let stud = |left: f32, top: f32| {
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(9.0))
            .h(px(9.0))
            .bg(rgb(0x00e8_c874))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(5.0)).h(px(5.0)).bg(rgb(0x005e_4a26)))
    };
    div()
        .absolute()
        .inset_0()
        .child(bloom(0.0, (24.0, 0xe8c8_742e), (60.0, 0xe8c8_740f)))
        // an omen weighs more than a popover: the alarm's stone also pools
        // plain shadow beneath itself
        .when(alarm, |frame| {
            frame.child(div().absolute().inset_0().shadow(vec![
                BoxShadow::new(px(0.0), px(8.0), rgba(0x0000_0099).into()).blur_radius(px(30.0)),
            ]))
        })
        .child(
            div()
                .absolute()
                .inset(px(8.0))
                .overflow_hidden()
                .children(band_strips(
                    height - 16.0,
                    0.0,
                    0.08,
                    0x008a_6d3b,
                    0x005e_4a26,
                    0x003a_2d18,
                )),
        )
        .child(
            div()
                .absolute()
                .inset(px(15.0))
                .bg(rgb(0x000f_0a06))
                .border_1()
                .border_color(rgba(0xe8c8_7473)),
        )
        .child(
            img(ImageSource::from(wash))
                .absolute()
                .inset(px(16.0))
                .object_fit(ObjectFit::Fill),
        )
        .child(wc3_rims(width, height))
        .child(stud(4.0, 4.0))
        .child(stud(width - 13.0, 4.0))
        .child(stud(4.0, height - 13.0))
        .child(stud(width - 13.0, height - 13.0))
}

fn wc3_rims(width: f32, height: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let origin = bounds.origin;
            let corners = [
                point(origin.x + px(8.0), origin.y + px(8.0)),
                point(origin.x + px(width - 8.0), origin.y + px(8.0)),
                point(origin.x + px(width - 8.0), origin.y + px(height - 8.0)),
                point(origin.x + px(8.0), origin.y + px(height - 8.0)),
            ];
            let mut builder = PathBuilder::stroke(px(1.5));
            builder.add_polygon(&corners, true);
            if let Ok(rim) = builder.build() {
                window.paint_path(rim, rgb(0x00e8_c874));
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

/// The room going dark behind a dialog. The legacy module carried this; it
/// belongs with the shell that replaced it.
#[must_use]
pub fn dimmer(closing: bool) -> gpui::AnyElement {
    let dimmer = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(rgba(0x0003_0552));
    if closing {
        dimmer
            .with_animation(
                "shared-modal-dimmer-close",
                Animation::new(Duration::from_millis(140)).with_easing(ease_in_out),
                |dimmer, delta| dimmer.opacity(1.0 - delta),
            )
            .into_any_element()
    } else {
        dimmer
            .with_animation(
                "shared-modal-dimmer-open",
                Animation::new(Duration::from_millis(160)).with_easing(ease_in_out),
                gpui::Styled::opacity,
            )
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// motion
// ---------------------------------------------------------------------------

/// How long the doc's closes and opens run.
const SCR_OPEN: Duration = Duration::from_millis(340);
const SCR_CLOSE: Duration = Duration::from_millis(260);
const WC3_OPEN: Duration = Duration::from_millis(420);
const WC3_CLOSE: Duration = Duration::from_millis(240);
/// Reduced motion swaps every entrance and exit for this one fade.
const REDUCED_FADE: Duration = Duration::from_millis(150);

/// The CRT power-cycle, held step by step — `steps(1)`, no easing, no
/// motion: pure phosphor. Each entry is (at, opacity) and holds until the
/// next.
const SCR_OPEN_STEPS: &[(f32, f32)] = &[
    (0.0, 0.0),
    (0.20, 1.0),
    (0.35, 0.25),
    (0.50, 1.0),
    (0.65, 0.6),
    (0.80, 1.0),
];
const SCR_CLOSE_STEPS: &[(f32, f32)] = &[
    (0.0, 1.0),
    (0.30, 0.3),
    (0.31, 0.9),
    (0.56, 0.15),
    (0.80, 0.5),
    (0.81, 0.0),
];

fn held(steps: &[(f32, f32)], delta: f32) -> f32 {
    steps
        .iter()
        .rev()
        .find(|(at, _)| delta >= *at)
        .map_or(0.0, |(_, value)| *value)
}

/// The panel under its realm's own motion. StarCraft II opens on the legacy
/// scan-beam reveal — the shipped animation the doc's holo-projector open
/// was describing, and better than a re-transcription of it. Remastered
/// power-cycles, Reforged drops like stone with a 3px thud and sinks away.
/// Every close is faster than its open; reduced motion swaps everything for
/// a 150ms fade.
#[must_use]
pub fn animated(
    variant: ModalVariant,
    panel: Stateful<Div>,
    closing: bool,
    reduced: bool,
    width: f32,
    height: f32,
) -> Div {
    if reduced {
        return faded(panel, closing, width, height);
    }
    match variant {
        ModalVariant::Sc2 => sc2_modal::animated(
            panel,
            closing,
            false,
            width,
            height,
            "shared-modal-sc2-open",
            "shared-modal-sc2-close",
            "shared-modal-sc2-scan-open",
            "shared-modal-sc2-scan-close",
        ),
        ModalVariant::Remastered => flicker(panel, closing, width, height),
        ModalVariant::Reforged => stone(panel, closing, width, height),
    }
}

/// [`animated`], for a dialog wearing [`error_frame`]: the same realm
/// motions, except StarCraft II's scan beam runs in the alert orange.
#[must_use]
pub fn error_animated(
    variant: ModalVariant,
    panel: Stateful<Div>,
    closing: bool,
    reduced: bool,
    width: f32,
    height: f32,
) -> Div {
    if reduced {
        return faded(panel, closing, width, height);
    }
    match variant {
        ModalVariant::Sc2 => sc2_modal::animated(
            panel,
            closing,
            true,
            width,
            height,
            "shared-modal-alarm-open",
            "shared-modal-alarm-close",
            "shared-modal-alarm-scan-open",
            "shared-modal-alarm-scan-close",
        ),
        ModalVariant::Remastered => flicker(panel, closing, width, height),
        ModalVariant::Reforged => stone(panel, closing, width, height),
    }
}

fn shell_box(width: f32, height: f32) -> Div {
    div().relative().w(px(width)).h(px(height))
}

fn faded(panel: Stateful<Div>, closing: bool, width: f32, height: f32) -> Div {
    let fade = Animation::new(REDUCED_FADE).with_easing(ease_in_out);
    let faded = if closing {
        panel
            .with_animation("shared-modal-fade-close", fade, |panel, delta| {
                panel.opacity(1.0 - delta)
            })
            .into_any_element()
    } else {
        panel
            .with_animation("shared-modal-fade-open", fade, gpui::Styled::opacity)
            .into_any_element()
    };
    shell_box(width, height).child(faded)
}

fn flicker(panel: Stateful<Div>, closing: bool, width: f32, height: f32) -> Div {
    let stepped = if closing {
        panel
            .with_animation(
                "shared-modal-scr-close",
                Animation::new(SCR_CLOSE),
                |panel, delta| panel.opacity(held(SCR_CLOSE_STEPS, delta)),
            )
            .into_any_element()
    } else {
        panel
            .with_animation(
                "shared-modal-scr-open",
                Animation::new(SCR_OPEN),
                |panel, delta| panel.opacity(held(SCR_OPEN_STEPS, delta)),
            )
            .into_any_element()
    };
    shell_box(width, height).child(stepped)
}

fn stone(panel: Stateful<Div>, closing: bool, width: f32, height: f32) -> Div {
    let mover = div()
        .absolute()
        .left_0()
        .w(px(width))
        .h(px(height))
        .child(panel);
    let moved = if closing {
        mover
            .with_animation(
                "shared-modal-wc3-close",
                Animation::new(WC3_CLOSE),
                |mover, delta| {
                    let sunk = cubic_bezier(0.55, 0.0, 0.85, 0.4, delta);
                    mover.top(px(14.0 * sunk)).opacity(1.0 - sunk)
                },
            )
            .into_any_element()
    } else {
        mover
            .with_animation(
                "shared-modal-wc3-open",
                Animation::new(WC3_OPEN),
                |mover, delta| {
                    // the doc's two keyframe legs: drop past rest to a 3px
                    // thud by 55%, then settle back up
                    let (offset, light) = if delta < 0.55 {
                        let leg = cubic_bezier(0.22, 1.0, 0.36, 1.0, delta / 0.55);
                        (29.0f32.mul_add(leg, -26.0), leg)
                    } else {
                        let leg = cubic_bezier(0.22, 1.0, 0.36, 1.0, (delta - 0.55) / 0.45);
                        (3.0 - 3.0 * leg, 1.0)
                    };
                    mover.top(px(offset)).opacity(light)
                },
            )
            .into_any_element()
    };
    shell_box(width, height).child(moved)
}

// ---------------------------------------------------------------------------
// baked light
// ---------------------------------------------------------------------------

/// The energy fields and the torch wash are radial washes with masked
/// hairlines — light gpui has no gradient for, so they are baked per size
/// and cached. Textures render at 2x so retina panes stay crisp. The cache
/// is interior-mutable because the components that draw dialogs hand their
/// chrome around by shared reference; the UI is single-threaded, so a
/// `RefCell` says exactly as much as is true.
#[derive(Default)]
pub struct ModalTextures {
    cache: RefCell<TextureCache>,
}

#[derive(Default)]
struct TextureCache {
    sc2_fields: HashMap<(u32, bool, bool), Arc<RenderImage>>,
    wc3_washes: HashMap<(u32, u32, bool), Arc<RenderImage>>,
    scanlines: HashMap<u32, Arc<RenderImage>>,
    veils: HashMap<u32, Arc<RenderImage>>,
    hazards: HashMap<u32, Arc<RenderImage>>,
    vignette: Option<Arc<RenderImage>>,
}

const FIELD_HEIGHT: f32 = 58.0;
const TEXTURE_SCALE: f32 = 2.0;

impl ModalTextures {
    fn sc2_field(&self, width: f32, top: bool, alarm: bool) -> Arc<RenderImage> {
        let key = (width.round().max(1.0) as u32, top, alarm);
        self.cache
            .borrow_mut()
            .sc2_fields
            .entry(key)
            .or_insert_with(|| render_image(bake_sc2_field(key.0 as f32, top, alarm)))
            .clone()
    }

    /// Remastered's interference: the last logical pixel of every four is a
    /// 28% black line. One texel wide — the renderer stretches it across.
    fn scanlines(&self, height: f32) -> Arc<RenderImage> {
        let key = height.round().max(1.0) as u32;
        self.cache
            .borrow_mut()
            .scanlines
            .entry(key)
            .or_insert_with(|| render_image(bake_scanlines(key as f32, 0.28)))
            .clone()
    }

    /// The same interference at whisper strength, for laying over a whole
    /// window rather than a dialog plate.
    pub fn scanline_veil(&self, height: f32) -> Arc<RenderImage> {
        let key = height.round().max(1.0) as u32;
        self.cache
            .borrow_mut()
            .veils
            .entry(key)
            .or_insert_with(|| render_image(bake_scanlines(key as f32, 0.10)))
            .clone()
    }

    /// A corner-darkening vignette, baked once at a small fixed size and
    /// stretched over whatever it dims — the falloff is soft enough that the
    /// scale-up never shows.
    pub fn vignette(&self) -> Arc<RenderImage> {
        self.cache
            .borrow_mut()
            .vignette
            .get_or_insert_with(|| render_image(bake_vignette()))
            .clone()
    }

    /// Remastered's hazard stripes: 45° caution amber over near-black.
    fn hazard(&self, width: f32) -> Arc<RenderImage> {
        let key = width.round().max(1.0) as u32;
        self.cache
            .borrow_mut()
            .hazards
            .entry(key)
            .or_insert_with(|| render_image(bake_hazard(key as f32)))
            .clone()
    }

    fn wc3_wash(&self, width: f32, height: f32, alarm: bool) -> Arc<RenderImage> {
        let key = (
            width.round().max(1.0) as u32,
            height.round().max(1.0) as u32,
            alarm,
        );
        self.cache
            .borrow_mut()
            .wc3_washes
            .entry(key)
            .or_insert_with(|| render_image(bake_wc3_wash(key.0 as f32, key.1 as f32, alarm)))
            .clone()
    }
}

/// An RGBA bake handed to the renderer, which consumes BGRA.
fn render_image(mut baked: RgbaImage) -> Arc<RenderImage> {
    for pixel in baked.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![Frame::new(baked)]))
}

/// Where a point sits on a radial gradient's ray: 0 at the centre, 1 at the
/// ellipse the stops are measured against.
fn ray(x: f32, y: f32, centre: (f32, f32), radii: (f32, f32)) -> f32 {
    let dx = (x - centre.0) / radii.0;
    let dy = (y - centre.1) / radii.1;
    dx.hypot(dy)
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    (to - from).mul_add(amount.clamp(0.0, 1.0), from)
}

/// One straight-alpha source-over, in place.
fn compose(under: &mut [f32; 4], over: [f32; 4]) {
    let alpha = over[3] + under[3] * (1.0 - over[3]);
    if alpha <= f32::EPSILON {
        *under = [0.0; 4];
        return;
    }
    for channel in 0..3 {
        under[channel] =
            over[3].mul_add(over[channel], under[channel] * under[3] * (1.0 - over[3])) / alpha;
    }
    under[3] = alpha;
}

fn to_pixel(value: [f32; 4]) -> Rgba<u8> {
    Rgba([
        (value[0].clamp(0.0, 255.0)).round() as u8,
        (value[1].clamp(0.0, 255.0)).round() as u8,
        (value[2].clamp(0.0, 255.0)).round() as u8,
        (value[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ])
}

/// The doc's energy field: a blue radial wash off the rim with fine 115°
/// scan hairlines masked by the same falloff, so the texture fades with the
/// glow instead of ending in a hard grid edge. The bottom field is the same
/// light at lower wattage.
fn bake_sc2_field(width: f32, top: bool, alarm: bool) -> RgbaImage {
    // the alarm's field is the legacy alert orange at the same wattage the
    // chrome fields were tuned down to
    let (wash_from, wash_to, hair) = if alarm {
        (
            [240.0, 160.0, 48.0],
            [140.0, 80.0, 20.0],
            [255.0, 194.0, 94.0],
        )
    } else {
        (
            [64.0, 150.0, 232.0],
            [28.0, 80.0, 150.0],
            [90.0, 170.0, 240.0],
        )
    };
    let (wash_full, wash_mid, hair_strength) = match (top, alarm) {
        (true, true) => (0.26, 0.10, 0.07),
        (true, false) => (0.32, 0.13, 0.09),
        (false, _) => (0.22, 0.09, 0.06),
    };
    let device_width = (width * TEXTURE_SCALE).round().max(1.0) as u32;
    let device_height = (FIELD_HEIGHT * TEXTURE_SCALE).round() as u32;
    let origin_y = if top { 0.0 } else { FIELD_HEIGHT };
    let (angle_sin, angle_cos) = (115.0f32.to_radians().sin(), 115.0f32.to_radians().cos());
    let mut baked = RgbaImage::new(device_width, device_height);
    for (x, y, pixel) in baked.enumerate_pixels_mut() {
        let lx = (x as f32 + 0.5) / TEXTURE_SCALE;
        let ly = (y as f32 + 0.5) / TEXTURE_SCALE;

        let wash_ray = ray(
            lx,
            ly,
            (width / 2.0, origin_y),
            (0.70 * width, 1.30 * FIELD_HEIGHT),
        );
        let mut value = if wash_ray <= 0.55 {
            let blend = wash_ray / 0.55;
            [
                lerp(wash_from[0], wash_to[0], blend),
                lerp(wash_from[1], wash_to[1], blend),
                lerp(wash_from[2], wash_to[2], blend),
                lerp(wash_full, wash_mid, blend),
            ]
        } else {
            [
                wash_to[0],
                wash_to[1],
                wash_to[2],
                lerp(wash_mid, 0.0, (wash_ray - 0.55) / 0.25),
            ]
        };

        // the hairline pattern rides over the wash, masked by its own
        // slightly tighter falloff
        let position = angle_sin.mul_add(lx, -(angle_cos * ly));
        if position.rem_euclid(7.0) < 1.0 {
            let mask_ray = ray(
                lx,
                ly,
                (width / 2.0, origin_y),
                (0.65 * width, 1.10 * FIELD_HEIGHT),
            );
            let mask = 1.0 - ((mask_ray - 0.20) / 0.55).clamp(0.0, 1.0);
            compose(
                &mut value,
                [hair[0], hair[1], hair[2], hair_strength * mask],
            );
        }
        *pixel = to_pixel(value);
    }
    baked
}

/// Reforged's light: warm torchlight washing down from the top of the plate
/// and shadow pooling at its base. The doc layers the torch over the shadow.
fn bake_wc3_wash(width: f32, height: f32, alarm: bool) -> RgbaImage {
    let device_width = (width * TEXTURE_SCALE).round().max(1.0) as u32;
    let device_height = (height * TEXTURE_SCALE).round().max(1.0) as u32;
    let corner = std::f32::consts::SQRT_2;
    let mut baked = RgbaImage::new(device_width, device_height);
    for (x, y, pixel) in baked.enumerate_pixels_mut() {
        let lx = (x as f32 + 0.5) / TEXTURE_SCALE;
        let ly = (y as f32 + 0.5) / TEXTURE_SCALE;

        let shade_ray = ray(
            lx,
            ly,
            (width / 2.0, height),
            (0.5 * width * corner, height * corner),
        );
        let mut value = [0.0, 0.0, 0.0, lerp(0.5, 0.0, shade_ray / 0.60)];

        let torch_ray = ray(
            lx,
            ly,
            (width / 2.0, 0.0),
            (0.5 * width * corner, height * corner),
        );
        // the omen's light: torchlight gold in chrome, ember-red from above
        // when something has gone wrong
        let (torch, strength, reach) = if alarm {
            ([200.0, 60.0, 30.0], 0.16, 0.60)
        } else {
            ([232.0, 200.0, 116.0], 0.08, 0.55)
        };
        compose(
            &mut value,
            [
                torch[0],
                torch[1],
                torch[2],
                lerp(strength, 0.0, torch_ray / reach),
            ],
        );
        *pixel = to_pixel(value);
    }
    baked
}

/// One texel wide: transparent for three logical pixels, 28% black for the
/// fourth. Stretched horizontally by the renderer.
fn bake_scanlines(height: f32, strength: f32) -> RgbaImage {
    let device_height = (height * TEXTURE_SCALE).round().max(1.0) as u32;
    let mut baked = RgbaImage::new(1, device_height);
    for (_, y, pixel) in baked.enumerate_pixels_mut() {
        let ly = (y as f32 + 0.5) / TEXTURE_SCALE;
        let value = if ly.rem_euclid(4.0) >= 3.0 {
            [0.0, 0.0, 0.0, strength]
        } else {
            [0.0; 4]
        };
        *pixel = to_pixel(value);
    }
    baked
}

/// Clear through the middle, easing to a 30% black at the corners. The
/// ellipse the falloff is measured against overhangs the box the way the
/// design's does, so the edges of the frame dim before the centre ever
/// does.
fn bake_vignette() -> RgbaImage {
    const SIZE: f32 = 128.0;
    let device = (SIZE * TEXTURE_SCALE).round() as u32;
    let mut baked = RgbaImage::new(device, device);
    for (x, y, pixel) in baked.enumerate_pixels_mut() {
        let lx = (x as f32 + 0.5) / TEXTURE_SCALE;
        let ly = (y as f32 + 0.5) / TEXTURE_SCALE;
        let position = ray(lx, ly, (SIZE / 2.0, SIZE / 2.0), (1.2 * SIZE, 1.1 * SIZE));
        let alpha = ((position - 0.40) / 0.20).clamp(0.0, 1.0) * 0.3;
        *pixel = to_pixel([0.0, 0.0, 0.0, alpha]);
    }
    baked
}

/// Caution stripes at 45°: ten logical pixels of amber, ten of near-black.
fn bake_hazard(width: f32) -> RgbaImage {
    let device_width = (width * TEXTURE_SCALE).round().max(1.0) as u32;
    let device_height = (8.0 * TEXTURE_SCALE).round() as u32;
    let mut baked = RgbaImage::new(device_width, device_height);
    for (x, y, pixel) in baked.enumerate_pixels_mut() {
        let lx = (x as f32 + 0.5) / TEXTURE_SCALE;
        let ly = (y as f32 + 0.5) / TEXTURE_SCALE;
        let value = if (lx + ly).rem_euclid(20.0) < 10.0 {
            [232.0, 168.0, 56.0, 1.0]
        } else {
            [26.0, 13.0, 5.0, 1.0]
        };
        *pixel = to_pixel(value);
    }
    baked
}

#[cfg(test)]
mod tests {
    use super::{SCR_CLOSE_STEPS, SCR_OPEN_STEPS, held};

    #[test]
    fn the_power_cycle_is_held_step_by_step() {
        // steps(1): each level holds until the next keyframe, nothing eases
        assert!((held(SCR_OPEN_STEPS, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((held(SCR_OPEN_STEPS, 0.19) - 0.0).abs() < f32::EPSILON);
        assert!((held(SCR_OPEN_STEPS, 0.21) - 1.0).abs() < f32::EPSILON);
        assert!((held(SCR_OPEN_STEPS, 0.40) - 0.25).abs() < f32::EPSILON);
        assert!((held(SCR_OPEN_STEPS, 1.0) - 1.0).abs() < f32::EPSILON);

        // the close lands dark and stays dark: fill-mode both is what keeps
        // a closed modal closed
        assert!((held(SCR_CLOSE_STEPS, 0.0) - 1.0).abs() < f32::EPSILON);
        assert!((held(SCR_CLOSE_STEPS, 0.9) - 0.0).abs() < f32::EPSILON);
        assert!((held(SCR_CLOSE_STEPS, 1.0) - 0.0).abs() < f32::EPSILON);
    }
}
