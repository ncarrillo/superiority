//! The modern input field and checkbox — dressed per realm, like the
//! buttons and the modal before them.
//!
//! From `Canvas.dc.html`'s INPUT FIELDS and CHECKBOXES sections: StarCraft
//! II gets a square well that lights cyan under focus; Remastered gets a
//! bare prompt line — underline only, a `>` that answers, a block cursor;
//! Reforged gets a carved stone slot with a gold rim and an inset shadow
//! that never leaves. Checkboxes: cyan square with an angular check, a
//! bracket toggle with no box at all, and a gold-studded recessed socket.
//! Error speaks each realm's own channel — alert orange, caution amber,
//! ember.
//!
//! The text engine ([`crate::foundation::text_input`]) stays the engine;
//! these dress its box and its ink. `dressed` styles any element (the SC2
//! composer is already a complicated seat), `field_shell` is the plain well
//! most surfaces want.

use gpui::{
    BoxShadow, Div, ElementId, PathBuilder, SharedString, Stateful, canvas, div, point, prelude::*,
    px, rgb, rgba,
};

use crate::foundation::text_input::FieldInk;
pub use crate::products::modal::ModalVariant;
use crate::products::scr::theme::FONT_INTERFACE as SCR_FONT;

/// Whether the field can answer. (Loading is a button's problem.)
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FieldLife {
    #[default]
    Ready,
    Disabled,
}

/// What the checkbox holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CheckMark {
    Empty,
    /// How much of the check has arrived, so a toggle can grow its mark the
    /// way the legacy checkbox did. A settled check passes 1.0.
    Check(f32),
    /// Some of a set, not all of it.
    Bar,
}

/// The engine ink each realm writes in.
#[must_use]
pub fn field_ink(variant: ModalVariant) -> FieldInk {
    match variant {
        ModalVariant::Sc2 => FieldInk {
            placeholder: 0x004a_5a70,
            cursor: 0x006b_c2f2,
            cursor_width: 1.5,
        },
        ModalVariant::Remastered => FieldInk {
            placeholder: 0x008a_4a3e,
            cursor: 0x00ff_8a78,
            // inverse video is Remastered's press; a block is its cursor
            cursor_width: 8.0,
        },
        ModalVariant::Reforged => FieldInk {
            placeholder: 0x006e_5f48,
            cursor: 0x00e8_c874,
            cursor_width: 1.0,
        },
    }
}

/// Dresses any seat as the realm's field: border, fill, focus light, and
/// the error channel. Composite fields (the SC2 composer) chain this onto
/// the seat they already have; plain wells use [`field_shell`].
pub fn dressed<E: Styled>(seat: E, variant: ModalVariant, focused: bool, error: bool) -> E {
    match variant {
        ModalVariant::Sc2 => {
            let seat = seat.border_1();
            if error {
                seat.border_color(rgb(0x00f0_a030))
                    .bg(rgba(0xf0a0_300d))
                    .shadow(vec![
                        BoxShadow::new(px(0.0), px(0.0), rgba(0xf0a0_3040).into())
                            .blur_radius(px(10.0)),
                    ])
            } else if focused {
                seat.border_color(rgb(0x0033_a8f0))
                    .bg(rgba(0x0208_10e6))
                    .shadow(vec![
                        BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f040).into())
                            .blur_radius(px(12.0)),
                        BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f00d).into())
                            .blur_radius(px(18.0))
                            .inset(),
                    ])
            } else {
                seat.border_color(rgba(0x133e_5be6)).bg(rgba(0x0208_10cc))
            }
        }
        // the prompt line: underline only, and the light falls off it
        // downward the way a terminal's glow does
        ModalVariant::Remastered => {
            let seat = seat.border_b_1();
            if error {
                seat.border_color(rgb(0x00e8_a838)).shadow(vec![
                    BoxShadow::new(px(0.0), px(8.0), rgba(0xe8a8_3880).into())
                        .blur_radius(px(12.0))
                        .spread_radius(px(-10.0)),
                ])
            } else if focused {
                seat.border_color(rgb(0x00ff_6a58)).shadow(vec![
                    BoxShadow::new(px(0.0), px(8.0), rgba(0xff5a_4880).into())
                        .blur_radius(px(12.0))
                        .spread_radius(px(-10.0)),
                ])
            } else {
                seat.border_color(rgba(0xc93a_2c80))
            }
        }
        // the carved slot: the inset shadow never leaves — stone stays carved
        ModalVariant::Reforged => {
            let carve = BoxShadow::new(px(0.0), px(2.0), rgba(0x0000_00b3).into())
                .blur_radius(px(5.0))
                .inset();
            let seat = seat.border_1();
            if error {
                seat.border_color(rgb(0x00c8_6a3a))
                    .bg(rgba(0xc85a_280f))
                    .shadow(vec![
                        carve,
                        BoxShadow::new(px(0.0), px(0.0), rgba(0xc86a_3a40).into())
                            .blur_radius(px(10.0)),
                    ])
            } else if focused {
                seat.border_color(rgb(0x00e8_c874))
                    .bg(rgb(0x0014_100a))
                    .shadow(vec![
                        carve,
                        BoxShadow::new(px(0.0), px(0.0), rgba(0xe8c8_7433).into())
                            .blur_radius(px(10.0)),
                    ])
            } else {
                seat.border_color(rgb(0x005e_4a26))
                    .bg(rgb(0x000f_0a06))
                    .shadow(vec![carve])
            }
        }
    }
}

/// A field that cannot answer, in the realm's own dimness.
pub fn dressed_disabled<E: Styled>(seat: E, variant: ModalVariant) -> E {
    match variant {
        ModalVariant::Sc2 => seat
            .border_1()
            .border_color(rgba(0x133e_5b66))
            .bg(rgba(0x0208_1066)),
        ModalVariant::Remastered => seat.border_b_1().border_color(rgba(0xc93a_2c33)),
        ModalVariant::Reforged => seat
            .border_1()
            .border_color(rgb(0x003a_2d18))
            .bg(rgb(0x000a_0704)),
    }
}

/// The plain input well: a dressed, padded row the caller drops the
/// engine's element into and sizes. Remastered's well carries its prompt.
#[must_use]
pub fn field_shell(variant: ModalVariant, focused: bool, life: FieldLife, error: bool) -> Div {
    let seat = div().flex().items_center();
    let seat = match variant {
        ModalVariant::Sc2 => seat.px(px(11.0)),
        ModalVariant::Remastered => seat
            .px(px(2.0))
            .gap(px(6.0))
            .child(prompt_glyph(variant, focused, life, error)),
        ModalVariant::Reforged => seat.px(px(11.0)),
    };
    if life == FieldLife::Disabled {
        dressed_disabled(seat, variant)
    } else {
        dressed(seat, variant, focused, error)
    }
}

/// Remastered's `>` — or its `!` when something is wrong. Composite seats
/// (the console composer) place it themselves; [`field_shell`] seats it.
#[must_use]
pub fn prompt_glyph(variant: ModalVariant, focused: bool, life: FieldLife, error: bool) -> Div {
    debug_assert_eq!(variant, ModalVariant::Remastered);
    let (glyph, colour) = if error {
        ("!", 0x00ff_cf6a)
    } else if life == FieldLife::Disabled {
        (">", 0x005e_2c24)
    } else if focused {
        (">", 0x00ff_8a78)
    } else {
        (">", 0x00b0_4a3e)
    };
    div()
        .flex_shrink_0()
        .font_family(SCR_FONT)
        .text_color(rgb(colour))
        .child(glyph)
}

/// The line under a field that has been refused, in the realm's own words'
/// dress. The words themselves are the caller's.
#[must_use]
pub fn error_caption(variant: ModalVariant, text: impl Into<SharedString>) -> Div {
    match variant {
        ModalVariant::Sc2 => div()
            .text_size(px(9.0))
            .text_color(rgb(0x00f0_b050))
            .child(text.into()),
        ModalVariant::Remastered => div()
            .font_family(SCR_FONT)
            .text_size(px(9.0))
            .text_color(rgb(0x00ff_cf6a))
            .child(SharedString::from(format!("/// {}", text.into()))),
        ModalVariant::Reforged => div()
            .font_family("Friz Quadrata TT")
            .italic()
            .text_size(px(11.0))
            .text_color(rgb(0x00ff_d0b0))
            .child(text.into()),
    }
}

/// The checkbox. Square and cyan for StarCraft II, a bracket toggle with no
/// box for Remastered, a recessed stone socket for Reforged. The caller
/// wires the click on the returned element and says what it holds; `Check`
/// carries an arrival amount so toggles can grow their mark.
#[must_use]
pub fn checkbox(
    id: impl Into<ElementId>,
    variant: ModalVariant,
    mark: CheckMark,
    life: FieldLife,
    error: bool,
) -> Stateful<Div> {
    if variant == ModalVariant::Remastered {
        return bracket_checkbox(id, mark, life, error);
    }
    let disabled = life == FieldLife::Disabled;
    let marked = !matches!(mark, CheckMark::Empty);
    let gold = variant == ModalVariant::Reforged;
    let seat = div()
        .id(id)
        .w(px(15.0))
        .h(px(15.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_1();
    let seat = if gold {
        // the socket stays recessed whatever it holds
        let carve = BoxShadow::new(px(0.0), px(1.0), rgba(0x0000_00b3).into())
            .blur_radius(px(3.0))
            .inset();
        let seat = seat.shadow(vec![carve]).bg(rgb(0x000f_0a06));
        if disabled {
            seat.border_color(rgb(0x003a_2d18))
                .bg(rgb(0x000a_0704))
                .opacity(0.4)
        } else if error {
            seat.border_color(rgb(0x00c8_6a3a)).bg(rgba(0xc85a_281f))
        } else if marked {
            seat.border_color(rgb(0x00e8_c874))
                .cursor_pointer()
                .hover(|seat| seat.border_color(rgb(0x00ff_e9a8)))
        } else {
            seat.border_color(rgb(0x005e_4a26))
                .cursor_pointer()
                .hover(|seat| seat.border_color(rgb(0x008a_6d3b)).bg(rgb(0x0014_100a)))
        }
    } else if disabled {
        seat.border_color(rgba(0x33a8_f04d)).opacity(0.4)
    } else if error {
        seat.border_color(rgb(0x00f0_a030)).bg(rgba(0xf0a0_301a))
    } else if marked {
        seat.border_color(rgb(0x0033_a8f0))
            .bg(rgba(0x33a8_f038))
            .shadow(vec![
                BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f066).into()).blur_radius(px(8.0)),
            ])
            .cursor_pointer()
            .hover(|seat| seat.border_color(rgb(0x006b_c2f2)))
    } else {
        seat.border_color(rgba(0x33a8_f080))
            .bg(rgba(0x33a8_f00d))
            .cursor_pointer()
            .hover(|seat| seat.border_color(rgb(0x006b_c2f2)).bg(rgba(0x33a8_f024)))
    };
    let ink = if gold { 0x00e8_c874 } else { 0x006b_c2f2 };
    match mark {
        CheckMark::Empty => seat,
        CheckMark::Check(amount) => seat.child(angular_check(ink, amount.clamp(0.0, 1.0))),
        CheckMark::Bar => {
            seat.child(
                div()
                    .w(px(7.0))
                    .h(px(2.0))
                    .bg(rgb(if gold { 0x00c8_b088 } else { ink })),
            )
        }
    }
}

/// Remastered has no box to check: the toggle is a bracket command.
fn bracket_checkbox(
    id: impl Into<ElementId>,
    mark: CheckMark,
    life: FieldLife,
    error: bool,
) -> Stateful<Div> {
    let disabled = life == FieldLife::Disabled;
    let (glyph, colour) = if error {
        ("[!]", 0x00ff_cf6a)
    } else if disabled {
        ("[  ]", 0x005e_2c24)
    } else {
        match mark {
            CheckMark::Empty => ("[  ]", 0x00b0_4a3e),
            // the mark grows nothing here — a terminal's check simply is
            CheckMark::Check(amount) if amount > 0.5 => ("[\u{2713}]", 0x00ff_8a78),
            CheckMark::Check(_) => ("[  ]", 0x00b0_4a3e),
            CheckMark::Bar => ("[-]", 0x00ff_8a78),
        }
    };
    div()
        .id(id)
        .flex_shrink_0()
        .font_family(SCR_FONT)
        .text_size(px(12.0))
        .text_color(rgb(colour))
        .when(!disabled, |seat| {
            seat.cursor_pointer()
                .hover(|seat| seat.text_color(rgb(0x00ff_6a58)))
        })
        .child(glyph)
}

/// The angular check, drawn as a real vector so it can arrive: `amount`
/// walks the stroke in, 0 to 1.
fn angular_check(ink: u32, amount: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            if amount <= 0.02 {
                return;
            }
            let origin = bounds.origin;
            // the doc's 10×8 polyline, centred in the 13px interior
            let points = [(1.0f32, 4.0f32), (4.0, 7.0), (9.0, 1.0)];
            let offset_x = 1.5;
            let offset_y = 2.5;
            // total path length, so the mark walks in point to point
            let legs = [((points[0]), (points[1])), ((points[1]), (points[2]))];
            let lengths: Vec<f32> = legs
                .iter()
                .map(|((ax, ay), (bx, by))| (bx - ax).hypot(by - ay))
                .collect();
            let total: f32 = lengths.iter().sum();
            let mut remaining = total * amount.clamp(0.0, 1.0);
            let mut builder = PathBuilder::stroke(px(2.0));
            builder.move_to(point(
                origin.x + px(points[0].0 + offset_x),
                origin.y + px(points[0].1 + offset_y),
            ));
            for (((ax, ay), (bx, by)), length) in legs.iter().zip(&lengths) {
                if remaining <= 0.0 {
                    break;
                }
                let walked = (remaining / length).min(1.0);
                builder.line_to(point(
                    origin.x + px(walked.mul_add(bx - ax, *ax) + offset_x),
                    origin.y + px(walked.mul_add(by - ay, *ay) + offset_y),
                ));
                remaining -= length;
            }
            if let Ok(check) = builder.build() {
                window.paint_path(check, rgb(ink));
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::{CheckMark, FieldLife, ModalVariant, field_ink};

    #[test]
    fn each_realm_writes_in_its_own_ink() {
        // the block cursor is Remastered's alone; the others keep a bar
        assert!(field_ink(ModalVariant::Remastered).cursor_width > 1.5);
        assert!(field_ink(ModalVariant::Sc2).cursor_width <= 1.5);
        assert!(field_ink(ModalVariant::Reforged).cursor_width <= 1.5);
    }

    #[test]
    fn the_types_say_what_a_field_can_hold() {
        assert_eq!(FieldLife::default(), FieldLife::Ready);
        assert_ne!(CheckMark::Check(1.0), CheckMark::Empty);
    }
}
