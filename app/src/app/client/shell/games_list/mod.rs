use super::*;

mod look;
mod model;
mod motion;
mod titlebar;
mod view;

pub(in crate::app::client) use look::CardLook;
pub(in crate::app::client) use model::{CARD_STATES, CardState, GAMES, GamePalette};
pub(in crate::app::client) use motion::{CardMotion, Motion, StageMotion};
use superiority_ui::products::games::PickerState;
pub(in crate::app::client) use titlebar::{RefreshChip, RefreshRun, SeveredPlaque, SignOutChip};
use view::{
    CardSize, card_index, card_product, hero_section, masthead, palette_section, picker_section,
    picker_size, picker_titlebar, slot_offset, together_section,
};

/// The states walk themselves so the whole set can be seen without touching
/// anything; a click steps them by hand.
pub(in crate::app::client) const STATE_DWELL: Duration = Duration::from_millis(2600);
/// How long a connect takes before it lands. Long enough to watch the beam
/// cross the art, which is the point of watching it at all.
const CONNECT_TIME: Duration = Duration::from_millis(2200);
const HERO_SECTION_HEIGHT: f32 = 540.0;
const SHEET: u32 = 0x0004_060a;
const RULE: u32 = 0x001a_2028;
const PAPER: u32 = 0x00e6_edf7;
const UNLIT: u32 = 0x001a_2430;

/// What the session behind a game is doing right now. The card is a view of
/// this rather than of a fixture, so what it says is what is true.
#[derive(Clone, Default)]
pub(in crate::app::client) struct LiveGame {
    pub(in crate::app::client) state: CardState,
    /// Who you are in there — the clan tag and name the roster knows, falling
    /// back to the account's battle tag before a channel has answered.
    pub(in crate::app::client) clan_tag: Option<String>,
    pub(in crate::app::client) handle: Option<String>,
    /// Where you are, and how many are there with you.
    pub(in crate::app::client) channel: Option<String>,
    /// The Battle.net region *this* product's session came in through. Per
    /// product, like everything else here: the shared copy showed
    /// `StarCraft II`'s region and battletag on every card.
    pub(in crate::app::client) region: Option<u32>,
    pub(in crate::app::client) online: Option<usize>,
    /// What the connection is doing, while it is doing it.
    pub(in crate::app::client) progress: Option<String>,
    /// "Step 2 of 4" while the handshake runs — the dialog's own counter, so
    /// the card says how far along it is rather than only what it is doing.
    pub(in crate::app::client) step: Option<String>,
}

/// A state change in flight: what the cards were, what they are becoming, and
/// how long ago that started. Every card on screen reads the same one, which is
/// what keeps three palettes in step.
#[derive(Clone, Copy)]
struct Walk {
    from: CardState,
    to: CardState,
    since: Instant,
    now: Instant,
}

impl Walk {
    fn look(self, palette: &GamePalette) -> CardLook {
        CardLook::resolve(palette, self.from, self.to, self.since, self.now)
    }
}

/// Which of the two screens the flags asked for: the sheet that walks the card
/// through its states, or the picker those cards make.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(in crate::app::client) enum GamesScreen {
    States,
    Picker,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches on the picker, not a state enum in disguise"
)]
pub(in crate::app::client) struct GamesComponent {
    /// Which screen to show, if either. Without a flag the whole surface stays
    /// out of the way.
    pub(in crate::app::client) showing: Option<GamesScreen>,
    pub(in crate::app::client) state: CardState,
    /// What the cards are crossing over from. A state change is a transition
    /// rather than a cut, so the card it was is still on screen for a moment.
    pub(in crate::app::client) previous: CardState,
    pub(in crate::app::client) advanced: Instant,
    /// The picker's selection and choreography: which realm is chosen, what it
    /// was (so the room crossfades), which card the pointer lifts, and where the
    /// entrance/enter-game animation has got to. Shared with the browser viewer.
    pub(in crate::app::client) picker: PickerState<Instant>,
    /// Which games the authoritative account owns, by program code. `None`
    /// until its license catalogue arrives. Live mode shows no cards while
    /// this is unknown; a preview with no catalogue still demonstrates all.
    pub(in crate::app::client) owned: Option<Vec<String>>,
    /// Whether this is a real client rather than the design screens.
    ///
    /// The palettes carry fixture identities, regions, pings, and channels so
    /// the design screens have something to draw. On a live card those read as
    /// somebody's real account — a card with no session of its own showed
    /// `ncarrillo` in `The Void` at 54 ms — so live mode shows nothing where it
    /// knows nothing.
    pub(in crate::app::client) live_mode: bool,
    /// Each realm's own state in the picker. The walk demonstrates the six on
    /// the hero card; these are what the three games are actually in.
    pub(in crate::app::client) card: [CardState; GAMES.len()],
    pub(in crate::app::client) card_from: [CardState; GAMES.len()],
    pub(in crate::app::client) card_since: [Instant; GAMES.len()],

    /// Which game the hero card is dressed as. The walk is about the states, so
    /// the game it demonstrates them on is a choice made once.
    pub(in crate::app::client) hero: usize,
    pub(in crate::app::client) scroll: ScrollHandle,
    /// Fades without journeys, for a reader who has asked for that.
    pub(in crate::app::client) reduced_motion: bool,
    /// Plays the enter-game sequence on its own once the entrance has settled,
    /// so the choreography can be watched without a keyboard.
    pub(in crate::app::client) rehearsing: bool,
    /// Set once the picker has dissolved into a realm. From here the client
    /// takes over and the picker is done for this session.
    pub(in crate::app::client) entered: bool,
    /// The live session behind each card, keyed by the product it belongs to.
    ///
    /// Per-product because more than one game can be signed in at once. While
    /// `StarCraft II` was the only playable one a single slot was accidentally
    /// right; with Remastered playable too, one slot showed `StarCraft II`'s
    /// channel, population, and errors on Remastered's card.
    pub(in crate::app::client) live: BTreeMap<Product, LiveGame>,
    /// The top bar's REFRESH, from the click until UP TO DATE has faded.
    pub(in crate::app::client) refresh: Option<RefreshRun>,
    /// When SIGN OUT was armed, if it is waiting for its confirming click.
    pub(in crate::app::client) sign_out_armed: Option<Instant>,
    /// The identity the bar keeps greyed while the account is signed out.
    pub(in crate::app::client) severing: Option<SeveredPlaque>,
}

impl GamesComponent {
    pub(in crate::app::client) fn new(
        showing: Option<GamesScreen>,
        owned: Option<Vec<String>>,
        reduced: bool,
    ) -> Self {
        let mut games = Self {
            showing,
            state: CardState::Idle,
            previous: CardState::Idle,
            advanced: Instant::now(),
            picker: PickerState::new(Instant::now()),
            owned,
            card: [CardState::Idle; GAMES.len()],
            card_from: [CardState::Idle; GAMES.len()],
            card_since: [Instant::now(); GAMES.len()],
            hero: 0,
            scroll: ScrollHandle::new(),
            reduced_motion: reduced,
            rehearsing: false,
            entered: false,
            live: BTreeMap::new(),
            live_mode: false,
            refresh: None,
            sign_out_armed: None,
            severing: None,
        };
        // each card opens in the state its game is actually in, and the choice
        // opens on a game the account has rather than on whatever is first
        for index in 0..GAMES.len() {
            games.card[index] = games.resting(index);
            games.card_from[index] = games.card[index];
        }
        games.picker.selected = (0..GAMES.len())
            .find(|index| games.owns(*index))
            .unwrap_or(0);
        games.picker.previously_selected = games.picker.selected;
        games
    }

    /// Walks to the next state once the current one has been up long enough.
    /// Returns whether anything moved, so the caller only repaints on a change.
    pub(in crate::app::client) fn advance_if_due(&mut self) -> bool {
        if Instant::now().saturating_duration_since(self.advanced) < STATE_DWELL {
            return false;
        }
        self.step();
        true
    }

    pub(in crate::app::client) fn step(&mut self) {
        let next = CARD_STATES
            .iter()
            .position(|state| *state == self.state)
            .map_or(0, |index| (index + 1) % CARD_STATES.len());
        self.previous = self.state;
        self.state = CARD_STATES[next];
        self.advanced = Instant::now();
    }

    /// Notes which card the pointer is over. Reaching for a card also chooses
    /// it, so the room follows the pointer; taking the pointer away leaves the
    /// choice where it was, so the room does not go dark behind you.
    pub(in crate::app::client) fn reach(&mut self, card: Option<usize>) -> bool {
        let mask = self.actionable_mask();
        self.picker.reach(card, Instant::now(), &mask)
    }

    /// Chooses a realm, keeping the one it replaced so the room can cross
    /// between them.
    pub(in crate::app::client) fn choose(&mut self, card: usize) -> bool {
        let mask = self.actionable_mask();
        self.picker.choose(card, Instant::now(), &mask)
    }

    /// The three cards' actionability, the mask the shared picker filters on.
    fn actionable_mask(&self) -> [bool; GAMES.len()] {
        std::array::from_fn(|index| self.actionable(index))
    }

    /// Walks the choreography on. The enter-game handoff sets [`Self::entered`]
    /// for the host to read, and a rehearsal walks itself in once the picker is
    /// still — the one way to watch the sequence without a keyboard.
    pub(in crate::app::client) fn advance_motion(&mut self, now: Instant) -> bool {
        let advanced = self.picker.advance(now);
        if advanced.entered.is_some() {
            self.entered = true;
            return true;
        }
        if self.rehearsing && self.picker.ready() {
            let chosen = self.picker.selected;
            self.begin_entering(chosen, now);
            return true;
        }
        advanced.animating
    }

    /// Commits to a realm. Nothing else on the picker answers while this runs.
    pub(in crate::app::client) fn begin_entering(&mut self, card: usize, now: Instant) {
        if !self.actionable(card) {
            return;
        }
        self.picker.begin_entering(card, now);
    }

    /// Comes back out to the list from inside a realm.
    ///
    /// Distinct from [`Self::snap_back`], which only reverses an entrance still
    /// in flight. Once a realm has been entered the picker is out of the way,
    /// and returning replays the entrance rather than appearing all at once.
    pub(in crate::app::client) fn return_to_list(&mut self, now: Instant) {
        self.entered = false;
        self.picker.return_to_list(now);
    }

    /// Backs out of one, from wherever it had got to.
    pub(in crate::app::client) fn snap_back(&mut self, now: Instant) -> bool {
        self.picker.snap_back(now)
    }

    /// How this card is placed right now: the entrance puts it on screen, and
    /// after that only entering a realm moves it. The resting slot offset is
    /// this shell's own layout concern.
    fn card_motion(&self, index: usize, size: CardSize, now: Instant) -> CardMotion {
        self.picker.card_motion(
            index,
            slot_offset(index, size, self),
            now,
            self.reduced_motion,
        )
    }

    /// The region this session came in through, in the words Battle.net uses
    /// for them. The ids are the service's own; anything we have no name for is
    /// left unsaid rather than guessed at.
    pub(in crate::app::client) fn region_name(region: Option<u32>) -> Option<&'static str> {
        match region? {
            1 => Some("Americas"),
            2 => Some("Europe"),
            3 => Some("Korea"),
            5 => Some("China"),
            _ => None,
        }
    }

    /// the live session behind a card: only playable games have one.
    pub(in crate::app::client) fn live_for(&self, index: usize) -> Option<&LiveGame> {
        let game = GAMES.get(index).filter(|game| game.playable)?;
        self.live.get(&Product::from_code(game.program)?)
    }

    /// Whether the picker has handed over to the client.
    pub(in crate::app::client) fn has_entered(&self) -> bool {
        self.entered
    }

    /// Which card is on its way into a realm, if any.
    pub(in crate::app::client) fn entering(&self) -> Option<usize> {
        self.picker.motion.entering_card()
    }

    /// Whether the picker is answering the pointer and the keyboard at all.
    pub(in crate::app::client) fn answering(&self) -> bool {
        matches!(self.picker.motion, Motion::Ready)
    }

    /// Whether the authoritative Battle.net account has this game.
    pub(in crate::app::client) fn owns(&self, index: usize) -> bool {
        // a game with no protocol behind it is never offered, however much of
        // it the account owns
        GAMES.get(index).is_some_and(|game| game.playable) && self.licensed(index)
    }

    /// Whether the account has this game, ignoring whether this client can do
    /// anything with it. The two are separate: one is about the account, the
    /// other about how much of this app is written yet.
    pub(in crate::app::client) fn licensed(&self, index: usize) -> bool {
        let Some(palette) = GAMES.get(index) else {
            return false;
        };
        let Some(owned) = self.owned.as_ref() else {
            return !self.live_mode;
        };
        owned
            .iter()
            .any(|program| program.eq_ignore_ascii_case(palette.program))
    }

    /// Whether this card can be used. Hidden, unprovisioned games never answer.
    pub(in crate::app::client) fn actionable(&self, index: usize) -> bool {
        self.owns(index)
    }

    /// Replaces the one authoritative license catalogue and moves selection to
    /// the first visible product if the old selection disappeared.
    pub(in crate::app::client) fn set_owned(&mut self, owned: Vec<String>) {
        self.owned = Some(owned);
        for index in 0..GAMES.len() {
            let resting = self.resting(index);
            if !self.owns(index) || self.card[index] == CardState::NoAccount {
                self.card[index] = resting;
                self.card_from[index] = resting;
                self.card_since[index] = Instant::now();
            }
        }
        if !self.owns(self.picker.selected)
            && let Some(first) = (0..GAMES.len()).find(|index| self.owns(*index))
        {
            self.picker.selected = first;
            self.picker.previously_selected = first;
            self.picker.chosen = Instant::now();
        }
        self.picker.hovered = self.picker.hovered.filter(|index| self.owns(*index));
    }

    #[must_use]
    pub(in crate::app::client) fn visible(&self, index: usize) -> bool {
        GAMES.get(index).is_some_and(|game| game.shown) && self.owns(index)
    }

    #[must_use]
    pub(in crate::app::client) fn visible_count(&self) -> usize {
        (0..GAMES.len())
            .filter(|index| self.visible(*index))
            .count()
    }

    #[must_use]
    pub(in crate::app::client) fn visible_position(&self, index: usize) -> usize {
        (0..index)
            .filter(|candidate| self.visible(*candidate))
            .count()
    }

    /// The state a card rests in: the one the game is actually in when the
    /// account has it, and the design's empty state when it does not.
    pub(in crate::app::client) fn resting(&self, index: usize) -> CardState {
        if self.owns(index) {
            GAMES
                .get(index)
                .map_or(CardState::Idle, |game| game.resting)
        } else {
            CardState::NoAccount
        }
    }

    /// Puts one realm into a state, keeping what it was so the card can cross
    /// over rather than cut.
    pub(in crate::app::client) fn set_card(&mut self, index: usize, state: CardState) {
        let Some(current) = self.card.get_mut(index) else {
            return;
        };
        if *current == state {
            return;
        }
        self.card_from[index] = *current;
        *current = state;
        self.card_since[index] = Instant::now();
    }

    /// Walks a card one step through the state machine, the way pressing it
    /// used to.
    ///
    /// Test-only since every playable product connects at startup: there is no
    /// longer a card with no session behind it for a press to start. The state
    /// machine it walks is still the real one, which is what these tests are
    /// checking.
    #[cfg(test)]
    pub(in crate::app::client) fn enter(&mut self, index: usize) {
        if !self.actionable(index) {
            return;
        }
        let Some(state) = self.card.get(index).copied() else {
            return;
        };
        let next = match state {
            CardState::Connected => self.resting(index),
            CardState::Connecting => CardState::Connected,
            _ => CardState::Connecting,
        };
        self.set_card(index, next);
    }

    /// Lands any connection that has been running long enough. Returns whether
    /// anything moved.
    pub(in crate::app::client) fn land_connections(&mut self, now: Instant) -> bool {
        let mut landed = false;
        for index in 0..self.card.len() {
            if self.card[index] == CardState::Connecting
                && now.saturating_duration_since(self.card_since[index]) >= CONNECT_TIME
            {
                self.set_card(index, CardState::Connected);
                landed = true;
            }
        }
        landed
    }

    /// Moves the choice from one card to the next, stopping at the ends rather
    /// than wrapping: a row of three is short enough to see whole, and wrapping
    /// past the edge of something you can see is disorienting.
    /// Steps over the games the account does not have: they are on screen to
    /// be seen, not to be landed on.
    pub(in crate::app::client) fn move_choice(&mut self, delta: isize) {
        let mask = self.actionable_mask();
        self.picker.move_choice(delta, Instant::now(), &mask);
    }

    /// The three cards' visibility, the mask the room crossfade filters on — a
    /// game with nothing behind it draws no room, even where it would be offered.
    fn visible_mask(&self) -> [bool; GAMES.len()] {
        std::array::from_fn(|index| self.visible(index))
    }

    /// The rooms to paint, bottom first. A crossfade is one image coming up
    /// *over* another, not both going half-transparent at once: fading them
    /// against each other shows whatever is behind them both, which reads as a
    /// dip to flat colour on the way past.
    ///
    /// There is always a room, because there is always a selection.
    pub(in crate::app::client) fn rooms(&self, now: Instant) -> Vec<(usize, f32)> {
        let mask = self.visible_mask();
        self.picker.rooms(now, &mask)
    }
}

impl SuperiorityView {
    /// Taking a card, from the pointer or the keyboard — one path, so the two
    /// cannot drift into meaning different things.
    ///
    /// Takes you into a realm.
    ///
    /// Every playable product is already connecting — taking a card chooses
    /// which session you are looking at, and going back to the games list lets
    /// you choose another. So this switches focus and enters; it does not
    /// connect anything.
    ///
    /// The exception is a card that has failed: its verb says RETRY, and that
    /// is the one press that starts something.
    pub(in crate::app::client) fn take_game_card(
        &mut self,
        index: usize,
        now: Instant,
        cx: &mut Context<Self>,
    ) {
        if !self.games.answering() || !self.games.actionable(index) {
            return;
        }
        self.games.choose(index);
        if let Some(product) = card_product(index) {
            self.focus_product(product);
        }
        match self.games.card[index] {
            // a connection under way is left alone: the card is already showing
            // what it is doing, and pressing at it would only lie about it
            CardState::Connecting => {}
            // a realm that failed is the one thing worth pressing at
            CardState::Unreachable | CardState::NoAccount => self.reconnect(cx),
            // and everything else takes you in — the session is there whether it
            // has finished signing in or not
            _ => self.games.begin_entering(index, now),
        }
    }

    /// Points every card at its own session.
    ///
    /// Each card is a view of one product's session, so every session has to be
    /// visited — not just the focused one. While this only synced the focused
    /// product, a background game's card had no report at all: it showed
    /// nothing while connecting and nothing when it failed, and its error only
    /// appeared once you clicked the card and made it the focused one.
    ///
    /// Visiting is a focus swap, the same as draining events, because the
    /// report is read from `self.session` and written to `self.focused`'s card.
    pub(in crate::app::client) fn sync_live_games(&mut self) {
        let front = self.focused;
        let others: Vec<Product> = self.suspended.keys().copied().collect();
        for product in others {
            if self.focus_product(product) {
                self.sync_live_game();
            }
        }
        self.focus_product(front);
        self.sync_live_game();
    }

    /// Points the focused product's card at its session: how far the connection
    /// has got, who it says you are, and where it has put you. Everything here
    /// is read from state the client already keeps — the card is a view of the
    /// session, not a second copy of it.
    fn sync_live_game(&mut self) {
        if !self.runtime.live_mode {
            return;
        }
        let state = match self.session.connection.stage {
            _ if self.session.connection.error.is_some() => CardState::Unreachable,
            ConnectionStage::Disconnected => CardState::Idle,
            ConnectionStage::Connected => CardState::Connected,
            _ => CardState::Connecting,
        };
        // the legacy dialog is hidden behind the picker now, so the card has to
        // say everything the dialog said — from the one place that has the
        // words, in the short form that fits a card beside its step counter
        let report = self.session.connection.progress(self.focused);
        let progress = (!matches!(state, CardState::Connected))
            .then(|| report.brief.trim_end_matches('.').to_owned());
        let step = (!report.step.is_empty()).then(|| report.step.clone());
        let account = self.local_account();
        let scr_channel = self
            .session
            .scr()
            .and_then(|scr| scr.channel.as_ref())
            .map(|channel| (channel.title.clone(), channel.members.len()));
        let wc3_channel = self
            .session
            .wc3()
            .and_then(Wc3SessionUi::active)
            .map(|channel| (channel.name.clone(), channel.members.len()));
        let live = LiveGame {
            state,
            clan_tag: account.and_then(|(user, _)| user.clan_tag.clone()),
            handle: account
                .map(|(user, _)| strip_character_code(&user.name).to_owned())
                .or_else(|| self.session.account_battle_tag.clone()),
            region: self.session.account_region,
            // the channel comes from the active tab when the session has no
            // StarCraft II-shaped local member to look it up through — which is
            // every session that is not StarCraft II's
            channel: account
                .map(|(_, channel)| channel.to_owned())
                .or_else(|| scr_channel.as_ref().map(|(channel, _)| channel.clone()))
                .or_else(|| wc3_channel.as_ref().map(|(channel, _)| channel.clone())),
            online: if self.session.is_sc2() {
                self.session
                    .channels
                    .active()
                    .map(|channel| channel.users.len())
            } else {
                scr_channel
                    .map(|(_, online)| online)
                    .or_else(|| wc3_channel.map(|(_, online)| online))
            },
            progress,
            step,
        };
        // the report describes the focused session, so it belongs to the
        // focused product's card and to no other
        let product = self.focused;
        let Some(card) = card_index(product) else {
            return;
        };
        let changed = self.games.live.get(&product).is_none_or(|current| {
            current.state != live.state
                || current.channel != live.channel
                || current.online != live.online
                || current.handle != live.handle
                || current.progress != live.progress
                || current.step != live.step
        });
        if changed {
            self.games.set_card(card, live.state);
            self.games.live.insert(product, live);
        }
    }

    /// The card system, laid out the way the design lays it out: the states
    /// walked one at a time against a hero card, the picker those cards
    /// actually make, the three palettes side by side in the same state, and
    /// the tokens they each read.
    pub(in crate::app::client) fn games_screen(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = Instant::now();
        let size = picker_size(f32::from(window.viewport_size().width));
        let walk = Walk {
            from: self.games.previous,
            to: self.games.state,
            since: self.games.advanced,
            now,
        };
        let hero = &GAMES[self.games.hero];
        let sheet = div()
            .id("games-sheet")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.games.scroll)
            .flex()
            .flex_col()
            .child(masthead())
            .child(hero_section(hero, walk, cx))
            .child(picker_section(&self.games, size, now, false, cx))
            .child(together_section(walk, cx))
            .child(palette_section());
        div()
            .id("games-screen")
            .size_full()
            .bg(rgb(SHEET))
            .font_family(FONT_INTERFACE)
            .child(sheet)
            // the design sheet is Superiority's own chrome, not a realm's:
            // the system bar is its bar
            .vertical_scrollbar_for(&self.games.scroll, window, cx)
            .into_any_element()
    }
}

impl SuperiorityView {
    /// The picker on its own — the screen the cards are actually for, without
    /// the sheet that explains them. It answers to the keyboard as well as the
    /// pointer, and lays itself out for the window it is given.
    pub(in crate::app::client) fn picker_screen(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = Instant::now();
        let size = picker_size(f32::from(window.viewport_size().width));
        // the picker is the whole window here: the room reaches every edge and
        // the cards sit in the middle of it, with nothing else to make room for
        div()
            .id("games-only")
            .size_full()
            .flex()
            .flex_col()
            .font_family(FONT_INTERFACE)
            .bg(rgb(SHEET))
            .child(picker_titlebar(
                self.runtime.authoritative_battle_tag.as_deref(),
                self.runtime.authoritative_region,
                &self.games,
                self.realms_in_flight(),
                now,
                window,
                cx,
            ))
            .child(picker_section(&self.games, size, now, true, cx))
            .into_any_element()
    }
}
