//! What the picker's top bar is doing: the REFRESH chip's poll, the SIGN OUT
//! chip's two-click arm, and the plaque that greys out while a sign-out is
//! in flight.
//!
//! From the `Titlebar` design: one refresh for the whole wall, counted; no
//! modal on sign out — the chip arms, a second click within three seconds
//! confirms, and the identity dims while the session is being severed.

use super::*;

/// How long UP TO DATE stays up before the chip reverts to REFRESH.
pub(in crate::app::client) const REFRESH_DONE_HOLD: Duration = Duration::from_millis(800);
/// How long an armed SIGN OUT waits for its confirming click.
pub(in crate::app::client) const SIGN_OUT_ARM: Duration = Duration::from_secs(3);
/// The least time the bar says SEVERING, so a hand-off that is over before
/// the next frame still reads as having happened.
const SEVERING_HOLD: Duration = Duration::from_millis(600);

/// One refresh of every realm's status, from the click to the flash that
/// says it landed.
#[derive(Clone, Copy, Debug)]
pub(in crate::app::client) struct RefreshRun {
    pub(in crate::app::client) started: Instant,
    /// How many realms were sent back round. The count on the chip is this
    /// minus whatever is still in flight.
    pub(in crate::app::client) total: usize,
    /// When the last of them settled; the chip flashes UP TO DATE from here.
    pub(in crate::app::client) done_at: Option<Instant>,
}

/// The identity the bar keeps showing, greyed, while the account it named is
/// being signed out. Battle.net forgets the name at once; the bar must not.
#[derive(Clone, Debug)]
pub(in crate::app::client) struct SeveredPlaque {
    pub(in crate::app::client) since: Instant,
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) region: Option<u32>,
}

/// What the REFRESH chip shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum RefreshChip {
    Ready,
    /// The arrow sweeps and the label counts the realms that have answered.
    Polling {
        done: usize,
        total: usize,
    },
    /// The flash after the last answer.
    UpToDate,
    /// Mid sign-out there is nothing left to refresh.
    Disabled,
}

/// What the SIGN OUT chip shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum SignOutChip {
    Ready,
    /// One click down; the next one within the window signs out.
    Armed,
    /// The sign-out is in flight.
    Severing,
    /// Nobody is signed in to sign out.
    Inert,
}

impl GamesComponent {
    /// A refresh has been asked for: `total` realms are going back round.
    pub(in crate::app::client) fn begin_refresh(&mut self, total: usize, now: Instant) {
        self.refresh = Some(RefreshRun {
            started: now,
            total,
            done_at: None,
        });
    }

    /// The REFRESH chip, given how many realms are still reconnecting.
    pub(in crate::app::client) fn refresh_chip(&self, in_flight: usize) -> RefreshChip {
        if self.severing.is_some() {
            return RefreshChip::Disabled;
        }
        match self.refresh {
            Some(RefreshRun {
                done_at: Some(_), ..
            }) => RefreshChip::UpToDate,
            Some(RefreshRun { total, .. }) => RefreshChip::Polling {
                done: total.saturating_sub(in_flight),
                total,
            },
            None => RefreshChip::Ready,
        }
    }

    /// The SIGN OUT chip, given whether anyone is signed in.
    pub(in crate::app::client) fn sign_out_chip(
        &self,
        signed_in: bool,
        now: Instant,
    ) -> SignOutChip {
        if self.severing.is_some() {
            return SignOutChip::Severing;
        }
        if !signed_in {
            return SignOutChip::Inert;
        }
        if self
            .sign_out_armed
            .is_some_and(|armed| now.saturating_duration_since(armed) < SIGN_OUT_ARM)
        {
            return SignOutChip::Armed;
        }
        SignOutChip::Ready
    }

    /// A press on SIGN OUT. The first arms the chip; the second, inside the
    /// window, confirms — and returns true so the caller signs out.
    pub(in crate::app::client) fn press_sign_out(&mut self, now: Instant) -> bool {
        let confirmed = self
            .sign_out_armed
            .take()
            .is_some_and(|armed| now.saturating_duration_since(armed) < SIGN_OUT_ARM);
        if !confirmed {
            self.sign_out_armed = Some(now);
        }
        confirmed
    }

    /// The sign-out is going out: keep the plaque, greyed, until the account
    /// is gone or a new one has arrived.
    pub(in crate::app::client) fn begin_severing(
        &mut self,
        name: String,
        region: Option<u32>,
        now: Instant,
    ) {
        self.sign_out_armed = None;
        self.severing = Some(SeveredPlaque {
            since: now,
            name,
            region,
        });
    }

    /// Moves the bar's clocks on: lands a refresh once nothing is in flight,
    /// lets UP TO DATE and an un-confirmed arm lapse, and ends SEVERING once
    /// a new identity is in or the reconnect behind the sign-out has stopped.
    /// Returns whether anything changed.
    pub(in crate::app::client) fn settle_titlebar(
        &mut self,
        now: Instant,
        in_flight: usize,
        signed_in: bool,
        reconnecting: bool,
    ) -> bool {
        let mut changed = false;
        if let Some(run) = &mut self.refresh {
            match run.done_at {
                None if in_flight == 0 => {
                    run.done_at = Some(now);
                    changed = true;
                }
                Some(done_at) if now.saturating_duration_since(done_at) >= REFRESH_DONE_HOLD => {
                    self.refresh = None;
                    changed = true;
                }
                _ => {}
            }
        }
        if self
            .sign_out_armed
            .is_some_and(|armed| now.saturating_duration_since(armed) >= SIGN_OUT_ARM)
        {
            self.sign_out_armed = None;
            changed = true;
        }
        if let Some(plaque) = &self.severing
            && (signed_in
                || (!reconnecting && now.saturating_duration_since(plaque.since) >= SEVERING_HOLD))
        {
            self.severing = None;
            changed = true;
        }
        changed
    }

    /// Whether the bar has a clock running that the next frame must read.
    pub(in crate::app::client) fn titlebar_live(&self) -> bool {
        self.refresh.is_some() || self.sign_out_armed.is_some() || self.severing.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn games() -> GamesComponent {
        GamesComponent::new(Some(GamesScreen::Picker), None, true)
    }

    #[test]
    fn the_second_press_inside_the_window_confirms_and_the_chip_says_so() {
        let mut games = games();
        let t0 = Instant::now();
        assert_eq!(games.sign_out_chip(true, t0), SignOutChip::Ready);
        assert!(!games.press_sign_out(t0));
        assert_eq!(
            games.sign_out_chip(true, t0 + Duration::from_secs(1)),
            SignOutChip::Armed
        );
        assert!(games.press_sign_out(t0 + Duration::from_secs(2)));
        assert_eq!(
            games.sign_out_chip(true, t0 + Duration::from_secs(2)),
            SignOutChip::Ready
        );
    }

    #[test]
    fn an_arm_left_alone_lapses_and_a_late_press_only_re_arms() {
        let mut games = games();
        let t0 = Instant::now();
        assert!(!games.press_sign_out(t0));
        let late = t0 + SIGN_OUT_ARM;
        assert_eq!(games.sign_out_chip(true, late), SignOutChip::Ready);
        assert!(games.settle_titlebar(late, 0, true, false));
        assert!(!games.press_sign_out(late));
        assert_eq!(games.sign_out_chip(true, late), SignOutChip::Armed);
    }

    #[test]
    fn the_refresh_counts_what_has_answered_and_flashes_once_everything_has() {
        let mut games = games();
        let t0 = Instant::now();
        games.begin_refresh(3, t0);
        assert_eq!(
            games.refresh_chip(3),
            RefreshChip::Polling { done: 0, total: 3 }
        );
        assert_eq!(
            games.refresh_chip(1),
            RefreshChip::Polling { done: 2, total: 3 }
        );
        assert!(games.settle_titlebar(t0 + Duration::from_secs(2), 0, true, false));
        assert_eq!(games.refresh_chip(0), RefreshChip::UpToDate);
        let held = t0 + Duration::from_secs(2) + REFRESH_DONE_HOLD;
        assert!(games.settle_titlebar(held, 0, true, false));
        assert_eq!(games.refresh_chip(0), RefreshChip::Ready);
    }

    #[test]
    fn severing_keeps_the_name_disables_refresh_and_ends_with_the_account() {
        let mut games = games();
        let t0 = Instant::now();
        games.begin_severing("ncarrillo".to_owned(), Some(1), t0);
        assert_eq!(games.sign_out_chip(false, t0), SignOutChip::Severing);
        assert_eq!(games.refresh_chip(0), RefreshChip::Disabled);
        assert_eq!(
            games.severing.as_ref().map(|plaque| plaque.name.as_str()),
            Some("ncarrillo")
        );
        // still reconnecting: the plaque stays greyed
        assert!(!games.settle_titlebar(t0 + Duration::from_secs(5), 0, false, true));
        // the reconnect stopped with nobody signed in: the bar lets go
        assert!(games.settle_titlebar(t0 + Duration::from_secs(5), 0, false, false));
        assert_eq!(games.sign_out_chip(false, t0), SignOutChip::Inert);
    }

    #[test]
    fn a_new_identity_ends_severing_at_once() {
        let mut games = games();
        let t0 = Instant::now();
        games.begin_severing("ncarrillo".to_owned(), None, t0);
        assert!(games.settle_titlebar(t0, 0, true, true));
        assert!(games.severing.is_none());
    }
}
