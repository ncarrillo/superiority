use gpui::{PathBuilder, canvas, point};

use super::*;

// the card renderers and sizing fixtures live in the shared crate now; the
// desktop reads them from there so both hosts draw one card system. `CardSize`,
// `masthead`, and `picker_size` are re-exported because `mod.rs` imports them
// from `view`.
use superiority_ui::foundation::assets::{AssetResolver, NativeAssetResolver};
use superiority_ui::products::games::{
    CardData, HERO_CARD, LiveCard, TRIO_CARD, card_art, card_button, card_shell, destination_line,
    fade, game_card, identity_region_line, marker_colour, progress_rail, realm_flood, room_layer,
    tracked,
};
pub(super) use superiority_ui::products::games::{CardSize, masthead, picker_size};
#[cfg(test)]
use superiority_ui::products::games::{PICKER_MEDIUM, PICKER_NARROW, PICKER_WIDE};

/// this game's art as the desktop bundle serves it — the native side of the
/// shared `AssetPaths`, resolved to an image the shared renderers can draw.
fn art_of(palette: &GamePalette) -> gpui::ImageSource {
    NativeAssetResolver.image(palette.art)
}

/// the neutral card inputs the shared renderers take, built from this shell's
/// own live-session state for one game.
fn card_data(games: &GamesComponent, index: usize, state: CardState) -> CardData {
    CardData {
        state,
        owned: games.owns(index),
        licensed: games.licensed(index),
        live_mode: games.live_mode,
        live: games.live_for(index).map(|live| LiveCard {
            clan_tag: live.clan_tag.clone(),
            handle: live.handle.clone(),
            channel: live.channel.clone(),
            region: live.region,
            online: live.online,
            progress: live.progress.clone(),
            step: live.step.clone(),
        }),
    }
}

/// the launcher's top bar is Superiority chrome, not a realm's: always the
/// system cyan, 34px tall, and it sits on the shell. From the `Titlebar`
/// design: window controls · realm refresh · session identity + sign out.
const PICKER_TITLEBAR_HEIGHT: f32 = 34.0;
const BAR_TOP: u32 = 0x000a_0e16;
const BAR_BOTTOM: u32 = 0x0007_0a10;
const BAR_RULE: u32 = 0x001a_2028;
const SYSTEM_CYAN: u32 = 0x006b_c2f2;
const SYSTEM_CYAN_LIT: u32 = 0x00e6_f9ff;
const IDENTITY: u32 = 0x007d_8fa8;
const IDENTITY_REGION: u32 = 0x0033_506e;
const IDENTITY_RULE: u32 = 0x33a8_f040;
const CHIP_LABEL_SIZE: f32 = 8.5;
const CHIP_TRACKING: f32 = 1.5;
const GLYPH_SIZE: f32 = 11.0;
/// one turn of the REFRESH arrow while the realms are being polled.
const SWEEP_PERIOD: Duration = Duration::from_millis(900);

/// how a chip is inked in one state: border and fill are `0xRRGGBBAA`, the
/// text is `0xRRGGBB`.
#[derive(Clone, Copy)]
struct ChipInk {
    border: u32,
    fill: u32,
    text: u32,
}

/// the ghost chip at rest, and lit under the pointer.
const CHIP_REST: ChipInk = ChipInk {
    border: 0x33a8_f059,
    fill: 0x0000_0000,
    text: SYSTEM_CYAN,
};
const CHIP_LIT: ChipInk = ChipInk {
    border: 0x33a8_f0ff,
    fill: 0x33a8_f01a,
    text: SYSTEM_CYAN_LIT,
};
const CHIP_POLLING: ChipInk = ChipInk {
    border: 0x33a8_f080,
    fill: 0x33a8_f00f,
    text: SYSTEM_CYAN,
};
const CHIP_UP_TO_DATE: ChipInk = ChipInk {
    border: 0x47d1_8499,
    fill: 0x47d1_8414,
    text: 0x0047_d184,
};
const CHIP_DISABLED: ChipInk = ChipInk {
    border: 0x33a8_f024,
    fill: 0x0000_0000,
    text: 0x002c_4258,
};
/// SIGN OUT warms to the legacy alert orange, because it drops the session.
const CHIP_WARM: ChipInk = ChipInk {
    border: 0xf0a0_30ff,
    fill: 0xf0a0_3014,
    text: 0x00f0_b050,
};
const CHIP_ARMED: ChipInk = ChipInk {
    border: 0xf0a0_30ff,
    fill: 0xf0a0_3038,
    text: 0x00ff_e9c0,
};
const CHIP_SEVERING: ChipInk = ChipInk {
    border: 0xf0a0_304d,
    fill: 0x0000_0000,
    text: 0x008a_6a30,
};
const ARMED_GLOW: u32 = 0xf0a0_304d;

/// the human half of a BattleTag. Battle.net owns the full value; the picker
/// omits only its numeric discriminator because this strip is identity, not an
/// address field.
pub(super) fn picker_account_name(battle_tag: Option<&str>) -> Option<String> {
    battle_tag.map(|battle_tag| strip_character_code(battle_tag).to_owned())
}

/// a chip: hairline border, 4×9 padding, a glyph and a tracked label in the
/// one colour. `lit` is the ink under the pointer; a chip with none is inert
/// and does not answer the pointer at all.
fn chip(
    id: &'static str,
    ink: ChipInk,
    lit: Option<ChipInk>,
    glyph: Option<(AnyElement, AnyElement)>,
    label: &str,
) -> Stateful<Div> {
    let group: SharedString = id.into();
    let mut chip = div()
        .id(id)
        .group(group.clone())
        .flex()
        .items_center()
        .gap(px(7.0))
        .px(px(9.0))
        .py(px(4.0))
        .border_1()
        .border_color(rgba(ink.border))
        .bg(rgba(ink.fill))
        .text_color(rgb(ink.text))
        // the bar around this is a window-drag surface. Chips own their
        // presses and must never hand them to the native window.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(lit) = lit {
        chip = chip.cursor_pointer().hover(move |style| {
            style
                .border_color(rgba(lit.border))
                .bg(rgba(lit.fill))
                .text_color(rgb(lit.text))
        });
    }
    if let Some((rest, lit_glyph)) = glyph {
        // the glyph is painted, not styled, so its lit twin rides on top and
        // shows with the rest of the chip
        chip = chip.child(
            div()
                .relative()
                .size(px(GLYPH_SIZE))
                .flex_shrink_0()
                .child(rest)
                .when(lit.is_some(), |seat| {
                    seat.child(
                        div()
                            .absolute()
                            .inset_0()
                            .opacity(0.0)
                            .group_hover(group, |style| style.opacity(1.0))
                            .child(lit_glyph),
                    )
                }),
        );
    }
    chip.child(tracked(
        label,
        CHIP_TRACKING,
        div()
            .font_family(FONT_NAVIGATION)
            .font_weight(FontWeight::BOLD)
            .text_size(px(CHIP_LABEL_SIZE))
            .whitespace_nowrap(),
    ))
}

/// the REFRESH arrow: an open circle with a head, turned by `phase` (0–1)
/// while the realms are being polled.
fn sweep_glyph(colour: u32, phase: f32) -> AnyElement {
    let scale = GLYPH_SIZE / 12.0;
    let turn = phase * std::f32::consts::TAU;
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let centre = (6.0, 6.0);
            let place = |x: f32, y: f32| {
                // rotate about the centre, then scale into the box
                let (dx, dy) = (x - centre.0, y - centre.1);
                let rx = turn.cos().mul_add(dx, -(turn.sin() * dy)) + centre.0;
                let ry = turn.sin().mul_add(dx, turn.cos() * dy) + centre.1;
                point(
                    bounds.origin.x + px(rx * scale),
                    bounds.origin.y + px(ry * scale),
                )
            };
            let mut builder = PathBuilder::stroke(px(1.5 * scale));
            // the arc: radius 4.5 from 3 o'clock, clockwise to the head
            const SWEEP: f32 = 5.436;
            const STEPS: usize = 40;
            builder.move_to(place(10.5, 6.0));
            for step in 1..=STEPS {
                let angle = SWEEP * step as f32 / STEPS as f32;
                builder.line_to(place(
                    4.5f32.mul_add(angle.cos(), 6.0),
                    4.5f32.mul_add(angle.sin(), 6.0),
                ));
            }
            builder.move_to(place(9.2, 0.8));
            builder.line_to(place(9.2, 3.4));
            builder.line_to(place(6.6, 3.4));
            if let Ok(path) = builder.build() {
                window.paint_path(path, rgb(colour));
            }
        },
    )
    .size(px(GLYPH_SIZE))
    .into_any_element()
}

/// the tick that says the poll landed.
fn check_glyph(colour: u32) -> AnyElement {
    let scale = GLYPH_SIZE / 12.0;
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let place = |x: f32, y: f32| {
                point(
                    bounds.origin.x + px(x * scale),
                    bounds.origin.y + px(y * scale),
                )
            };
            let mut builder = PathBuilder::stroke(px(1.5 * scale));
            builder.move_to(place(2.0, 6.5));
            builder.line_to(place(5.0, 9.5));
            builder.line_to(place(10.0, 2.5));
            if let Ok(path) = builder.build() {
                window.paint_path(path, rgb(colour));
            }
        },
    )
    .size(px(GLYPH_SIZE))
    .into_any_element()
}

/// Battletag and region in steel. Not a button: the plaque is context, not a
/// target. It dims while the session it names is being severed.
fn identity_plaque(name: &str, region: Option<&'static str>, dimmed: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .whitespace_nowrap()
        .font_family(FONT_INTERFACE)
        .text_size(px(9.5))
        .text_color(rgb(IDENTITY))
        .opacity(if dimmed { 0.5 } else { 1.0 })
        .child(name.to_owned())
        .children(region.map(|region| {
            div()
                .text_color(rgb(IDENTITY_REGION))
                .child(format!("· {region}"))
        }))
}

/// the REFRESH chip in whichever state the poll is in.
fn refresh_chip(
    state: RefreshChip,
    started: Option<Instant>,
    now: Instant,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    match state {
        RefreshChip::Ready => chip(
            "picker-refresh",
            CHIP_REST,
            Some(CHIP_LIT),
            Some((
                sweep_glyph(CHIP_REST.text, 0.0),
                sweep_glyph(CHIP_LIT.text, 0.0),
            )),
            "REFRESH",
        )
        .on_click(cx.listener(|this, _, _, cx| this.refresh_games(cx))),
        RefreshChip::Polling { done, total } => {
            let phase = started.map_or(0.0, |started| {
                (now.saturating_duration_since(started).as_secs_f32() / SWEEP_PERIOD.as_secs_f32())
                    .fract()
            });
            let label = format!("POLLING {done}/{total}");
            chip(
                "picker-refresh",
                CHIP_POLLING,
                None,
                Some((
                    sweep_glyph(CHIP_POLLING.text, phase),
                    sweep_glyph(CHIP_POLLING.text, phase),
                )),
                &label,
            )
        }
        RefreshChip::UpToDate => chip(
            "picker-refresh",
            CHIP_UP_TO_DATE,
            None,
            Some((
                check_glyph(CHIP_UP_TO_DATE.text),
                check_glyph(CHIP_UP_TO_DATE.text),
            )),
            "UP TO DATE",
        ),
        RefreshChip::Disabled => chip(
            "picker-refresh",
            CHIP_DISABLED,
            None,
            Some((
                sweep_glyph(CHIP_DISABLED.text, 0.0),
                sweep_glyph(CHIP_DISABLED.text, 0.0),
            )),
            "REFRESH",
        )
        .cursor(gpui::CursorStyle::OperationNotAllowed),
    }
}

/// the SIGN OUT chip. No modal: the chip arms, and a second click inside the
/// window signs out.
fn sign_out_chip(state: SignOutChip, cx: &mut Context<SuperiorityView>) -> Stateful<Div> {
    let press = cx.listener(|this, _, window, cx| {
        let now = Instant::now();
        if this.games.press_sign_out(now) {
            let name = picker_account_name(this.runtime.authoritative_battle_tag.as_deref())
                .unwrap_or_default();
            let region = this.runtime.authoritative_region;
            this.games.begin_severing(name, region, now);
            this.sign_out(window, cx);
        }
        cx.notify();
    });
    match state {
        SignOutChip::Ready => chip(
            "picker-sign-out",
            CHIP_REST,
            Some(CHIP_WARM),
            None,
            "SIGN OUT",
        )
        .on_click(press),
        SignOutChip::Armed => chip(
            "picker-sign-out",
            CHIP_ARMED,
            Some(CHIP_ARMED),
            None,
            "CONFIRM?",
        )
        .shadow(vec![
            gpui::BoxShadow::new(px(0.0), px(0.0), rgba(ARMED_GLOW).into()).blur_radius(px(10.0)),
        ])
        .on_click(press),
        SignOutChip::Severing => {
            chip("picker-sign-out", CHIP_SEVERING, None, None, "SEVERING").child(
                // the three squares the doc sets after the word
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .ml(px(2.0))
                    .children((0..3).map(|_| div().size(px(3.0)).bg(rgb(CHIP_SEVERING.text)))),
            )
        }
        SignOutChip::Inert => chip("picker-sign-out", CHIP_DISABLED, None, None, "SIGN OUT"),
    }
}

/// the top bar. `connected_region` is the numeric region Battle.net returned
/// with the authoritative SC2 logon; the label is omitted if the service did
/// not return one rather than filling it from a locale or fixture.
pub(super) fn picker_titlebar(
    battle_tag: Option<&str>,
    connected_region: Option<u32>,
    games: &GamesComponent,
    in_flight: usize,
    now: Instant,
    _window: &Window,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    #[cfg(target_os = "macos")]
    let controls = 78.0;
    #[cfg(target_os = "windows")]
    let controls = 0.0;

    let account_name = picker_account_name(battle_tag);
    let signed_in = account_name.is_some();
    // mid sign-out the plaque keeps the name Battle.net has already let go
    // of, greyed, so the bar says what is happening to whom
    let plaque = games.severing.as_ref().map_or_else(
        || {
            account_name.as_deref().map(|name| {
                identity_plaque(name, GamesComponent::region_name(connected_region), false)
            })
        },
        |severed| {
            Some(identity_plaque(
                &severed.name,
                GamesComponent::region_name(severed.region),
                true,
            ))
        },
    );

    let refresh = refresh_chip(
        games.refresh_chip(in_flight),
        games.refresh.map(|run| run.started),
        now,
        cx,
    );
    let sign_out = sign_out_chip(games.sign_out_chip(signed_in, now), cx);

    #[cfg(target_os = "windows")]
    let right_inset = platform::WINDOW_CONTROLS_WIDTH + 14.0;
    #[cfg(target_os = "macos")]
    let right_inset = 14.0;

    let header = div()
        .id("picker-titlebar")
        .relative()
        .h(px(PICKER_TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(14.0))
        .pl(px(14.0))
        .pr(px(right_inset))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgb(BAR_TOP), 0.0),
            linear_color_stop(rgb(BAR_BOTTOM), 1.0),
        ))
        .border_b_1()
        .border_color(rgb(BAR_RULE))
        // the native traffic lights, untouched: nothing of ours sits near them
        .child(div().w(px(controls)).flex_shrink_0())
        .child(div().flex_1())
        .child(refresh)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .pl(px(14.0))
                .border_l_1()
                .border_color(rgba(IDENTITY_RULE))
                .children(plaque)
                .child(sign_out),
        );
    #[cfg(target_os = "windows")]
    let header = header
        .window_control_area(WindowControlArea::Drag)
        .child(platform::window_controls(_window));
    #[cfg(target_os = "macos")]
    let header = header.on_mouse_down(MouseButton::Left, |_, window, cx| {
        cx.stop_propagation();
        platform::begin_window_drag(window);
    });
    header
}

/// one card, one state, big enough to read what changed. Clicking anywhere in
/// the section steps the walk on.
pub(super) fn hero_section(
    hero: &'static GamePalette,
    walk: Walk,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    let state = walk.to;
    let look = walk.look(hero);
    let position = state_position(state);
    div()
        .id("games-hero")
        .relative()
        .flex_shrink_0()
        .min_h(px(HERO_SECTION_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(64.0))
        .overflow_hidden()
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| {
            this.games.step();
            cx.notify();
        }))
        // the game's own light on the floor of the room, which is the only
        // thing in the section that is not the card
        .when(look.glow > 0.01, |section| {
            section.child(div().absolute().inset_0().bg(gpui::linear_gradient(
                0.0,
                gpui::linear_color_stop(rgba(fade(hero.glow, look.glow * 0.25)), 0.0),
                gpui::linear_color_stop(rgba(hero.glow & 0xffff_ff00), 0.75),
            )))
        })
        .child(hero_readout(hero, state, position))
        .child(game_card(hero, &look, HERO_CARD, art_of(hero)))
}

pub(super) fn hero_readout(hero: &'static GamePalette, state: CardState, position: usize) -> Div {
    div()
        .relative()
        .w(px(320.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(rgb(0x006e_7a8c))
                .child(format!("GAME CARD · LIVE STATES · {}", hero.title())),
        )
        .child(
            div()
                .font_family(FONT_NAVIGATION)
                .font_weight(FontWeight::BOLD)
                .text_size(px(32.0))
                .text_color(rgb(PAPER))
                .child(state.label()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(0x006e_7a8c))
                .child(format!(
                    "{} / {} · auto-cycles · click anywhere to step",
                    position + 1,
                    CARD_STATES.len()
                )),
        )
        .child(
            div()
                .pt(px(8.0))
                .flex()
                .gap(px(5.0))
                .children(CARD_STATES.iter().enumerate().map(|(index, candidate)| {
                    div().w(px(36.0)).h(px(3.0)).bg(rgb(if index == position {
                        marker_colour(*candidate, hero)
                    } else {
                        UNLIT
                    }))
                })),
        )
        .child(
            div()
                .pt(px(8.0))
                .text_size(px(10.5))
                .text_color(rgb(0x008a_96a8))
                .child(state.note()),
        )
        .child(
            div()
                .pt(px(6.0))
                .text_size(px(10.0))
                .text_color(rgb(0x005a_6478))
                .child(
                    "everything on the card derives from one palette definition per game — \
                     swap the game and the same six states re-dress themselves.",
                ),
        )
}

/// the picker those cards make. Each realm rests in the state it is actually
/// in, and the room takes the colour of whichever one you reach for — the
/// layers crossfade over each other rather than both going transparent at once.
pub(super) fn picker_section(
    games: &GamesComponent,
    size: CardSize,
    now: Instant,
    fills: bool,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    let stage = games.picker.motion.stage(now, games.reduced_motion);
    div()
        .id("games-picker")
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(44.0))
        .px(px(size.section_pad))
        .pt(px(64.0))
        .pb(px(44.0))
        .overflow_hidden()
        // as the whole screen it takes the room it is given and centres the
        // cards in it; inside the sheet it is one section among several, and
        // takes only the height it needs
        .when(fills, |section| section.flex_1().min_h_0().justify_center())
        .when(!fills, |section| {
            section.flex_shrink_0().border_t_1().border_color(rgb(RULE))
        })
        // the room is never flat: it is lit from above even with nothing
        // reached for, which is what the cards sit in rather than on
        .bg(gpui::linear_gradient(
            180.0,
            gpui::linear_color_stop(rgba(0x1014_1cff), 0.0),
            gpui::linear_color_stop(rgba(0x0507_0aff), 0.65),
        ))
        // the stage blooms out of black on the way in rather than being lit
        // before anything has arrived
        .child(
            div()
                .absolute()
                .inset_0()
                .bg(rgba(0x0406_0aff))
                .opacity(1.0 - stage.glow),
        )
        .children(
            games
                .rooms(now)
                .into_iter()
                .filter_map(|(index, light)| Some((GAMES.get(index)?, light)))
                // the room dims as the realm floods over it, so the two are
                // never both at full and fighting
                .map(|(palette, light)| {
                    room_layer(palette, light * (1.0 - stage.flood), art_of(palette))
                }),
        )
        // the chosen realm's nebula, taking the whole stage: the selector
        // becomes the realm rather than opening a door onto it
        .children(
            stage
                .flooding
                .and_then(|index| GAMES.get(index))
                .filter(|_| stage.flood > 0.001)
                .map(|palette| realm_flood(palette, stage.flood, art_of(palette))),
        )
        // the stacked lockup, scaled to a header rather than a hero: the
        // tile wears the full mark and the tagline dims to slate, so the
        // cards stay the loudest thing on screen. SELECT A GAME is gone —
        // the cards underneath already say it.
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(11.0))
                .opacity(stage.title)
                .top(px(stage.title_rise))
                .child(img("images/brand/logo-tile.png").w(px(72.0)).h(px(72.0)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(5.0))
                        .child(tracked(
                            "SUPERIORITY",
                            8.0,
                            div()
                                .font_family(FONT_NAVIGATION)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(16.0))
                                .text_color(rgb(PAPER)),
                        ))
                        .child(tracked(
                            "UPLINK ESTABLISHED",
                            3.5,
                            div().text_size(px(9.0)).text_color(rgb(0x008f_a2ba)),
                        )),
                ),
        )
        .child(card_row(games, size, now, cx))
        // one quiet line says what the keyboard does, instead of every card
        // shouting its own glyph
        .child(
            div()
                .relative()
                .flex()
                .justify_center()
                .gap(px(10.0))
                .opacity(stage.title)
                .text_size(px(9.0))
                .text_color(rgb(0x005a_6478))
                .child("← → SELECT")
                .child("·")
                .child("⏎ ENTER")
                .child("·")
                .child("1 2 3 JUMP"),
        )
        .children((stage.entering > 0.001).then(|| entering_overlay(games, &stage)))
}

/// the cards themselves. The row wraps, so a window too narrow for three
/// abreast stacks them instead of pushing them off the edge.
pub(super) fn card_row(
    games: &GamesComponent,
    size: CardSize,
    now: Instant,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    div()
        .id("games-row")
        .relative()
        .flex()
        .flex_wrap()
        .items_stretch()
        .justify_center()
        .gap(px(size.gap_between))
        // leaving the row drops the lift, not the choice: the room stays where
        // the pointer left it rather than going dark behind you
        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
            if !hovered && this.games.reach(None) {
                cx.notify();
            }
        }))
        .children(
            GAMES
                .iter()
                .enumerate()
                .filter(|(index, _)| games.visible(*index))
                .map(|(index, palette)| {
                    let motion = games.card_motion(index, size, now);
                    // the one being entered holds its slot and shows nothing: the copy
                    // above the row is the one doing the travelling
                    let held = CardMotion {
                        opacity: if games.entering() == Some(index) {
                            0.0
                        } else {
                            motion.opacity
                        },
                        glide: 0.0,
                        scale: 1.0,
                        ..motion
                    };
                    picker_card(index, palette, games, size, &held, now, cx)
                }),
        )
        .children((games.visible_count() == 0).then(|| {
            let message = if games.owned.is_none() {
                "SYNCING YOUR BATTLE.NET LIBRARY"
            } else {
                "NO SUPPORTED GAMES ON THIS ACCOUNT"
            };
            div()
                .w(px(420.0))
                .py(px(34.0))
                .flex()
                .justify_center()
                .border_1()
                .border_color(rgb(RULE))
                .font_family(FONT_NAVIGATION)
                .text_size(px(12.0))
                .text_color(rgb(0x006e_7a8c))
                .child(message)
        }))
        // the card being entered is drawn again, over the row, so that growing
        // it does not shove its neighbours sideways: in the flow it holds its
        // slot and shows nothing, and the copy above does the travelling
        .children(
            games
                .entering()
                .and_then(|index| Some((index, GAMES.get(index)?)))
                .map(|(index, palette)| leaving_card(index, palette, games, size, now, cx)),
        )
}

/// the winner, lifted out of the row. It starts exactly where its slot is, so
/// the moment it appears nothing has visibly changed, and from there it glides
/// to the middle and grows into the flood.
pub(super) fn leaving_card(
    index: usize,
    palette: &'static GamePalette,
    games: &GamesComponent,
    size: CardSize,
    now: Instant,
    cx: &mut Context<SuperiorityView>,
) -> Div {
    let motion = games.card_motion(index, size, now);
    let grown = size.width * motion.scale;
    // growing from the middle rather than the left edge, which is where the eye
    // is already looking
    let resting = index_offset(index, size, games);
    div()
        .absolute()
        .top(px(size.art.mul_add(-(motion.scale - 1.0), 0.0) / 2.0))
        .left(px(motion.glide + resting - (grown - size.width) / 2.0))
        .opacity(motion.opacity)
        // the outer frame carries where it is; the card itself carries only
        // how big it has grown
        .child(picker_card(
            index,
            palette,
            games,
            size,
            &CardMotion {
                opacity: 1.0,
                offset: 0.0,
                glide: 0.0,
                ..motion
            },
            now,
            cx,
        ))
}

/// where a card's slot starts, measured from the left of the row.
pub(super) fn index_offset(index: usize, size: CardSize, games: &GamesComponent) -> f32 {
    let index = u16::try_from(games.visible_position(index)).unwrap_or(u16::MAX);
    f32::from(index) * (size.width + size.gap_between)
}

/// how far this card sits from the middle of the row, in pixels — which is how
/// far it travels when it is the one being entered.
/// Where a card sits in the row.
///
/// Measured across the cards actually drawn, not across `GAMES`: a hidden game
/// still has an entry, and counting it put the row off centre by half a slot.
pub(super) fn slot_offset(index: usize, size: CardSize, games: &GamesComponent) -> f32 {
    let count = games.visible_count();
    let position = games.visible_position(index);
    let middle = (count as f32 - 1.0) / 2.0;
    (middle - position as f32) * (size.width + size.gap_between)
}

/// the picker exactly as the handoff left it: the flood at full, ENTERING with
/// its rail run out, and the top bar still in place. The realm fades in over
/// this rather than over the window's bare ground.
pub(super) fn afterglow_plate(
    games: &GamesComponent,
    card: usize,
    palette: &'static GamePalette,
    titlebar: impl IntoElement,
) -> Div {
    let stage = StageMotion {
        title: 0.0,
        title_rise: 0.0,
        glow: 1.0,
        flood: 1.0,
        flooding: Some(card),
        entering: 1.0,
        progress: 1.0,
    };
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .bg(rgb(SHEET))
        .child(titlebar)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                // the same room the section lights, so the plate and the frame
                // before it are one image
                .bg(gpui::linear_gradient(
                    180.0,
                    gpui::linear_color_stop(rgba(0x1014_1cff), 0.0),
                    gpui::linear_color_stop(rgba(0x0507_0aff), 0.65),
                ))
                .child(realm_flood(palette, 1.0, art_of(palette)))
                .child(entering_overlay(games, &stage)),
        )
}

/// what is left once the card has dissolved into the flood: where you are
/// going, and how far along it is.
pub(super) fn entering_overlay(games: &GamesComponent, stage: &StageMotion) -> Div {
    let palette = stage
        .flooding
        .and_then(|index| GAMES.get(index))
        .unwrap_or(&GAMES[0]);
    let _ = games;
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.0))
        .opacity(stage.entering)
        .child(
            div()
                .font_family(FONT_NAVIGATION)
                .font_weight(FontWeight::BOLD)
                .text_size(px(24.0))
                .text_color(rgb(palette.text))
                .child("ENTERING"),
        )
        .child(
            div()
                .flex()
                .gap(px(6.0))
                .text_size(px(12.0))
                .text_color(rgb(palette.dim))
                .child(format!("{} ·", palette.title_lower()))
                .children(
                    (!games.live_mode)
                        .then(|| div().text_color(rgb(palette.ok)).child(palette.channel)),
                ),
        )
        .child(
            div().w(px(220.0)).h(px(2.0)).bg(rgba(0x133e_5b99)).child(
                div()
                    .h(px(2.0))
                    .w(relative(stage.progress))
                    .bg(rgb(palette.focus))
                    .shadow(scope_glow(palette.glow)),
            ),
        )
}

/// the same state on all three palettes at once, which is where the claim that
/// only the palette changes is either true or visibly not.
pub(super) fn together_section(walk: Walk, cx: &mut Context<SuperiorityView>) -> Stateful<Div> {
    div()
        .id("games-together")
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(18.0))
        .px(px(20.0))
        .pt(px(28.0))
        .pb(px(32.0))
        .border_t_1()
        .border_color(rgb(RULE))
        .bg(rgb(0x0005_070a))
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| {
            this.games.step();
            cx.notify();
        }))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(0x006e_7a8c))
                .child(
                    "TOGETHER — the same state, all three palettes in sync. \
                     one card definition; only the palette changes.",
                ),
        )
        .child(
            div()
                .flex()
                .justify_center()
                .gap(px(28.0))
                .children(GAMES.iter().map(|palette| {
                    game_card(palette, &walk.look(palette), TRIO_CARD, art_of(palette))
                })),
        )
}

/// every value the card reads, per game.
pub(super) fn palette_section() -> Div {
    div()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .p(px(20.0))
        .border_t_1()
        .border_color(rgb(RULE))
        .bg(rgb(SHEET))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(0x006e_7a8c))
                .child(
                    "PALETTES — every value the card reads, per game. behaviour and \
                     structure never change; only these tokens do.",
                ),
        )
        .child(
            div()
                .flex()
                .gap(px(18.0))
                .children(GAMES.iter().map(palette_panel)),
        )
}

pub(super) fn palette_panel(palette: &'static GamePalette) -> Div {
    div()
        .w(px(300.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(14.0))
        .border_1()
        .border_color(rgb(0x001a_2430))
        .child(
            div()
                .pb(px(4.0))
                .font_family(palette.font)
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .text_color(rgb(palette.text))
                .child(palette.dressing),
        )
        .children(palette.reference().map(|(line, colour)| {
            div()
                .text_size(px(10.5))
                .text_color(rgb(colour))
                .child(line)
        }))
}

/// the picker's card. It carries the three lines that say who you are here and
/// when you last were, where the walked card carries one line about the state
/// it is demonstrating.
pub(super) fn picker_card(
    index: usize,
    palette: &'static GamePalette,
    games: &GamesComponent,
    size: CardSize,
    motion: &CardMotion,
    now: Instant,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    let owned = games.owns(index);
    let actionable = games.actionable(index);
    let size = CardSize {
        width: size.width * motion.scale,
        art: size.art * motion.scale,
        ..size
    };
    let state = games.card[index];
    let data = card_data(games, index, state);
    let mut look = CardLook::resolve(
        palette,
        games.card_from[index],
        state,
        games.card_since[index],
        now,
    );
    if !owned {
        look.out_of_play(palette);
    }
    if games.live_mode {
        look.live();
    }
    let chosen = actionable && games.picker.selected == index;
    // in the picker the bloom belongs to the pointer, not to the state: a card
    // that lit itself for being connected would glow all the time, and then a
    // glow would stop meaning "this one".
    // only the focused card carries the return key and the primary verb; the
    // others keep their full light — a card that dimmed read as disabled
    card_shell(palette, &look, size, 0.0)
        .id(("game-card", index))
        .relative()
        .opacity(look.card_light * motion.opacity)
        .top(px(motion.offset))
        .left(px(motion.glide))
        .when(actionable && !chosen, |card| {
            card.border_color(rgba(fade(look.border << 8, 0.45)))
        })
        // a game the account does not have is shown and not offered: no
        // pointer, no lift, nothing that answers
        .when(actionable, |card| {
            card.cursor_pointer()
                // the choice wears the edge and the glow — it stays put when
                // the pointer wanders off
                .when(chosen && motion.lit, |card| {
                    card.border_color(rgb(palette.focus))
                        .shadow(scope_glow(palette.glow))
                })
                // and the card under the pointer lifts, whichever card that is
                .hover(move |style| {
                    style
                        .border_color(rgb(palette.focus))
                        .shadow(scope_glow(palette.glow))
                })
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if this.games.reach(hovered.then_some(index)) {
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.take_game_card(index, Instant::now(), cx);
                    cx.notify();
                }))
        })
        .child(card_art(palette, &look, size, art_of(palette)))
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(9.0))
                .px(px(size.pad_x))
                .pt(px(12.0))
                .pb(px(size.pad_bottom))
                // two lines, destination first: "where will Enter take me" is
                // the most actionable fact on the card, so it leads and
                // carries the accent
                .child(destination_line(palette, &data, &look, size))
                .when(owned, |lines| {
                    lines.child(identity_region_line(palette, &data, size))
                })
                .child(progress_rail(palette, &look))
                .child(card_button(
                    palette,
                    &look,
                    size,
                    out_of_play_verb(games, index),
                    chosen,
                )),
        )
}

/// what the button says on a card that cannot be pressed. The design's empty
/// state offers SIGN IN, which is right for a game you own and have no profile
/// on — and wrong for both of the reasons a card is dark here.
pub(super) fn out_of_play_verb(games: &GamesComponent, index: usize) -> Option<&'static str> {
    if games.owns(index) {
        return None;
    }
    Some(if games.actionable(index) {
        "SIGN IN"
    } else if games.licensed(index) {
        "SOON"
    } else {
        "NOT OWNED"
    })
}

pub(super) fn state_position(state: CardState) -> usize {
    CARD_STATES
        .iter()
        .position(|candidate| *candidate == state)
        .unwrap_or(0)
}

pub(super) fn card_index(product: Product) -> Option<usize> {
    GAMES
        .iter()
        .position(|game| Product::from_code(game.program) == Some(product))
}

pub(super) fn card_product(index: usize) -> Option<Product> {
    GAMES
        .get(index)
        .and_then(|game| Product::from_code(game.program))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        CARD_STATES, CardState, GamesComponent, GamesScreen, STATE_DWELL, picker_account_name,
        picker_size,
    };

    #[test]
    fn picker_identity_hides_only_the_battletag_discriminator() {
        assert_eq!(
            picker_account_name(Some("ncarrillo#1369")),
            Some("ncarrillo".to_owned())
        );
        assert_eq!(
            picker_account_name(Some("Account Without Tag")),
            Some("Account Without Tag".to_owned())
        );
        assert_eq!(picker_account_name(None), None);
    }

    #[test]
    fn the_room_crosses_over_rather_than_dipping_through_the_dark() {
        use superiority_ui::products::games::ROOM;
        let now = Instant::now();
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), None, false);

        // there is always a room, and it starts on the first game
        assert_eq!(games.picker.selected, 0);
        let resting = games.rooms(now);
        assert_eq!(resting.len(), 1);
        assert_eq!(resting[0].0, 0);

        // every implemented realm can take the room with it
        games.reach(Some(2));
        assert_eq!(games.picker.hovered, Some(2));
        assert_eq!(games.picker.selected, 2);

        // one room goes out as the other comes in, and the two always sum to
        // one: no double-exposure while it runs, and nothing to pop at the end
        games.picker.selected = 0;
        games.picker.previously_selected = 1;
        games.picker.chosen = now;
        let midway = games.rooms(now + ROOM / 2);
        assert_eq!(midway.len(), 2);
        assert_eq!(midway[0].0, 1);
        assert_eq!(midway[1].0, 0);
        assert!(
            (midway[0].1 + midway[1].1 - 1.0).abs() < 0.001,
            "the two rooms must sum to one, got {midway:?}"
        );

        // and the outgoing room is already at zero when it is dropped
        let nearly = games.rooms(now + ROOM * 99 / 100);
        assert!(nearly[0].1 < 0.05, "outgoing should be spent: {nearly:?}");
        let landed = games.rooms(now + ROOM + Duration::from_millis(1));
        assert_eq!(landed.len(), 1);
        assert_eq!(landed[0].0, 0);

        // taking the pointer away keeps the choice: the room never goes dark
        games.reach(None);
        assert_eq!(games.picker.hovered, None);
        assert_eq!(games.picker.selected, 0);
        assert_eq!(games.rooms(now).last().map(|room| room.0), Some(0));
    }

    #[test]
    fn the_row_is_centred_on_the_cards_it_actually_draws() {
        // a hidden middle game keeps its catalogue entry, but the two licensed
        // cards must still balance around the real centre of the drawn row.
        let size = super::HERO_CARD;
        let games = GamesComponent::new(
            Some(GamesScreen::Picker),
            Some(vec!["S2".to_owned(), "W3".to_owned()]),
            false,
        );
        let shown: Vec<f32> = (0..super::GAMES.len())
            .filter(|index| games.visible(*index))
            .map(|index| super::slot_offset(index, size, &games))
            .collect();
        assert!(!shown.is_empty());
        let sum: f32 = shown.iter().sum();
        assert!(
            sum.abs() < 0.001,
            "the drawn cards must balance about the centre, got {shown:?}"
        );
    }

    #[test]
    fn the_row_gives_up_width_before_it_gives_up_its_row() {
        let same = |left: f32, right: f32| (left - right).abs() < f32::EPSILON;
        // three abreast where there is room for three
        assert!(same(picker_size(1400.0).width, super::PICKER_WIDE.width));
        // narrower cards while two still fit
        assert!(same(picker_size(900.0).width, super::PICKER_MEDIUM.width));
        // and stacked once even two will not
        assert!(same(picker_size(500.0).width, super::PICKER_NARROW.width));
        // the gutter closes up as the window does
        assert!(picker_size(500.0).section_pad < picker_size(1400.0).section_pad);
    }

    #[test]
    fn a_card_connects_lands_and_can_be_taken_again() {
        let now = Instant::now();
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        // a realm you are already connected to has nowhere to go back to: it
        // rests connected, so taking it leaves it exactly there
        games.enter(0);
        assert_eq!(games.card[0], CardState::Connected);

        // from a cold card, entering starts a connection that lands on its own
        games.set_card(0, CardState::Idle);
        games.enter(0);
        assert_eq!(games.card[0], CardState::Connecting);
        assert!(!games.land_connections(now));

        games.card_since[0] = now
            .checked_sub(super::CONNECT_TIME + Duration::from_millis(1))
            .expect("clock reaches back");
        assert!(games.land_connections(now));
        assert_eq!(games.card[0], CardState::Connected);
    }

    #[test]
    fn a_game_the_authoritative_account_does_not_have_is_hidden_and_inert() {
        let owned = Some(vec!["W3".to_owned()]);
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), owned, false);

        assert!(!games.licensed(0), "the account does not have it");
        assert!(!games.owns(0));
        // it opens in the design's empty state rather than in an offer
        assert_eq!(games.card[0], CardState::NoAccount);

        assert!(!games.visible(0));
        assert!(!games.actionable(0));
        assert!(!games.choose(0));
        assert!(!games.reach(Some(0)));
        assert_eq!(games.picker.hovered, None);
        games.enter(0);
        assert_eq!(games.card[0], CardState::NoAccount);
        assert_eq!(super::out_of_play_verb(&games, 0), Some("NOT OWNED"));
    }

    #[test]
    fn only_a_game_with_a_protocol_behind_it_is_offered() {
        // every drawn product now has a protocol implementation behind it.
        let all = Some(vec!["S2".to_owned(), "S1".to_owned(), "W3".to_owned()]);
        let games = GamesComponent::new(Some(GamesScreen::Picker), all, false);

        assert!(games.owns(0), "starcraft ii speaks its native channel");
        assert!(games.owns(1), "remastered speaks the classic one");
        assert!(games.licensed(2), "the account has warcraft iii");
        assert!(games.owns(2), "reforged speaks the wc3 classic channel");
        assert_eq!(games.card[2], CardState::Idle);
    }

    #[test]
    fn saying_nothing_about_ownership_offers_everything_it_can() {
        // design-time screens with no account catalogue demonstrate everything.
        let games = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        assert!((0..super::GAMES.len()).all(|index| games.licensed(index)));
        assert!(games.owns(0));

        // a live client waits for the authoritative license response and does
        // not flash unprovisioned products while ownership is unknown.
        let mut live = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        live.live_mode = true;
        assert_eq!(live.visible_count(), 0);
    }

    #[test]
    fn a_card_only_reads_the_session_that_belongs_to_it() {
        // showing every card the one live session said that all three realms
        // were in whatever channel StarCraft II happened to be in
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        games.live.insert(
            crate::Product::StarCraft2,
            super::LiveGame {
                state: CardState::Connected,
                channel: Some("Arcade".to_owned()),
                online: Some(116),
                ..super::LiveGame::default()
            },
        );

        assert!(games.live_for(0).is_some(), "the game it belongs to");
        // Remastered is playable too now, so this is no longer true by accident
        // — it is true because the session is keyed by product
        assert!(games.live_for(1).is_none(), "and no other");
        assert!(games.live_for(2).is_none());
    }

    #[test]
    fn a_dark_card_says_which_kind_of_dark_it_is() {
        // all implemented products stay active when the account owns them.
        let owns_all = GamesComponent::new(
            Some(GamesScreen::Picker),
            Some(vec!["S2".to_owned(), "S1".to_owned(), "W3".to_owned()]),
            false,
        );
        assert_eq!(super::out_of_play_verb(&owns_all, 2), None);
        assert_eq!(super::out_of_play_verb(&owns_all, 0), None);
        assert_eq!(super::out_of_play_verb(&owns_all, 1), None);

        // a product the authoritative account does not own is not actionable.
        let owns_none = GamesComponent::new(Some(GamesScreen::Picker), Some(Vec::new()), false);
        assert_eq!(super::out_of_play_verb(&owns_none, 2), Some("NOT OWNED"));
    }

    #[test]
    fn replacing_the_authoritative_catalogue_moves_selection_to_a_visible_game() {
        let mut games = GamesComponent::new(
            Some(GamesScreen::Picker),
            Some(vec!["S2".to_owned()]),
            false,
        );
        assert_eq!(games.picker.selected, 0);

        games.set_owned(vec!["W3".to_owned()]);

        assert!(!games.visible(0));
        assert!(games.visible(2));
        assert_eq!(games.picker.selected, 2);
        assert_eq!(games.visible_count(), 1);
    }

    #[test]
    fn taking_a_card_does_the_one_thing_its_verb_says() {
        // this is the whole reason the pointer and the keyboard share a path:
        // a click that ran the connect toggle over a connected card mapped it
        // to itself and did nothing at all
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        games.picker.motion = super::Motion::Ready;
        assert_eq!(games.card[0], CardState::Connected);

        games.begin_entering(0, Instant::now());
        assert_eq!(games.entering(), Some(0));
        assert!(!games.answering(), "and nothing else answers while it runs");
    }

    #[test]
    fn the_handoff_keeps_the_last_frame_under_the_realm_and_then_lets_go() {
        // the realm fades in over the flood it dissolved into; without this the
        // picker drew one settled frame of the list, then the bare window, and
        // only then the realm
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        games.picker.motion = super::Motion::Ready;
        let start = Instant::now();
        games.begin_entering(0, start);
        assert!(!games.has_entered());
        assert_eq!(games.afterglow(start), None);

        let handoff = start + superiority_ui::products::games::motion::entering_length();
        assert!(games.advance_motion(handoff));
        assert!(games.has_entered());
        assert_eq!(
            games.afterglow(handoff),
            Some(0),
            "the flood stays while the realm reveals"
        );
        let later = handoff + std::time::Duration::from_secs(2);
        assert_eq!(games.afterglow(later), None, "and is gone once it has");

        games.return_to_list(later);
        assert!(!games.has_entered());
        assert_eq!(games.afterglow(later), None);
    }

    #[test]
    fn the_keyboard_never_lands_on_a_card_that_answers_to_nothing() {
        let mut games = GamesComponent::new(Some(GamesScreen::Picker), None, false);
        // the choice starts on the first playable game and moves between the
        // playable ones
        assert_eq!(games.picker.selected, 0);
        games.move_choice(-1);
        assert_eq!(
            games.picker.selected, 0,
            "there is nothing to the left of the first"
        );
        games.move_choice(1);
        assert_eq!(games.picker.selected, 1, "remastered answers now");

        // Warcraft III is the third playable realm and the row still clamps
        // at its ends.
        games.move_choice(1);
        assert_eq!(games.picker.selected, 2);
        games.move_choice(9);
        assert_eq!(games.picker.selected, 2);
    }

    #[test]
    fn the_walk_dwells_then_steps_and_comes_back_round() {
        let mut games = GamesComponent::new(Some(GamesScreen::States), None, false);
        assert_eq!(games.state, CardState::Idle);

        // a state that has only just arrived is not due to leave
        assert!(!games.advance_if_due());
        assert_eq!(games.state, CardState::Idle);

        games.advanced = Instant::now()
            .checked_sub(STATE_DWELL + Duration::from_millis(1))
            .expect("the clock must reach back one dwell");
        assert!(games.advance_if_due());
        assert_eq!(games.state, CardState::Focused);
        // and stepping resets the clock, so each state gets its full dwell
        assert!(!games.advance_if_due());

        // the walk is a loop, not a line
        for _ in 1..CARD_STATES.len() {
            games.step();
        }
        assert_eq!(games.state, CardState::Idle);
    }
}
