//! The host-neutral card renderers. Every function here is pure: it takes a
//! palette, a resolved [`CardLook`], a [`CardSize`], and — where a card reads a
//! session — a [`CardData`], and returns a gpui element. Nothing reaches into a
//! host: art arrives already resolved to an [`ImageSource`], so the same card
//! draws on the desktop and in the browser.

use gpui::{
    Animation, AnimationExt as _, AnyElement, Div, FontWeight, ImageSource, ObjectFit, Stateful,
    div, ease_in_out, img, prelude::*, px, relative, rgb, rgba,
};

use crate::products::buttons as ui_buttons;
use crate::products::sc2::theme::{FONT_NAVIGATION, scope_glow};

use super::look::CardLook;
use super::model::{CardData, CardSize, CardState, GamePalette, region_name};

const MASTHEAD: u32 = 0x000a_0d12;
const RULE: u32 = 0x001a_2028;
const PAPER: u32 = 0x00e6_edf7;
const SCAN_WIDTH: f32 = 0.26;
const SCAN_PERIOD: std::time::Duration = std::time::Duration::from_millis(1400);
const EMBER_PERIOD: std::time::Duration = std::time::Duration::from_millis(1200);

#[must_use]
pub fn masthead() -> Div {
    div()
        .flex_shrink_0()
        .flex()
        .items_baseline()
        .justify_between()
        .px(px(20.0))
        .py(px(12.0))
        .bg(rgb(MASTHEAD))
        .border_b_1()
        .border_color(rgb(RULE))
        .child(
            div()
                .font_family(FONT_NAVIGATION)
                .text_size(px(13.0))
                .text_color(rgb(PAPER))
                .child("GAME CARD — LIVE STATES"),
        )
        .child(
            div()
                .text_size(px(10.5))
                .text_color(rgb(0x006e_7a8c))
                .child("design-time data"),
        )
}

/// One card. The structure is the same in all six states and for all three
/// games; everything that differs comes off the palette, and everything that
/// moves comes off the look, which is already mid-transition when it arrives.
/// The demo card carries the state's own status line; a host that draws the
/// picker composes [`destination_line`]/[`identity_region_line`] itself.
#[must_use]
pub fn game_card(palette: &GamePalette, look: &CardLook, size: CardSize, art: ImageSource) -> Div {
    card_shell(palette, look, size, look.glow)
        .child(card_art(palette, look, size, art))
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(size.gap))
                .px(px(size.pad_x))
                .pt(px(size.pad_top))
                .pb(px(size.pad_bottom))
                .child(status_line(palette, look, size))
                .child(progress_rail(palette, look))
                .child(card_button(palette, look, size, None, true)),
        )
}

/// The card's own frame. `glow` is passed in rather than read off the look
/// because the two screens mean different things by it: on the walked cards it
/// is the state's own bloom, and in the picker it belongs to the pointer.
#[must_use]
pub fn card_shell(palette: &GamePalette, look: &CardLook, size: CardSize, glow: f32) -> Div {
    div()
        .w(px(size.width))
        .flex()
        .flex_col()
        .bg(rgba(palette.card_fill))
        .border_1()
        .border_color(rgb(look.border))
        .opacity(look.card_light)
        .when(glow > 0.01, |card| {
            card.shadow(scope_glow(fade(palette.glow, glow)))
        })
}

/// The card's first line, split for colour: the accent half names where
/// Enter takes you, the muted half carries the count or the working text.
/// No destination means no accent — the muted half says what is true alone.
#[must_use]
pub fn destination_parts(
    palette: &GamePalette,
    data: &CardData,
    look: &CardLook,
) -> (Option<String>, String) {
    if let Some(live) = data.live.as_ref() {
        if let Some(progress) = live.progress.clone() {
            // mid-handshake the step counter rides the working line
            let detail = match live.step.clone() {
                Some(step) => format!("{progress} · {step}"),
                None => progress,
            };
            return (None, detail);
        }
        if let Some(channel) = live.channel.clone() {
            let count = live
                .online
                .map(|online| format!("· {online} online"))
                .unwrap_or_default();
            return (Some(format!("→ {channel}")), count);
        }
        return (None, look.status.clone());
    }
    if !data.owned {
        return if data.licensed {
            (None, "In your library · no client yet".to_owned())
        } else {
            (None, "Not on this account".to_owned())
        };
    }
    if data.live_mode {
        // no session behind this card yet, and nothing true to say about it
        return (None, String::new());
    }
    // the design screens: the connected fixture leads with its channel, the
    // rest keep their recency or the state's own words
    if data.state == CardState::Connected {
        let (channel, count) = palette
            .recency
            .split_once(" · ")
            .map_or((palette.recency, String::new()), |(channel, count)| {
                (channel, format!("· {count}"))
            });
        return (Some(format!("→ {channel}")), count);
    }
    if data.state.lit() {
        (None, look.status.clone())
    } else {
        (None, palette.recency.to_owned())
    }
}

/// Who this game knows you as, with the clan tag in the clan's own colour. The
/// session's answer wins over the fixture's wherever there is one. A roster
/// name carries its tag inline, so the tag comes off the name rather than being
/// said twice.
#[must_use]
pub fn identity_parts(palette: &GamePalette, data: &CardData) -> (Option<String>, String) {
    let live = data.live.as_ref();
    let live_mode = data.live_mode;
    // the palette's identity is design-time dressing; a live card that has not
    // been told who it is says nothing rather than borrowing it
    let fixture = |value: Option<&'static str>| (!live_mode).then_some(value).flatten();
    let tag = live
        .map_or_else(
            || fixture(palette.clan_tag).map(str::to_owned),
            |live| live.clan_tag.clone(),
        )
        .map(|tag| tag.trim_matches(['<', '>']).to_owned())
        .filter(|tag| !tag.is_empty());
    let handle = live
        .and_then(|live| live.handle.clone())
        .or_else(|| fixture(Some(palette.handle)).map(str::to_owned))
        .unwrap_or_default();
    let handle = tag.as_ref().map_or(handle.clone(), |tag| {
        handle
            .strip_prefix(&format!("<{tag}> "))
            .unwrap_or(&handle)
            .to_owned()
    });
    (tag, handle)
}

#[must_use]
pub fn destination_line(
    palette: &GamePalette,
    data: &CardData,
    look: &CardLook,
    size: CardSize,
) -> Div {
    let (destination, detail) = destination_parts(palette, data, look);
    // a line that does not fit ends in an ellipsis rather than mid-letter
    div()
        .flex()
        .gap(px(5.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(size.status))
        .text_color(rgb(palette.sub))
        .children(destination.map(|destination| {
            div()
                .flex_shrink_0()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.beacon))
                .child(destination)
        }))
        .child(div().min_w_0().truncate().child(detail))
}

/// The second line: who this realm knows you as, and where it is. A live
/// session knows its region; it does not measure a latency, so none is
/// claimed.
#[must_use]
pub fn identity_region_line(palette: &GamePalette, data: &CardData, size: CardSize) -> Div {
    let (tag, handle) = identity_parts(palette, data);
    let region = data.live.as_ref().map_or_else(
        || (!data.live_mode).then(|| palette.region.to_owned()),
        |live| region_name(live.region).map(str::to_owned),
    );
    div()
        .flex()
        .gap(px(5.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(size.status))
        .text_color(rgb(palette.dim))
        .children(tag.map(|tag| {
            div()
                .flex_shrink_0()
                .text_color(rgb(palette.clan))
                .child(format!("<{tag}>"))
        }))
        .child(div().min_w_0().truncate().child(handle))
        .children(region.map(|region| {
            div()
                .flex_shrink_0()
                .text_color(rgb(palette.sub))
                .child(format!("· {region}"))
        }))
}

/// The line under the title: the identity where the state names you, and the
/// state's own words everywhere else.
#[must_use]
pub fn status_line(palette: &GamePalette, look: &CardLook, size: CardSize) -> Div {
    div()
        .h(px(size.status + 3.5))
        .flex()
        .gap(px(5.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(size.status))
        .text_color(rgb(look.status_colour))
        // the tag is the clan's colour wherever it appears, which is not the
        // colour of the line it sits on
        .children(look.status_tag.map(|tag| {
            div()
                .flex_shrink_0()
                .text_color(rgb(palette.clan))
                .child(tag)
        }))
        .child(look.status.clone())
}

#[must_use]
pub fn card_art(palette: &GamePalette, look: &CardLook, size: CardSize, art: ImageSource) -> Div {
    // Reforged has no photograph; its zone is painted light — ember from
    // below-left, moonlight kissing the top-right corner — lit like a WC3
    // loading screen, which is what makes the gold chrome make sense
    let art_zone: AnyElement = if palette.torchlit {
        div()
            .absolute()
            .inset(px(-look.art_overhang))
            .opacity(look.art_light)
            .bg(gpui::linear_gradient(
                180.0,
                gpui::linear_color_stop(rgba(0x0f0a_06ff), 0.0),
                gpui::linear_color_stop(rgba(0x1a10_04ff), 1.0),
            ))
            .child(div().absolute().inset_0().bg(gpui::linear_gradient(
                45.0,
                gpui::linear_color_stop(rgba(0xe88c_3073), 0.0),
                gpui::linear_color_stop(rgba(0xe88c_3000), 0.7),
            )))
            .child(div().absolute().inset_0().bg(gpui::linear_gradient(
                225.0,
                gpui::linear_color_stop(rgba(0x5a6e_8c40), 0.0),
                gpui::linear_color_stop(rgba(0x5a6e_8c00), 0.65),
            )))
            .into_any_element()
    } else {
        img(art)
            .absolute()
            .inset(px(-look.art_overhang))
            .opacity(look.art_light)
            .grayscale(look.colourless)
            .object_fit(ObjectFit::Cover)
            .into_any_element()
    };
    div()
        .relative()
        // the art takes whatever height the card has spare, rather than
        // leaving an empty band between the meta and the button
        .min_h(px(size.art))
        .flex_1()
        .overflow_hidden()
        .child(art_zone)
        // the art fades into the body rather than stopping at a seam
        .child(div().absolute().inset_0().bg(gpui::linear_gradient(
            180.0,
            gpui::linear_color_stop(rgba(palette.fade & 0xffff_ff00), 0.4),
            gpui::linear_color_stop(rgba(palette.fade), 1.0),
        )))
        // the title sits on the art, film-poster style, over that fade
        .child(
            div()
                .absolute()
                .left(px(size.pad_x))
                .right(px(size.pad_x))
                .bottom(px(10.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(palette.font)
                .font_weight(FontWeight::BOLD)
                .text_size(px(size.title))
                .text_color(rgb(look.title))
                .child(palette.title()),
        )
        // a lit card throws its own colour up off the floor of the art
        .when(look.glow > 0.01, |art| {
            art.child(div().absolute().inset_0().bg(gpui::linear_gradient(
                0.0,
                gpui::linear_color_stop(rgba(fade(palette.glow, look.glow * 0.24)), 0.0),
                gpui::linear_color_stop(rgba(palette.glow & 0xffff_ff00), 0.65),
            )))
        })
        // an unreachable realm goes cold: the grey art takes the game's own
        // error colour rather than turning a generic red
        .when(look.cold > 0.01, |art| {
            art.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(fade(palette.err << 8, look.cold * 0.14))),
            )
        })
        .when(look.ember > 0.01, |art| {
            art.child(
                div()
                    .absolute()
                    .left(px(10.0))
                    .top(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .opacity(look.ember)
                    .child(ember_beacon(palette))
                    .child(
                        div()
                            .font_family(FONT_NAVIGATION)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(size.badge))
                            .text_color(rgb(palette.err_light))
                            .child("SIGNAL LOST"),
                    ),
            )
        })
        .when(look.scan > 0.01, |art| {
            art.child(scan_beam(palette, look.scan))
        })
        .when(look.badge > 0.01, |art| {
            // a glowing dot and the realm's own accent: still scannable as
            // "all good", but the game's name stays the loudest thing here
            art.child(
                div()
                    .absolute()
                    .right(px(8.0))
                    .top(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .opacity(look.badge)
                    .child(
                        div()
                            .size(px(6.0))
                            .rounded_full()
                            .bg(rgb(palette.beacon))
                            .shadow(vec![
                                gpui::BoxShadow::new(
                                    px(0.0),
                                    px(0.0),
                                    rgba(fade(palette.beacon << 8, 0.9)).into(),
                                )
                                .blur_radius(px(6.0)),
                            ]),
                    )
                    .child(
                        div()
                            .text_size(px(size.badge))
                            .text_color(rgb(palette.beacon))
                            .child("CONNECTED"),
                    ),
            )
        })
}

/// A band of the game's own light travelling across the art — the only thing on
/// the card that says work is happening rather than waiting.
#[must_use]
pub fn scan_beam(palette: &GamePalette, light: f32) -> AnyElement {
    let glow = palette.glow;
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(SCAN_WIDTH))
        .opacity(light)
        .with_animation(
            "game-card-scan",
            Animation::new(SCAN_PERIOD).repeat(),
            move |beam, delta| {
                beam.left(relative(delta.mul_add(1.4, -0.3)))
                    .bg(gpui::linear_gradient(
                        90.0,
                        gpui::linear_color_stop(rgba(glow & 0xffff_ff00), 0.0),
                        gpui::linear_color_stop(rgba(glow), 0.5),
                    ))
            },
        )
        .into_any_element()
}

#[must_use]
pub fn ember_beacon(palette: &GamePalette) -> AnyElement {
    let err = palette.err;
    div()
        .size(px(8.0))
        .rounded_full()
        .bg(rgb(err))
        .with_animation(
            "game-card-ember",
            Animation::new(EMBER_PERIOD)
                .with_easing(ease_in_out)
                .repeat(),
            |beacon, delta| beacon.opacity(1.0 - (delta * 0.65)),
        )
        .into_any_element()
}

/// The handshake's own progress. The rail fades with the state while the fill
/// crawls on its own two-second clock, which is what makes a handshake feel
/// like one.
#[must_use]
pub fn progress_rail(palette: &GamePalette, look: &CardLook) -> Div {
    div()
        .h(px(2.0))
        .bg(rgba(0x6e7a_8c40))
        .opacity(look.progress_rail)
        .child(
            div()
                .h(px(2.0))
                .w(relative(look.progress))
                .bg(rgb(palette.focus))
                .shadow(scope_glow(palette.glow)),
        )
}

/// Which shared personality dresses this palette's buttons.
#[must_use]
pub fn personality(palette: &GamePalette) -> ui_buttons::ModalVariant {
    match palette.program {
        "S1" => ui_buttons::ModalVariant::Remastered,
        "W3" => ui_buttons::ModalVariant::Reforged,
        _ => ui_buttons::ModalVariant::Sc2,
    }
}

#[must_use]
pub fn card_button(
    palette: &GamePalette,
    look: &CardLook,
    size: CardSize,
    out_of_play: Option<&'static str>,
    seated: bool,
) -> Stateful<Div> {
    let realm = personality(palette);
    let life = if out_of_play.is_some() {
        ui_buttons::ButtonLife::Disabled
    } else if look.verb == CardState::Connecting.verb() {
        ui_buttons::ButtonLife::Loading
    } else {
        ui_buttons::ButtonLife::Ready
    };
    let tone = if out_of_play.is_none() && look.verb == CardState::Unreachable.verb() {
        ui_buttons::ButtonTone::Danger
    } else {
        ui_buttons::ButtonTone::Chrome
    };
    let weight = if seated {
        ui_buttons::ButtonWeight::Primary
    } else {
        ui_buttons::ButtonWeight::Ghost
    };
    let verb = out_of_play.unwrap_or(look.verb);
    let label = ui_buttons::worded(
        realm,
        &if palette.upper {
            verb.to_owned()
        } else {
            titlecase(verb)
        },
    );
    ui_buttons::button(palette.dressing, realm, weight, tone, life, label)
        .h(px(size.button))
        .relative()
        .opacity(look.button_light)
        // only the seat carries the glyph: three \u{21b5} at once said
        // "Enter does… which one?" It hangs off the button's right edge
        // rather than sitting in the row, so the verb stays centred and
        // nothing shifts when the seat moves from card to card.
        .when(seated && look.return_key > 0.01, |button| {
            button.child(
                div()
                    .absolute()
                    .right(px(8.0))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .px(px(5.0))
                            .opacity(look.return_key)
                            .border_1()
                            .border_color(rgba(fade(palette.focus << 8, 0.5)))
                            .text_size(px(10.0))
                            .text_color(rgb(palette.focus))
                            .child("\u{21b5}"),
                    ),
            )
        })
}

/// The lockup's letter-spacing, which gpui does not have: every letter is
/// its own element and the tracking is the gap between them. Sound for the
/// caps-only strings the lockup sets; a shaped script would need the real
/// thing. A space cannot carry the word gap itself — shaping collapses a
/// whitespace-only run — so it becomes a spacer about one space wide.
#[must_use]
pub fn tracked(text: &str, tracking: f32, styled: Div) -> Div {
    styled
        .flex()
        .gap(px(tracking))
        .children(text.chars().map(|letter| {
            if letter == ' ' {
                div().w(px(2.5))
            } else {
                div().child(letter.to_string())
            }
        }))
}

/// Warcraft's voice is not shouted, so its verbs are not either.
#[must_use]
pub fn titlecase(word: &str) -> String {
    word.split(' ')
        .map(|part| {
            let mut letters = part.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>()
                    + &letters.collect::<String>().to_lowercase()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Puts an alpha on a `0xRRGGBBAA` colour, which is how a value that is
/// crossing over is dimmed without touching its hue.
#[must_use]
pub fn fade(colour: u32, light: f32) -> u32 {
    let alpha = (light.clamp(0.0, 1.0) * 255.0).round().clamp(0.0, 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a byte immediately above"
    )]
    let alpha = alpha as u32;
    (colour & 0xffff_ff00) | alpha
}

/// The realm taking the whole stage. It is the same art the room was showing,
/// brought up to full and lit from below in the game's own colour.
#[must_use]
pub fn realm_flood(palette: &GamePalette, flood: f32, art: ImageSource) -> Div {
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .opacity(flood)
        .child(
            img(art)
                .absolute()
                .inset_0()
                .size_full()
                .opacity(0.55)
                .object_fit(ObjectFit::Cover),
        )
        .child(div().absolute().inset_0().bg(gpui::linear_gradient(
            0.0,
            gpui::linear_color_stop(rgba(fade(palette.glow, 0.3)), 0.0),
            gpui::linear_color_stop(rgba(0x0208_10b3), 0.75),
        )))
}

/// One game's light in the room behind the cards: its art, and its own colour
/// thrown up off the floor.
#[must_use]
pub fn room_layer(palette: &GamePalette, light: f32, art: ImageSource) -> Div {
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .opacity(light)
        .child(
            img(art)
                .absolute()
                .inset_0()
                .size_full()
                .opacity(0.22)
                .object_fit(ObjectFit::Cover),
        )
        .child(div().absolute().inset_0().bg(gpui::linear_gradient(
            0.0,
            gpui::linear_color_stop(rgba(fade(palette.glow, 0.18)), 0.0),
            gpui::linear_color_stop(rgba(palette.glow & 0xffff_ff00), 0.6),
        )))
}

#[cfg(test)]
mod tests {
    use super::super::model::{CardData, GAMES, LiveCard};
    use super::identity_parts;

    fn live(clan_tag: Option<&str>, handle: Option<&str>) -> CardData {
        CardData {
            state: super::CardState::Idle,
            owned: true,
            licensed: true,
            live_mode: true,
            live: Some(LiveCard {
                clan_tag: clan_tag.map(str::to_owned),
                handle: handle.map(str::to_owned),
                ..LiveCard::default()
            }),
        }
    }

    fn design(live_mode: bool) -> CardData {
        CardData {
            state: super::CardState::Idle,
            owned: true,
            licensed: true,
            live_mode,
            live: None,
        }
    }

    #[test]
    fn the_clan_tag_is_worn_once() {
        // a roster name arrives already wearing its tag, and the card draws the
        // tag itself in the clan's colour — printing both said `<MDGTN> <MDGTN>
        // ncarrillo`
        assert_eq!(
            identity_parts(&GAMES[0], &live(Some("MDGTN"), Some("<MDGTN> ncarrillo"))),
            (Some("MDGTN".to_owned()), "ncarrillo".to_owned())
        );

        // a name that never carried one is left alone, and so is a tag that
        // belongs to somebody else
        assert_eq!(
            identity_parts(&GAMES[0], &live(Some("MDGTN"), Some("ncarrillo"))).1,
            "ncarrillo"
        );
        assert_eq!(
            identity_parts(&GAMES[0], &live(Some("ROOT"), Some("<MDGTN> ncarrillo"))).1,
            "<MDGTN> ncarrillo"
        );
    }

    #[test]
    fn a_live_card_with_no_session_says_nothing_rather_than_the_fixture() {
        // the palettes carry a handle, region, ping, and channel so the design
        // screens have something to draw. on a live card they read as somebody's
        // real account — Remastered's tile showed `ncarrillo` in `The Void` at
        // 54 ms, none of which had ever been true of that session
        let (tag, handle) = identity_parts(&GAMES[1], &design(true));
        assert_eq!(
            handle, "",
            "no session, so nothing is claimed about who you are"
        );
        assert_eq!(tag, None);

        let (tag, handle) = identity_parts(&GAMES[1], &design(false));
        assert_eq!(handle, GAMES[1].handle);
        assert_eq!(tag.is_some(), GAMES[1].clan_tag.is_some());
    }
}

#[must_use]
pub const fn marker_colour(state: CardState, room: &GamePalette) -> u32 {
    match state {
        CardState::Connected => room.ok,
        CardState::Unreachable => room.err,
        CardState::NoAccount => 0x006e_7a8c,
        _ => room.focus,
    }
}
