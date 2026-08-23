//! The modern button — one weight system, dressed per realm.
//!
//! From `Canvas.dc.html`'s BUTTONS section: primary carries fill and bloom,
//! ghost is border-only, and danger reuses each realm's error channel — SC2's
//! legacy alert orange, Remastered's caution amber, Reforged's ember. Pressed
//! always darkens and insets, except Remastered, whose press is inverse
//! video. Loading never spins. The legacy nine-patch `action_button` stays
//! until this one is validated; the games list wears these first.
//!
//! The component styles and reacts; the caller sizes it (`.h`/`.w`) and
//! words it — including the realm's own loading vocabulary. Labels arrive
//! already cased for the realm; Remastered's brackets are added here because
//! they are the component's chrome, not the words.

use gpui::{
    BoxShadow, Div, ElementId, FontWeight, SharedString, Stateful, div, prelude::*, px, rgb, rgba,
};

pub use crate::products::modal::ModalVariant;
use crate::products::sc2::theme::FONT_NAVIGATION;
use crate::products::scr::theme::FONT_INTERFACE as SCR_FONT;

const WC3_FONT: &str = crate::products::wc3::theme::FONT_TITLE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonWeight {
    /// Fill and bloom: the action the dialog is about.
    Primary,
    /// Border only (Remastered: bare command text): everything else.
    Ghost,
}

/// Which channel the button speaks in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonTone {
    /// The realm's own chrome accent.
    Chrome,
    /// The realm's error channel: alert orange, caution amber, ember.
    Danger,
}

/// What the button is doing, when it is not being pointed at.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonLife {
    #[default]
    Ready,
    Disabled,
    Loading,
}

/// One realm's ink for one tone: the border, fill alpha or bevel, and text
/// at each state the pointer can put it in.
struct ButtonInk {
    edge: u32,
    edge_hover: u32,
    edge_pressed: u32,
    fill: f32,
    fill_hover: f32,
    fill_pressed: f32,
    text: u32,
    text_hover: u32,
    text_pressed: u32,
    glow: Option<(f32, u32)>,
    glow_hover: Option<(f32, u32)>,
    /// Reforged's fills are bevels, not washes: (top, bottom) alphas for
    /// default, hover, and pressed — the pressed pair inverts, which is what
    /// makes stone feel pushed in.
    bevel: Option<[(f32, f32); 3]>,
}

/// The colour a fill alpha rides on, per tone.
const fn base(variant: ModalVariant, tone: ButtonTone) -> u32 {
    match (variant, tone) {
        (ModalVariant::Sc2, ButtonTone::Chrome) => 0x0033_a8f0,
        (ModalVariant::Sc2, ButtonTone::Danger) => 0x00f0_a030,
        (ModalVariant::Remastered, ButtonTone::Chrome) => 0x00c9_3a2c,
        (ModalVariant::Remastered, ButtonTone::Danger) => 0x00e8_a838,
        (ModalVariant::Reforged, ButtonTone::Chrome) => 0x00e8_c874,
        (ModalVariant::Reforged, ButtonTone::Danger) => 0x00c8_5a28,
    }
}

fn ink(variant: ModalVariant, weight: ButtonWeight, tone: ButtonTone) -> ButtonInk {
    use ButtonTone::{Chrome, Danger};
    use ButtonWeight::{Ghost, Primary};
    use ModalVariant::{Reforged, Remastered, Sc2};
    match (variant, weight, tone) {
        (Sc2, Primary, Chrome) => ButtonInk {
            edge: 0x0033_a8f0,
            edge_hover: 0x006b_c2f2,
            edge_pressed: 0x0022_77b8,
            fill: 0.18,
            fill_hover: 0.32,
            fill_pressed: 0.38,
            text: 0x00e6_f9ff,
            text_hover: 0x00ff_ffff,
            text_pressed: 0x00c8_e4f8,
            glow: Some((14.0, 0x33a8_f040)),
            glow_hover: Some((20.0, 0x33a8_f066)),
            bevel: None,
        },
        (Sc2, Primary, Danger) => ButtonInk {
            edge: 0x00f0_a030,
            edge_hover: 0x00ff_c25e,
            edge_pressed: 0x00b8_6e18,
            fill: 0.18,
            fill_hover: 0.30,
            fill_pressed: 0.36,
            text: 0x00ff_e9c0,
            text_hover: 0x00ff_f6e4,
            text_pressed: 0x00f0_d8a8,
            glow: Some((14.0, 0xf0a0_304d)),
            glow_hover: Some((20.0, 0xf0a0_3073)),
            bevel: None,
        },
        (Sc2, Ghost, Chrome) => ButtonInk {
            edge: 0x0033_a8f0,
            edge_hover: 0x0033_a8f0,
            edge_pressed: 0x0033_a8f0,
            fill: 0.06,
            fill_hover: 0.16,
            fill_pressed: 0.24,
            text: 0x006b_c2f2,
            text_hover: 0x00e6_f9ff,
            text_pressed: 0x00c8_e4f8,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Sc2, Ghost, Danger) => ButtonInk {
            edge: 0x00f0_a030,
            edge_hover: 0x00f0_a030,
            edge_pressed: 0x00f0_a030,
            fill: 0.06,
            fill_hover: 0.16,
            fill_pressed: 0.24,
            text: 0x00f0_b050,
            text_hover: 0x00ff_e9c0,
            text_pressed: 0x00f0_d8a8,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Remastered, Primary, Chrome) => ButtonInk {
            edge: 0x00c9_3a2c,
            edge_hover: 0x00ff_6a58,
            edge_pressed: 0x00ff_6a58,
            fill: 0.14,
            fill_hover: 0.26,
            fill_pressed: 1.0,
            text: 0x00ff_8a78,
            text_hover: 0x00ff_f0ec,
            text_pressed: 0x0014_0505,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Remastered, Primary, Danger) => ButtonInk {
            edge: 0x00e8_a838,
            edge_hover: 0x00ff_cf6a,
            edge_pressed: 0x00ff_cf6a,
            fill: 0.12,
            fill_hover: 0.24,
            fill_pressed: 1.0,
            text: 0x00ff_cf6a,
            text_hover: 0x00ff_f4dc,
            text_pressed: 0x0014_0d05,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Remastered, Ghost, Chrome) => ButtonInk {
            edge: 0,
            edge_hover: 0,
            edge_pressed: 0,
            fill: 0.0,
            fill_hover: 0.0,
            fill_pressed: 1.0,
            text: 0x00d8_8070,
            text_hover: 0x00ff_d0c8,
            text_pressed: 0x001a_0505,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Remastered, Ghost, Danger) => ButtonInk {
            edge: 0,
            edge_hover: 0,
            edge_pressed: 0,
            fill: 0.0,
            fill_hover: 0.0,
            fill_pressed: 1.0,
            text: 0x00e8_a838,
            text_hover: 0x00ff_f4dc,
            text_pressed: 0x0014_0d05,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Reforged, Primary, Chrome) => ButtonInk {
            edge: 0x00e8_c874,
            edge_hover: 0x00ff_e9a8,
            edge_pressed: 0x008a_6d3b,
            fill: 0.17,
            fill_hover: 0.25,
            fill_pressed: 0.17,
            text: 0x00ff_e9a8,
            text_hover: 0x00ff_f6dc,
            text_pressed: 0x00e8_d4a0,
            glow: Some((12.0, 0xe8c8_7433)),
            glow_hover: Some((18.0, 0xe8c8_7452)),
            bevel: Some([(0.22, 0.12), (0.32, 0.18), (0.14, 0.20)]),
        },
        (Reforged, Primary, Danger) => ButtonInk {
            edge: 0x00c8_6a3a,
            edge_hover: 0x00e8_8a4a,
            edge_pressed: 0x008a_4a26,
            fill: 0.19,
            fill_hover: 0.27,
            fill_pressed: 0.22,
            text: 0x00ff_d0b0,
            text_hover: 0x00ff_e4d0,
            text_pressed: 0x00f0_c4a4,
            glow: Some((12.0, 0xc86a_3a40)),
            glow_hover: Some((18.0, 0xc86a_3a61)),
            bevel: Some([(0.25, 0.12), (0.36, 0.18), (0.20, 0.24)]),
        },
        (Reforged, Ghost, Chrome) => ButtonInk {
            edge: 0x005e_4a26,
            edge_hover: 0x008a_6d3b,
            edge_pressed: 0x008a_6d3b,
            fill: 0.05,
            fill_hover: 0.12,
            fill_pressed: 0.16,
            text: 0x00c8_b088,
            text_hover: 0x00ff_e9a8,
            text_pressed: 0x00e8_d4a0,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
        (Reforged, Ghost, Danger) => ButtonInk {
            edge: 0x008a_4a26,
            edge_hover: 0x00c8_6a3a,
            edge_pressed: 0x00c8_6a3a,
            fill: 0.06,
            fill_hover: 0.14,
            fill_pressed: 0.18,
            text: 0x00e8_a878,
            text_hover: 0x00ff_d0b0,
            text_pressed: 0x00f0_c4a4,
            glow: None,
            glow_hover: None,
            bevel: None,
        },
    }
}

/// How a realm mutes a button that cannot answer: (border, text). `None`
/// border means none is drawn, which is Remastered's ghost either way.
const fn muted(variant: ModalVariant, weight: ButtonWeight, life: ButtonLife) -> (u32, u32) {
    use ModalVariant::{Reforged, Remastered, Sc2};
    match (variant, life) {
        (Sc2, ButtonLife::Loading) => (0x33a8_f080, 0x006b_c2f2),
        (Sc2, _) => match weight {
            ButtonWeight::Primary => (0x33a8_f02e, 0x0033_506e),
            ButtonWeight::Ghost => (0x33a8_f024, 0x002c_4258),
        },
        (Remastered, ButtonLife::Loading) => (0xc93a_2c66, 0x00d8_8070),
        (Remastered, _) => (0xc93a_2c2e, 0x006e_352c),
        (Reforged, ButtonLife::Loading) => (0x8a6d_3bff, 0x00c8_b088),
        (Reforged, _) => match weight {
            ButtonWeight::Primary => (0x4a3a_22ff, 0x006e_5f48),
            ButtonWeight::Ghost => (0x3a2d_18ff, 0x0054_462e),
        },
    }
}

/// The words as the realm sets them: Remastered's commands wear brackets.
/// Everyone else's label passes through, already cased by the caller.
#[must_use]
pub fn worded(variant: ModalVariant, label: &str) -> String {
    if variant == ModalVariant::Remastered {
        format!("[ {label} ]")
    } else {
        label.to_owned()
    }
}

fn inset_press() -> Vec<BoxShadow> {
    vec![
        BoxShadow::new(px(0.0), px(2.0), rgba(0x0208_10b3).into())
            .blur_radius(px(7.0))
            .inset(),
    ]
}

fn bloom(shadow: (f32, u32)) -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.0), px(0.0), rgba(shadow.1).into()).blur_radius(px(shadow.0))]
}

/// The button, styled and reactive but unsized and unwired: the caller sets
/// its box and hooks the click on the returned element. A `Disabled` or
/// `Loading` button neither reacts nor invites — no hover, no press, no
/// pointer.
#[must_use]
pub fn button(
    id: impl Into<ElementId>,
    variant: ModalVariant,
    weight: ButtonWeight,
    tone: ButtonTone,
    life: ButtonLife,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let bordered = !(variant == ModalVariant::Remastered && weight == ButtonWeight::Ghost);
    let seat = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .when(bordered, gpui::Styled::border_1)
        .child(label.into());
    let seat = match variant {
        ModalVariant::Sc2 => seat
            .font_family(FONT_NAVIGATION)
            .font_weight(FontWeight::BOLD)
            .text_size(px(11.0)),
        ModalVariant::Remastered => seat.font_family(SCR_FONT).text_size(px(12.0)),
        ModalVariant::Reforged => seat.font_family(WC3_FONT).text_size(px(13.5)),
    };
    if life != ButtonLife::Ready {
        let (edge, text) = muted(variant, weight, life);
        let colour = base(variant, tone);
        return seat
            .border_color(rgba(edge))
            .text_color(rgb(text))
            .when(life == ButtonLife::Loading, |seat| {
                seat.bg(rgba(fade(colour, 0.08)))
            });
    }
    let ink = ink(variant, weight, tone);
    let colour = base(variant, tone);
    let fill_of = move |alpha: f32, pair: Option<(f32, f32)>| -> gpui::Background {
        match pair {
            Some((top, bottom)) => gpui::linear_gradient(
                180.0,
                gpui::linear_color_stop(rgba(fade(colour, top)), 0.0),
                gpui::linear_color_stop(rgba(fade(colour, bottom)), 1.0),
            ),
            None => rgba(fade(colour, alpha)).into(),
        }
    };
    let bevel = ink.bevel;
    let pressed_fill = if ink.fill_pressed >= 1.0 {
        // inverse video: Remastered's press is the terminal's, not a bevel's
        rgba((colour << 8) | 0xff).into()
    } else {
        fill_of(ink.fill_pressed, bevel.map(|bevel| bevel[2]))
    };
    let (pressed_edge, pressed_text) = (ink.edge_pressed, ink.text_pressed);
    let (hover_edge, hover_text, hover_fill) = (ink.edge_hover, ink.text_hover, ink.fill_hover);
    let hover_glow = ink.glow_hover;
    let inverse = ink.fill_pressed >= 1.0;
    seat.cursor_pointer()
        .bg(fill_of(ink.fill, bevel.map(|bevel| bevel[0])))
        .border_color(rgb(ink.edge))
        .text_color(rgb(ink.text))
        .when_some(ink.glow, |seat, glow| seat.shadow(bloom(glow)))
        .hover(move |style| {
            let style = style
                .bg(fill_of(hover_fill, bevel.map(|bevel| bevel[1])))
                .border_color(rgb(hover_edge))
                .text_color(rgb(hover_text));
            match hover_glow {
                Some(glow) => style.shadow(bloom(glow)),
                None => style,
            }
        })
        .active(move |style| {
            let style = style
                .bg(pressed_fill)
                .border_color(rgb(pressed_edge))
                .text_color(rgb(pressed_text));
            if inverse {
                style
            } else {
                style.shadow(inset_press())
            }
        })
}

/// Puts an alpha on a `0x00RRGGBB` colour.
const fn fade(colour: u32, alpha: f32) -> u32 {
    let alpha = (alpha * 255.0) as u32;
    (colour << 8) | if alpha > 255 { 255 } else { alpha }
}

#[cfg(test)]
mod tests {
    use super::{ModalVariant, worded};

    #[test]
    fn remastered_speaks_in_brackets_and_nobody_else_does() {
        assert_eq!(worded(ModalVariant::Remastered, "CONNECT"), "[ CONNECT ]");
        assert_eq!(worded(ModalVariant::Sc2, "CONNECT"), "CONNECT");
        assert_eq!(worded(ModalVariant::Reforged, "Connect"), "Connect");
    }
}
