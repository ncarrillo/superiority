//! The resolved look of a card mid-transition. A [`CardLook`] is every number
//! the renderer paints already worked out, so the view decides nothing. It is
//! generic over [`AnimationClock`] so the desktop drives it with `Instant` and
//! the browser with `f64` milliseconds.

use std::time::Duration;

use gpui::ease_in_out;

use crate::foundation::animation::AnimationClock;

use super::model::{CardState, GamePalette};

/// The doc gives every property its own clock: the border settles in 600ms
/// while the art is still travelling for 1400, and the handshake bar crawls for
/// two full seconds. Keeping them apart is most of what makes a state change
/// read as one thing moving rather than six things snapping.
const EDGE: Duration = Duration::from_millis(600);
const ART_LIGHT: Duration = Duration::from_millis(900);
const ART_TRAVEL: Duration = Duration::from_millis(1400);
const MARK: Duration = Duration::from_millis(600);
const BEAM: Duration = Duration::from_millis(400);
const HANDSHAKE: Duration = Duration::from_millis(2000);
/// The room behind the picker takes its time, because it is the biggest thing
/// on screen and the pointer only brushed a card.
pub const ROOM: Duration = Duration::from_millis(700);

/// A card for a game the account does not have is not in a state — it is out of
/// play. The design's empty state was drawn for somebody who has the game and
/// no profile on it, so on its own it still reads as an offer: colour in the
/// bevel, a card at three quarters, art you can make out. These take it the
/// rest of the way down.
const OUT_OF_PLAY_CARD: f32 = 0.4;
const OUT_OF_PLAY_ART: f32 = 0.18;
const OUT_OF_PLAY_BUTTON: f32 = 0.35;

/// How far a transition of `duration` has got since it started, eased the way
/// the doc eases everything.
#[must_use]
pub fn eased<C: AnimationClock>(since: C, duration: Duration, now: C) -> f32 {
    let elapsed = now.elapsed(since).as_secs_f32();
    let span = duration.as_secs_f32();
    if span <= 0.0 {
        return 1.0;
    }
    ease_in_out((elapsed / span).clamp(0.0, 1.0))
}

fn lerp(from: f32, to: f32, blend: f32) -> f32 {
    to.mul_add(blend, from * (1.0 - blend))
}

/// Colours cross over channel by channel. They are `0x00RRGGBB` here, which is
/// what `rgb` reads, so the alpha byte is nobody's business.
fn mix(from: u32, to: u32, blend: f32) -> u32 {
    let channel = |shift: u32| {
        let from = f32::from(u8::try_from((from >> shift) & 0xff).unwrap_or(u8::MAX));
        let to = f32::from(u8::try_from((to >> shift) & 0xff).unwrap_or(u8::MAX));
        let mixed = lerp(from, to, blend).round().clamp(0.0, 255.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to a byte immediately above"
        )]
        let mixed = mixed as u32;
        mixed << shift
    };
    channel(16) | channel(8) | channel(0)
}

/// A card mid-transition: every number already resolved, so the view paints
/// what it is given rather than deciding anything.
pub struct CardLook {
    pub border: u32,
    pub glow: f32,
    pub card_light: f32,
    pub art_light: f32,
    pub art_overhang: f32,
    pub colourless: bool,
    pub title: u32,
    pub status: String,
    pub status_tag: Option<&'static str>,
    pub status_colour: u32,
    pub badge: f32,
    pub ember: f32,
    pub cold: f32,
    pub scan: f32,
    pub progress: f32,
    pub progress_rail: f32,
    pub button_light: f32,
    pub button_colour: u32,
    pub verb: &'static str,
    pub return_key: f32,
}

impl CardLook {
    /// Takes a resolved card out of play: everything dimmer than any state can
    /// be, no colour left anywhere, and the art barely there. Nothing about a
    /// card in this condition should look like it is waiting to be pressed.
    /// Drops everything the palette made up.
    ///
    /// [`GamePalette::status`] builds its line from the design-time handle,
    /// region, ping, channel, and handshake, which is what the design screens
    /// want and what a live card must never show — those strings read as a real
    /// account in a real channel. A live card says what its session says, or
    /// says nothing.
    pub fn live(&mut self) {
        self.status = String::new();
        self.status_tag = None;
    }

    pub fn out_of_play(&mut self, palette: &GamePalette) {
        self.card_light = OUT_OF_PLAY_CARD;
        self.art_light = OUT_OF_PLAY_ART;
        self.colourless = true;
        self.glow = 0.0;
        self.border = palette.structural_dim;
        self.title = palette.sub;
        self.status_colour = palette.sub;
        self.button_light = OUT_OF_PLAY_BUTTON;
        self.button_colour = palette.sub;
        self.badge = 0.0;
        self.ember = 0.0;
        self.cold = 0.0;
        self.scan = 0.0;
        self.progress_rail = 0.0;
        self.return_key = 0.0;
    }

    /// Resolves the card between the state it was in and the state it is going
    /// to. At the start it is exactly the old state and at the end exactly the
    /// new one, which is what keeps a transition from inventing a look of its
    /// own.
    #[must_use]
    pub fn resolve<C: AnimationClock>(
        palette: &GamePalette,
        from: CardState,
        to: CardState,
        since: C,
        now: C,
    ) -> Self {
        let edge = eased(since, EDGE, now);
        let art_light = eased(since, ART_LIGHT, now);
        let travel = eased(since, ART_TRAVEL, now);
        let mark = eased(since, MARK, now);
        let beam = eased(since, BEAM, now);
        let handshake = eased(since, HANDSHAKE, now);
        // words and greyscale have nothing to cross through, so they belong to
        // whichever state is current the moment it becomes current
        Self {
            border: mix(palette.border(from), palette.border(to), edge),
            glow: lerp(
                f32::from(u8::from(from.lit())),
                f32::from(u8::from(to.lit())),
                edge,
            ),
            card_light: lerp(from.card_light(), to.card_light(), edge),
            art_light: lerp(from.art_light(), to.art_light(), art_light),
            art_overhang: lerp(from.art_overhang(), to.art_overhang(), travel),
            colourless: to.colourless(),
            title: mix(palette.title_colour(from), palette.title_colour(to), edge),
            status: palette.status(to),
            status_tag: palette.status_tag(to),
            status_colour: mix(palette.status_colour(from), palette.status_colour(to), edge),
            badge: lerp(mark_of(from.badged()), mark_of(to.badged()), mark),
            ember: lerp(mark_of(from.ember()), mark_of(to.ember()), mark),
            cold: lerp(mark_of(from.ember()), mark_of(to.ember()), art_light),
            scan: lerp(mark_of(from.scanning()), mark_of(to.scanning()), beam),
            progress: lerp(
                from.progress().unwrap_or(0.05),
                to.progress().unwrap_or(0.05),
                handshake,
            ),
            progress_rail: lerp(
                mark_of(from.progress().is_some()),
                mark_of(to.progress().is_some()),
                mark,
            ),
            button_light: lerp(button_light(from), button_light(to), edge),
            button_colour: mix(
                button_colour(palette, from),
                button_colour(palette, to),
                edge,
            ),
            verb: to.verb(),
            return_key: lerp(
                mark_of(from.shows_return_key()),
                mark_of(to.shows_return_key()),
                mark,
            ),
        }
    }
}

fn mark_of(shown: bool) -> f32 {
    f32::from(u8::from(shown))
}

fn button_light(state: CardState) -> f32 {
    if state.lit() { 1.0 } else { 0.55 }
}

const fn button_colour(palette: &GamePalette, state: CardState) -> u32 {
    if state.lit() {
        palette.text
    } else {
        palette.dim
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::super::model::{CARD_STATES, CardState, GAMES};
    use super::{CardLook, eased, mix};

    #[test]
    fn a_transition_starts_at_the_old_state_and_ends_at_the_new_one() {
        let palette = &GAMES[0];
        let now = Instant::now();
        let opening = CardLook::resolve(palette, CardState::Idle, CardState::Focused, now, now);
        // nothing has moved yet, so the card is still exactly resting
        assert!((opening.art_light - CardState::Idle.art_light()).abs() < f32::EPSILON);
        assert_eq!(opening.border, palette.border(CardState::Idle));

        let landed = CardLook::resolve(
            palette,
            CardState::Idle,
            CardState::Focused,
            now.checked_sub(Duration::from_secs(4))
                .expect("clock reaches back"),
            now,
        );
        assert!((landed.art_light - CardState::Focused.art_light()).abs() < f32::EPSILON);
        assert_eq!(landed.border, palette.border(CardState::Focused));
        // and the words are the new state's from the first frame, because a
        // sentence has nothing to cross through
        assert_eq!(opening.status, palette.status(CardState::Focused));
    }

    #[test]
    fn a_card_out_of_play_is_dimmer_than_any_state_it_could_be_in() {
        let palette = &GAMES[0];
        let now = Instant::now();
        let mut out = CardLook::resolve(
            palette,
            CardState::NoAccount,
            CardState::NoAccount,
            now,
            now,
        );
        out.out_of_play(palette);

        for state in CARD_STATES {
            let resting = CardLook::resolve(palette, state, state, now, now);
            assert!(
                out.card_light < resting.card_light,
                "out of play is not dimmer than {state:?}"
            );
            assert!(
                out.art_light < resting.art_light,
                "out of play art is not dimmer than {state:?}"
            );
            assert!(
                out.button_light < resting.button_light,
                "out of play button is not dimmer than {state:?}"
            );
        }

        assert!(out.colourless);
        assert!(out.glow.abs() < f32::EPSILON);
        assert!(out.badge.abs() < f32::EPSILON);
        assert!(out.scan.abs() < f32::EPSILON);
        assert!(out.return_key.abs() < f32::EPSILON);
    }

    #[test]
    fn colours_cross_over_rather_than_cutting() {
        assert_eq!(mix(0x0000_0000, 0x00ff_ffff, 0.0), 0x0000_0000);
        assert_eq!(mix(0x0000_0000, 0x00ff_ffff, 1.0), 0x00ff_ffff);
        // halfway is halfway in every channel, not a jump at the midpoint
        assert_eq!(mix(0x0000_0000, 0x00ff_ffff, 0.5), 0x0080_8080);
        assert_eq!(mix(0x0033_a8f0, 0x0033_a8f0, 0.5), 0x0033_a8f0);
    }

    #[test]
    fn the_clock_is_bounded_at_both_ends() {
        let now = Instant::now();
        assert!(eased(now, Duration::from_millis(600), now).abs() < f32::EPSILON);
        let long_ago = now
            .checked_sub(Duration::from_secs(10))
            .expect("clock reaches back");
        assert!((eased(long_ago, Duration::from_millis(600), now) - 1.0).abs() < f32::EPSILON);
        // a property with no duration is simply already there
        assert!((eased(now, Duration::ZERO, now) - 1.0).abs() < f32::EPSILON);
    }
}
