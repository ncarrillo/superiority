//! The picker's selection-and-choreography state, shared by the desktop shell
//! and the browser viewer so one state machine drives both. The host owns three
//! things this deliberately does not: which cards are *actionable* (a licence
//! and a recovered protocol on the desktop; a live feed in the viewer), each
//! card's resting *slot offset* (a layout concern), and what *entering* a realm
//! means (the desktop hands the client the front door; the viewer mounts a realm
//! view). Everything else — which realm is chosen, what it was, and where the
//! entrance/room/enter-game choreography has got to — lives here.

use crate::foundation::animation::AnimationClock;

use super::look::{ROOM, eased};
use super::model::GAMES;
use super::motion::{CardMotion, Motion};

/// What one turn of the choreography did, for the host to react to.
pub struct Advance {
    /// The realm a just-settled enter-game glide committed to — the host hands
    /// over to it. `None` on an ordinary frame.
    pub entered: Option<usize>,
    /// Whether the choreography still wants another frame.
    pub animating: bool,
}

/// The selection and choreography a picker carries. `C` is the host's animation
/// clock — `Instant` on the desktop, `f64` milliseconds in the browser.
pub struct PickerState<C> {
    pub motion: Motion<C>,
    /// Which realm is chosen. Sticky, and starts on the first game: a picker
    /// with nothing selected is one you have to start over with.
    pub selected: usize,
    /// What it was, so the room crossfades between them instead of cutting.
    pub previously_selected: usize,
    /// When the selection last changed, for the room crossfade.
    pub chosen: C,
    /// Which card the pointer is over. Transient: it lifts the card under the
    /// pointer and nobody else, and is nobody once the pointer leaves.
    pub hovered: Option<usize>,
}

impl<C: AnimationClock> PickerState<C> {
    #[must_use]
    pub fn new(now: C) -> Self {
        Self {
            motion: Motion::Waiting,
            selected: 0,
            previously_selected: 0,
            chosen: now,
            hovered: None,
        }
    }

    fn playable(actionable: &[bool], card: usize) -> bool {
        actionable.get(card).copied().unwrap_or(false)
    }

    /// Reaches for a card: hovering one also selects it, so the room follows the
    /// pointer; leaving leaves the selection where it was, so the room does not
    /// go dark behind you. A card you cannot act on takes no attention with it.
    pub fn reach(&mut self, card: Option<usize>, now: C, actionable: &[bool]) -> bool {
        let card = card.filter(|card| Self::playable(actionable, *card));
        let moved = self.hovered != card;
        self.hovered = card;
        if let Some(card) = card {
            return self.choose(card, now, actionable) || moved;
        }
        moved
    }

    /// Chooses a realm, keeping the one it replaced so the room can cross
    /// between them.
    pub fn choose(&mut self, card: usize, now: C, actionable: &[bool]) -> bool {
        if self.selected == card || card >= GAMES.len() || !Self::playable(actionable, card) {
            return false;
        }
        self.previously_selected = self.selected;
        self.selected = card;
        self.chosen = now;
        true
    }

    /// Steps the selection to the next actionable realm in a direction, skipping
    /// any that answer to nothing.
    pub fn move_choice(&mut self, delta: isize, now: C, actionable: &[bool]) {
        let last = isize::try_from(GAMES.len().saturating_sub(1)).unwrap_or(0);
        let step = delta.signum();
        let mut next = isize::try_from(self.selected).unwrap_or(0);
        loop {
            let candidate = next + step;
            if candidate < 0 || candidate > last {
                return;
            }
            next = candidate;
            let index = usize::try_from(next).unwrap_or(0);
            if Self::playable(actionable, index) {
                self.choose(index, now, actionable);
                return;
            }
        }
    }

    /// The rooms to paint behind the cards, bottom first — a crossfade between
    /// the realm left and the one arriving.
    #[must_use]
    pub fn rooms(&self, now: C, actionable: &[bool]) -> Vec<(usize, f32)> {
        if !Self::playable(actionable, self.selected) {
            return Vec::new();
        }
        let blend = eased(self.chosen, ROOM, now);
        let mut rooms = Vec::with_capacity(2);
        if self.previously_selected != self.selected && blend < 1.0 {
            rooms.push((self.previously_selected, 1.0 - blend));
        }
        rooms.push((self.selected, blend));
        rooms
    }

    /// How this card is placed right now: the entrance puts it on screen, and
    /// after that only entering a realm moves it. The host supplies the card's
    /// resting `slot` offset, which is its own layout concern.
    #[must_use]
    pub fn card_motion(&self, index: usize, slot: f32, now: C, reduced: bool) -> CardMotion {
        self.motion
            .entrance_card(index, now, reduced)
            .unwrap_or_else(|| self.motion.card(index, self.selected, slot, now, reduced))
    }

    /// Winds the choreography on: the entrance settles into ready, a snap-back
    /// settles back into it, and an enter-game glide hands over once it has
    /// dissolved. The rehearsal auto-trigger and the meaning of the handoff stay
    /// with the host.
    pub fn advance(&mut self, now: C) -> Advance {
        let mut entered = None;
        if let Motion::Entering { card, .. } = self.motion
            && self.motion.settled(now)
        {
            entered = Some(card);
            self.motion = Motion::Ready;
        }
        match self.motion {
            // the entrance is timed from the frame it is first seen on
            Motion::Waiting => self.motion = Motion::Entrance { started: now },
            Motion::Entrance { .. } | Motion::SnappingBack { .. } if self.motion.settled(now) => {
                self.motion = Motion::Ready;
            }
            _ => {}
        }
        Advance {
            entered,
            animating: !self.motion.settled(now),
        }
    }

    /// Commits to a realm. Nothing else answers while the glide runs. The host
    /// gates on actionability before calling.
    pub fn begin_entering(&mut self, card: usize, now: C) {
        if matches!(self.motion, Motion::Entering { .. }) {
            return;
        }
        self.motion = Motion::Entering { card, started: now };
    }

    /// Backs out of an entrance still in flight, from wherever it had got to.
    pub fn snap_back(&mut self, now: C) -> bool {
        let Motion::Entering { .. } = self.motion else {
            return false;
        };
        self.motion = Motion::SnappingBack {
            from: self.motion.commitment_now(now),
            started: now,
        };
        true
    }

    /// Comes back out to the list from inside a realm, replaying the entrance
    /// rather than appearing all at once.
    pub fn return_to_list(&mut self, now: C) {
        self.motion = Motion::Entrance { started: now };
    }

    #[must_use]
    pub fn ready(&self) -> bool {
        matches!(self.motion, Motion::Ready)
    }
}
