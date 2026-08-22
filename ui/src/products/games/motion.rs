//! The picker's choreography — the entrance, and entering a realm — as pure
//! motion over an [`AnimationClock`], so the desktop drives it with `Instant`
//! and the browser with `f64` milliseconds and both play the same sequence.

use std::time::Duration;

use crate::foundation::animation::{AnimationClock, cubic_bezier};

/// The entrance plays once per launch. Its beats, from the motion sheet: the
/// stage blooms and the title settles, then the cards rise left to right, and
/// the card you can already press lights last.
const TITLE_AT: Duration = Duration::from_millis(120);
const FIRST_CARD_AT: Duration = Duration::from_millis(350);
const CARD_STAGGER: Duration = Duration::from_millis(130);
const LIT_AT: Duration = Duration::from_millis(1100);
const TITLE_SETTLE: Duration = Duration::from_millis(700);
const CARD_FADE: Duration = Duration::from_millis(500);
const CARD_TRAVEL: Duration = Duration::from_millis(700);
const TITLE_RISE: f32 = 14.0;
const CARD_RISE: f32 = 28.0;

/// Entering a realm: the unchosen cards lose first, then the chosen one glides
/// to the middle and grows while its nebula floods the stage, and finally it
/// dissolves into that flood.
const LOSERS_AT: Duration = Duration::ZERO;
const CENTRE_AT: Duration = Duration::from_millis(250);
const DISSOLVE_AT: Duration = Duration::from_millis(900);
const LOSER_FALL: f32 = 30.0;
const FLOOD_FADE: Duration = Duration::from_millis(800);
const DISSOLVE: Duration = Duration::from_millis(600);
const ENTER_PROGRESS: Duration = Duration::from_millis(900);
const CHOSEN_SCALE: f32 = 1.25;
const SNAP_BACK: Duration = Duration::from_millis(200);

/// `cubic-bezier(0.16, 1, 0.3, 1)`: fast in, long settle, nothing bounces.
#[must_use]
pub fn settle(fraction: f32) -> f32 {
    cubic_bezier(0.16, 1.0, 0.3, 1.0, fraction)
}

/// Where a beat that starts at `at` and runs for `over` has got to.
fn beat(elapsed: Duration, at: Duration, over: Duration) -> f32 {
    let Some(since) = elapsed.checked_sub(at) else {
        return 0.0;
    };
    let span = over.as_secs_f32();
    if span <= 0.0 {
        return 1.0;
    }
    settle((since.as_secs_f32() / span).clamp(0.0, 1.0))
}

/// What the picker is in the middle of. Generic over the clock so a host can
/// drive it with whatever time source it has.
#[derive(Clone, Copy)]
pub enum Motion<C> {
    /// Built, but not yet on screen. The entrance is timed from the first frame.
    Waiting,
    Entrance {
        started: C,
    },
    Ready,
    Entering {
        card: usize,
        started: C,
    },
    SnappingBack {
        from: f32,
        started: C,
    },
}

/// How the stage as a whole is dressed at this instant.
pub struct StageMotion {
    pub title: f32,
    pub title_rise: f32,
    pub glow: f32,
    pub flood: f32,
    pub flooding: Option<usize>,
    pub entering: f32,
    pub progress: f32,
}

/// And how one card is.
pub struct CardMotion {
    pub opacity: f32,
    pub offset: f32,
    pub glide: f32,
    pub scale: f32,
    pub lit: bool,
}

impl<C: AnimationClock> Motion<C> {
    #[must_use]
    pub fn entering_card(self) -> Option<usize> {
        match self {
            Self::Entering { card, .. } => Some(card),
            _ => None,
        }
    }

    /// Whether anything is still moving.
    #[must_use]
    pub fn settled(self, now: C) -> bool {
        match self {
            Self::Ready => true,
            Self::Waiting => false,
            Self::Entrance { started } => now.elapsed(started) >= entrance_length(),
            Self::Entering { started, .. } => now.elapsed(started) >= entering_length(),
            Self::SnappingBack { started, .. } => now.elapsed(started) >= SNAP_BACK,
        }
    }

    /// How far into entering a realm this is, for a caller that needs to know
    /// where to snap back from.
    #[must_use]
    pub fn commitment_now(self, now: C) -> f32 {
        self.commitment(now)
    }

    fn commitment(self, now: C) -> f32 {
        match self {
            Self::Entering { started, .. } => {
                let total = (DISSOLVE_AT + DISSOLVE).as_secs_f32();
                (now.elapsed(started).as_secs_f32() / total).clamp(0.0, 1.0)
            }
            Self::SnappingBack { from, started } => {
                let back =
                    (now.elapsed(started).as_secs_f32() / SNAP_BACK.as_secs_f32()).clamp(0.0, 1.0);
                from * (1.0 - back)
            }
            Self::Waiting | Self::Entrance { .. } | Self::Ready => 0.0,
        }
    }

    #[must_use]
    pub fn stage(self, now: C, reduced: bool) -> StageMotion {
        let leaving = self.commitment(now);
        match self {
            Self::Waiting => StageMotion {
                title: 0.0,
                title_rise: travel(TITLE_RISE, reduced),
                glow: 0.0,
                flood: 0.0,
                flooding: None,
                entering: 0.0,
                progress: 0.0,
            },
            Self::Entrance { started } => {
                let elapsed = now.elapsed(started);
                let title = beat(elapsed, TITLE_AT, TITLE_SETTLE);
                StageMotion {
                    title,
                    title_rise: travel(TITLE_RISE * (1.0 - title), reduced),
                    glow: beat(elapsed, Duration::ZERO, TITLE_SETTLE),
                    flood: 0.0,
                    flooding: None,
                    entering: 0.0,
                    progress: 0.0,
                }
            }
            Self::Ready => StageMotion {
                title: 1.0,
                title_rise: 0.0,
                glow: 1.0,
                flood: 0.0,
                flooding: None,
                entering: 0.0,
                progress: 0.0,
            },
            Self::Entering { card, started } => {
                let elapsed = now.elapsed(started);
                StageMotion {
                    title: 1.0 - beat(elapsed, LOSERS_AT, CARD_FADE),
                    title_rise: 0.0,
                    glow: 1.0,
                    flood: beat(elapsed, CENTRE_AT, FLOOD_FADE),
                    flooding: Some(card),
                    entering: beat(elapsed, DISSOLVE_AT, DISSOLVE),
                    progress: 0.08f32
                        .mul_add(1.0 - beat(elapsed, DISSOLVE_AT, ENTER_PROGRESS), 0.0)
                        + beat(elapsed, DISSOLVE_AT, ENTER_PROGRESS),
                }
            }
            Self::SnappingBack { .. } => StageMotion {
                title: 1.0 - leaving,
                title_rise: 0.0,
                glow: 1.0,
                flood: leaving,
                flooding: None,
                entering: 0.0,
                progress: 0.0,
            },
        }
    }

    /// `slot` is how far this card sits from the middle of the row, in whole
    /// cards — the distance it glides when it wins.
    #[must_use]
    pub fn card(self, index: usize, chosen: usize, slot: f32, now: C, reduced: bool) -> CardMotion {
        match self {
            Self::Waiting => CardMotion {
                opacity: 0.0,
                offset: travel(CARD_RISE, reduced),
                glide: 0.0,
                scale: 1.0,
                lit: false,
            },
            Self::Entrance { .. } | Self::Ready => CardMotion {
                opacity: 1.0,
                offset: 0.0,
                glide: 0.0,
                scale: 1.0,
                lit: true,
            },
            Self::Entering { card, started } => {
                let elapsed = now.elapsed(started);
                if card == index {
                    let travelled = beat(elapsed, CENTRE_AT, CARD_TRAVEL);
                    CardMotion {
                        opacity: 1.0 - beat(elapsed, DISSOLVE_AT, DISSOLVE),
                        offset: 0.0,
                        glide: travel(slot * travelled, reduced),
                        scale: (CHOSEN_SCALE - 1.0).mul_add(travelled, 1.0),
                        lit: true,
                    }
                } else {
                    CardMotion {
                        opacity: 1.0 - beat(elapsed, LOSERS_AT, CARD_FADE),
                        offset: travel(LOSER_FALL * beat(elapsed, LOSERS_AT, CARD_TRAVEL), reduced),
                        glide: 0.0,
                        scale: 1.0,
                        lit: true,
                    }
                }
            }
            Self::SnappingBack { .. } => {
                let leaving = self.commitment(now);
                if chosen == index {
                    CardMotion {
                        opacity: 1.0,
                        offset: 0.0,
                        glide: travel(slot * leaving, reduced),
                        scale: (CHOSEN_SCALE - 1.0).mul_add(leaving, 1.0),
                        lit: true,
                    }
                } else {
                    CardMotion {
                        opacity: 1.0 - leaving,
                        offset: travel(LOSER_FALL * leaving, reduced),
                        glide: 0.0,
                        scale: 1.0,
                        lit: true,
                    }
                }
            }
        }
    }

    /// The entrance's per-card beat, the one thing the stage cannot answer for
    /// every card at once.
    #[must_use]
    pub fn entrance_card(self, index: usize, now: C, reduced: bool) -> Option<CardMotion> {
        if matches!(self, Self::Waiting) {
            return Some(self.card(index, index, 0.0, now, reduced));
        }
        let Self::Entrance { started } = self else {
            return None;
        };
        let elapsed = now.elapsed(started);
        let at = FIRST_CARD_AT + CARD_STAGGER * u32::try_from(index).unwrap_or(0);
        let opacity = beat(elapsed, at, CARD_FADE);
        let travelled = beat(elapsed, at, CARD_TRAVEL);
        Some(CardMotion {
            opacity,
            offset: travel(CARD_RISE * (1.0 - travelled), reduced),
            glide: 0.0,
            scale: 1.0,
            lit: elapsed >= LIT_AT,
        })
    }
}

/// How long entering a realm takes, end to end.
#[must_use]
pub fn entering_length() -> Duration {
    DISSOLVE_AT + ENTER_PROGRESS
}

fn entrance_length() -> Duration {
    let last_card = FIRST_CARD_AT + CARD_STAGGER * 2 + CARD_TRAVEL;
    last_card.max(LIT_AT)
}

/// Reduced motion keeps every fade and drops every journey.
fn travel(distance: f32, reduced: bool) -> f32 {
    if reduced { 0.0 } else { distance }
}

#[cfg(test)]
mod tests {
    use super::{CARD_STAGGER, FIRST_CARD_AT, LIT_AT, Motion, settle};

    // milliseconds as the clock, exactly as the browser drives it
    fn ms(value: u64) -> f64 {
        value as f64
    }

    #[test]
    fn the_settle_is_fast_in_and_slow_out_and_never_overshoots() {
        assert!(settle(0.0).abs() < 0.001);
        assert!((settle(1.0) - 1.0).abs() < 0.01);
        assert!(settle(0.25) > 0.6);
        for step in 0_u8..=100 {
            let value = settle(f32::from(step) / 100.0);
            assert!((-0.001..=1.001).contains(&value), "overshot to {value}");
        }
    }

    #[test]
    fn the_cards_come_in_left_to_right_and_the_pressable_one_lights_last() {
        let entrance = Motion::Entrance { started: 0.0 };
        let first = entrance.entrance_card(0, ms(0), false).unwrap();
        assert!(first.opacity.abs() < f32::EPSILON);
        assert!(first.offset > 0.0);

        let mid = ms(FIRST_CARD_AT.as_millis() as u64 + 40);
        assert!(entrance.entrance_card(0, mid, false).unwrap().opacity > 0.0);
        assert!(entrance.entrance_card(2, mid, false).unwrap().opacity.abs() < f32::EPSILON);

        let before = ms(LIT_AT.as_millis() as u64 - 1);
        assert!(!entrance.entrance_card(0, before, false).unwrap().lit);
        assert!(
            entrance
                .entrance_card(0, ms(LIT_AT.as_millis() as u64), false)
                .unwrap()
                .lit
        );
        let all_home = FIRST_CARD_AT + CARD_STAGGER * 2;
        assert!(LIT_AT > all_home);
    }

    #[test]
    fn entering_a_realm_takes_the_others_down_and_the_chosen_one_over() {
        let entering = Motion::Entering {
            card: 0,
            started: 0.0,
        };
        let landed = ms(1_500);
        assert!(entering.card(0, 0, 322.0, ms(100), false).glide.abs() < f32::EPSILON);
        assert!(entering.card(1, 0, 0.0, ms(100), false).opacity < 1.0);

        let winner = entering.card(0, 0, 322.0, landed, false);
        assert!(winner.glide > 300.0);
        assert!(winner.scale > 1.2);
        assert!(winner.opacity < 0.01);

        let stage = entering.stage(landed, false);
        assert!((stage.flood - 1.0).abs() < 0.01);
        assert_eq!(stage.flooding, Some(0));
        assert!(stage.title < 0.01);
    }

    #[test]
    fn backing_out_snaps_from_wherever_it_had_got_to() {
        let snapping = Motion::SnappingBack {
            from: 0.6,
            started: 0.0,
        };
        assert!((snapping.stage(ms(0), false).flood - 0.6).abs() < 0.01);
        assert!(snapping.settled(ms(201)));
        assert!(!snapping.settled(ms(100)));
    }
}
