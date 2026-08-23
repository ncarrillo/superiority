//! The card system's data: the six states, the per-game palette, the three
//! design-time games, and the sizes the card is drawn at. Nothing here reads a
//! live session — a host adapts its session into [`CardData`]/[`LiveCard`] and
//! hands that to the renderers.

use crate::foundation::assets::AssetPaths;
use crate::products::sc2::theme::{FONT_INTERFACE, FONT_NAVIGATION};

/// The six states a game card is ever in. Structure and behaviour never change
/// between them — only what the card is dressed in — so they are a table rather
/// than six views.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub enum CardState {
    /// Resting. The art is dimmed and the bevel is at half brightness.
    #[default]
    Idle,
    /// The realm wakes: the art blooms and zooms and the ping appears.
    Focused,
    /// A scan beam sweeps the art while the status names the protocol stage.
    Connecting,
    /// The badge lands, the status turns green, the verb flips to ENTER.
    Connected,
    /// No identity here yet. Grey, and the bevel drops to a whisper.
    NoAccount,
    /// Cold, with an ember beacon and a retry countdown.
    Unreachable,
}

pub const CARD_STATES: [CardState; 6] = [
    CardState::Idle,
    CardState::Focused,
    CardState::Connecting,
    CardState::Connected,
    CardState::NoAccount,
    CardState::Unreachable,
];

impl CardState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Focused => "HOVER / FOCUSED",
            Self::Connecting => "CONNECTING",
            Self::Connected => "CONNECTED",
            Self::NoAccount => "NO ACCOUNT",
            Self::Unreachable => "UNREACHABLE",
        }
    }

    #[must_use]
    pub const fn note(self) -> &'static str {
        match self {
            Self::Idle => "Resting. Art dimmed, bevel at half brightness. The card waits.",
            Self::Focused => {
                "The realm wakes: art blooms and zooms, its focus colour floods the floor, ping appears."
            }
            Self::Connecting => {
                "Scan beam sweeps the art while the status line names the protocol stage."
            }
            Self::Connected => {
                "Badge lands on the art, status turns green, the verb flips to ENTER."
            }
            Self::NoAccount => {
                "Grey — no identity here yet. The bevel stays but drops to a whisper."
            }
            Self::Unreachable => "Cold, ember beacon pulsing. Retry countdown runs inline.",
        }
    }

    /// What the button says. The verb is the state — there is no separate
    /// enabled flag to fall out of step with it.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Idle | Self::Focused => "CONNECT",
            Self::Connecting => "CONNECTING…",
            Self::Connected => "ENTER",
            Self::NoAccount => "SIGN IN",
            Self::Unreachable => "RETRY NOW",
        }
    }

    /// Only a card you can actually enter offers the return key.
    #[must_use]
    pub const fn shows_return_key(self) -> bool {
        matches!(self, Self::Connected)
    }

    /// How much of the art's own light is left. gpui has no brightness filter,
    /// so this is opacity over the card's near-black fill, which comes to the
    /// same thing.
    #[must_use]
    pub const fn art_light(self) -> f32 {
        match self {
            Self::Focused => 1.0,
            Self::Connected => 0.9,
            Self::Connecting => 0.75,
            Self::Idle | Self::Unreachable => 0.5,
            Self::NoAccount => 0.45,
        }
    }

    /// The zoom is drawn by overhanging the frame rather than by a transform:
    /// the art is laid out larger than the window it shows through.
    #[must_use]
    pub const fn art_overhang(self) -> f32 {
        match self {
            Self::Focused => 10.0,
            Self::Connecting => 4.0,
            Self::Connected => 6.0,
            Self::Idle | Self::NoAccount | Self::Unreachable => 0.0,
        }
    }

    /// A card with nothing behind it loses its colour entirely.
    #[must_use]
    pub const fn colourless(self) -> bool {
        matches!(self, Self::NoAccount | Self::Unreachable)
    }

    #[must_use]
    pub const fn card_light(self) -> f32 {
        match self {
            Self::NoAccount => 0.75,
            _ => 1.0,
        }
    }

    #[must_use]
    pub const fn lit(self) -> bool {
        matches!(self, Self::Focused | Self::Connecting | Self::Connected)
    }

    #[must_use]
    pub const fn scanning(self) -> bool {
        matches!(self, Self::Connecting)
    }

    #[must_use]
    pub const fn badged(self) -> bool {
        matches!(self, Self::Connected)
    }

    #[must_use]
    pub const fn ember(self) -> bool {
        matches!(self, Self::Unreachable)
    }

    /// How far the handshake has got. `None` draws no bar at all rather than an
    /// empty one, which would read as stalled.
    #[must_use]
    pub const fn progress(self) -> Option<f32> {
        match self {
            Self::Connecting => Some(0.7),
            _ => None,
        }
    }
}

/// Everything a card reads, per game. Behaviour and structure never change;
/// only these tokens do, which is what lets one card definition dress itself as
/// three games.
#[derive(Clone, Copy)]
pub struct GamePalette {
    pub name: &'static str,
    pub program: &'static str,
    /// Whether this client can actually take you into the game. Each enabled
    /// product has its own recovered protocol and realm presentation.
    pub playable: bool,
    /// Whether the picker draws this card at all.
    ///
    /// A game with nothing behind it yet is hidden rather than shown dark.
    pub shown: bool,
    pub font: &'static str,
    /// The title is set in the game's own voice: shouted for the two space
    /// games, spoken for the fantasy one. gpui has no small caps, so the
    /// difference is spelled by casing the string itself.
    pub upper: bool,
    /// The realm's art, as the two paths a host resolver chooses between.
    pub art: AssetPaths,
    pub card_fill: u32,
    pub fade: u32,
    pub focus: u32,
    pub glow: u32,
    pub ok: u32,
    pub err: u32,
    pub err_light: u32,
    pub text: u32,
    pub dim: u32,
    pub sub: u32,
    pub clan: u32,
    /// The edge a resting card wears, and the fainter one a card with nobody
    /// behind it does. Like every colour the card blends, these are
    /// `0x00RRGGBB`: a transition mixes them channel by channel, so an alpha
    /// byte hiding in one of them would be mixed as if it were blue.
    pub structural: u32,
    pub structural_dim: u32,
    /// The realm's own accent for quiet status: the CONNECTED beacon, the
    /// destination arrow, and the button's resting ink. A solid green chip on
    /// all three cards was the loudest thing on the page; this is the same
    /// fact said in each card's voice (`Canvas-2` critique, 1).
    pub beacon: u32,
    /// Reforged's art zone is painted light, not photographed: ember from
    /// below-left, moonlight at the top-right corner (`Canvas-2` critique, 4).
    pub torchlit: bool,
    /// The identity this game knows you by, which is not the same on all
    /// three: `StarCraft: Remastered` has no clans to tag you with.
    pub clan_tag: Option<&'static str>,
    pub handle: &'static str,
    pub region: &'static str,
    pub ping: &'static str,
    pub channel: &'static str,
    /// What the picker says about this game when it is not being walked
    /// through its states: where you were last, or who is in there now.
    pub recency: &'static str,
    /// The state the picker shows this game in. One realm is connected and the
    /// others are not, which is the ordinary case rather than a demonstration.
    pub resting: CardState,
    /// The protocol stage, named in the game's own words.
    pub handshake: &'static str,
    /// How this game's type and buttons are described in the palette table.
    pub type_note: &'static str,
    /// The name the palette table gives this dressing.
    pub dressing: &'static str,
}

impl GamePalette {
    #[must_use]
    pub fn title_lower(&self) -> String {
        self.name.to_owned()
    }

    #[must_use]
    pub fn reference(&self) -> [(String, u32); 4] {
        [
            (
                format!(
                    "focus {:06x} · ok {:06x} · err {:06x}",
                    self.focus, self.ok, self.err
                ),
                self.sub,
            ),
            (
                format!(
                    "text {:06x} / {:06x} / {:06x}",
                    self.text, self.dim, self.sub
                ),
                self.sub,
            ),
            (self.type_note.to_owned(), self.sub),
            (format!("handshake line: {}", self.handshake), self.sub),
        ]
    }

    #[must_use]
    pub fn title(&self) -> String {
        if self.upper {
            self.name.to_uppercase()
        } else {
            self.name.to_owned()
        }
    }

    /// The clan tag the status line opens with, when the state is one that
    /// names you at all. It is split out because it takes the game's clan
    /// colour rather than the colour of the line it sits on.
    #[must_use]
    pub const fn status_tag(&self, state: CardState) -> Option<&'static str> {
        match state {
            CardState::Idle | CardState::Focused => self.clan_tag,
            _ => None,
        }
    }

    /// The rest of that line. It says something different in every state, which
    /// is most of what tells the states apart at a glance.
    #[must_use]
    pub fn status(&self, state: CardState) -> String {
        match state {
            CardState::Idle => format!("{} · {}", self.handle, self.region),
            CardState::Focused => {
                format!("{} · {} · {}", self.handle, self.region, self.ping)
            }
            CardState::Connecting => self.handshake.to_owned(),
            CardState::Connected => format!("{} · 1 online · {}", self.channel, self.ping),
            CardState::NoAccount => "No profile on this network yet".to_owned(),
            CardState::Unreachable => "Unreachable · retrying in 0:07".to_owned(),
        }
    }

    /// The colour that line takes: the state's own where it has one, and the
    /// game's ordinary body colour where it does not.
    #[must_use]
    pub const fn status_colour(&self, state: CardState) -> u32 {
        match state {
            CardState::Connecting => self.focus,
            CardState::Connected => self.ok,
            CardState::NoAccount => self.sub,
            CardState::Unreachable => self.err_light,
            CardState::Idle | CardState::Focused => self.dim,
        }
    }

    /// The edge the card wears. A lit card takes its focus colour, an
    /// unreachable one its error colour, and a card with nobody behind it
    /// recedes further than an idle one.
    #[must_use]
    pub const fn border(&self, state: CardState) -> u32 {
        match state {
            CardState::Focused | CardState::Connecting | CardState::Connected => self.focus,
            CardState::Unreachable => self.err,
            CardState::NoAccount => self.structural_dim,
            CardState::Idle => self.structural,
        }
    }

    #[must_use]
    pub const fn title_colour(&self, state: CardState) -> u32 {
        if state.lit() { self.text } else { self.dim }
    }
}

/// The three games, with the design-time identity the cards are dressed in.
/// Nothing here is read from a live session — this is the card system's own
/// fixture, so the six states can be looked at without three accounts and a
/// broken network.
pub const GAMES: [GamePalette; 3] = [
    GamePalette {
        name: "StarCraft II",
        // confirmed against a live account state, and the same code the client
        // sends when it asks Battle.net for web credentials
        program: "S2",
        playable: true,
        shown: true,
        font: FONT_NAVIGATION,
        upper: true,
        art: AssetPaths::new(
            "images/backgrounds/shakuras-nebula.png",
            "ui/backgrounds/shakuras-nebula.png",
        ),
        card_fill: 0x0208_10f2,
        fade: 0x0208_10f2,
        focus: 0x0033_a8f0,
        glow: 0x33a8_f059,
        ok: 0x0047_d184,
        err: 0x00e8_734a,
        err_light: 0x00e8_a58a,
        text: 0x00e6_f9ff,
        dim: 0x00a9_b8cc,
        sub: 0x007d_8fa8,
        clan: 0x00f0_aa64,
        structural: 0x0013_3e5b,
        structural_dim: 0x000d_2a3f,
        beacon: 0x006b_c2f2,
        torchlit: false,
        clan_tag: Some("<MDGTN>"),
        handle: "ncarrillo",
        region: "Americas",
        ping: "32 ms",
        channel: "Midigation",
        recency: "Midigation · 1 online",
        resting: CardState::Connected,
        handshake: "Handshake · AUTH_LOGON_CHALLENGE",
        type_note: "type: Eurostile Extd caps · button: engraved texture",
        dressing: "SC2 · HEX GLASS",
    },
    GamePalette {
        name: "StarCraft: Remastered",
        program: "S1",
        playable: true,
        shown: true,
        font: FONT_INTERFACE,
        upper: true,
        art: AssetPaths::new(
            "images/backgrounds/swarm-horizon.png",
            "ui/backgrounds/swarm-horizon.png",
        ),
        card_fill: 0x0408_06f2,
        fade: 0x0408_06f2,
        focus: 0x004a_e08a,
        glow: 0x4ae0_8a59,
        ok: 0x004a_e08a,
        err: 0x00e8_734a,
        err_light: 0x00e8_a58a,
        text: 0x00a8_f0c0,
        dim: 0x008f_c9a4,
        sub: 0x005e_8a6e,
        clan: 0x00d8_e09a,
        structural: 0x002e_4636,
        structural_dim: 0x001f_2f24,
        beacon: 0x005f_d8a8,
        torchlit: false,
        clan_tag: None,
        handle: "ncarrillo",
        region: "U.S. East (Azeroth)",
        ping: "54 ms",
        channel: "The Void",
        recency: "last: The Void · 3d ago",
        resting: CardState::Idle,
        handshake: "Logon · SID_AUTH_INFO",
        type_note: "type: mono caps · button: carved console bevel",
        dressing: "SC:R · TERRAN CONSOLE",
    },
    GamePalette {
        name: "Warcraft III: Reforged",
        program: "W3",
        playable: true,
        shown: true,
        font: crate::products::wc3::theme::FONT_TITLE,
        upper: false,
        art: AssetPaths::new(
            "images/backgrounds/wc3-orc-standard.png",
            "ui/backgrounds/wc3-orc-standard.png",
        ),
        card_fill: 0x1610_0af2,
        fade: 0x0d09_06f2,
        focus: 0x00e8_c874,
        glow: 0xe8c8_7459,
        ok: 0x007c_c46a,
        err: 0x00e0_7a4a,
        err_light: 0x00f0_b394,
        text: 0x00f2_e8d0,
        dim: 0x00e8_dcc0,
        sub: 0x009c_8a6e,
        clan: 0x00e8_c874,
        structural: 0x004a_3a22,
        structural_dim: 0x0032_2717,
        beacon: 0x00e8_c874,
        torchlit: true,
        clan_tag: Some("<MDGTN>"),
        handle: "ncarrillo",
        region: "Northrend",
        ping: "41 ms",
        channel: "Clan Midigation",
        recency: "last: Clan Midigation · yesterday",
        resting: CardState::Idle,
        handshake: "Realm · AuroraChat bootstrap",
        type_note: "type: small-caps serif · button: stone bevel gradient",
        dressing: "WC3:R · Stone & Gold",
    },
];

/// What a host's live session says about the game behind a card. A host adapts
/// its own session shape into this; the renderers read only this, so the card
/// says what the session says or says nothing.
#[derive(Clone, Default)]
pub struct LiveCard {
    /// Who you are in there — the clan tag and name the roster knows.
    pub clan_tag: Option<String>,
    pub handle: Option<String>,
    /// Where you are, and how many are there with you.
    pub channel: Option<String>,
    /// The Battle.net region this product's session came in through.
    pub region: Option<u32>,
    pub online: Option<usize>,
    /// What the connection is doing, while it is doing it.
    pub progress: Option<String>,
    /// "Step 2 of 4" while the handshake runs.
    pub step: Option<String>,
}

/// Everything a card renderer needs from the host to draw one card: which state
/// it rests in, whether the account owns and can play it, whether this is a
/// live client rather than the design screens, and the live session behind it.
#[derive(Clone)]
pub struct CardData {
    pub state: CardState,
    pub owned: bool,
    pub licensed: bool,
    pub live_mode: bool,
    pub live: Option<LiveCard>,
}

/// The region this session came in through, in the words Battle.net uses for
/// them. The ids are the service's own; anything we have no name for is left
/// unsaid rather than guessed at.
#[must_use]
pub fn region_name(region: Option<u32>) -> Option<&'static str> {
    match region? {
        1 => Some("Americas"),
        2 => Some("Europe"),
        3 => Some("Korea"),
        5 => Some("China"),
        _ => None,
    }
}

/// The card is drawn at three sizes and is otherwise one definition: the hero
/// where a single state is being read, the picker where the realms are chosen
/// between, and the trio where all three palettes are compared at once.
#[derive(Clone, Copy)]
pub struct CardSize {
    pub width: f32,
    pub art: f32,
    pub pad_x: f32,
    pub pad_top: f32,
    pub pad_bottom: f32,
    pub gap: f32,
    pub title: f32,
    pub status: f32,
    pub button: f32,
    pub badge: f32,
    /// The gutter the section keeps at the window's edge, and the gap between
    /// cards. Both close up as the window narrows, because a card is worth
    /// more than the air beside it.
    pub section_pad: f32,
    pub gap_between: f32,
}

pub const HERO_CARD: CardSize = CardSize {
    width: 320.0,
    art: 170.0,
    pad_x: 20.0,
    pad_top: 18.0,
    pad_bottom: 20.0,
    gap: 12.0,
    title: 14.5,
    status: 13.0,
    button: 36.0,
    badge: 9.5,
    section_pad: 48.0,
    gap_between: 32.0,
};

/// Three abreast, which is what the design draws.
pub const PICKER_WIDE: CardSize = CardSize {
    width: 290.0,
    art: 150.0,
    ..HERO_CARD
};

/// Narrow enough that three no longer fit: the cards give up some width before
/// they give up their row.
pub const PICKER_MEDIUM: CardSize = CardSize {
    width: 260.0,
    art: 136.0,
    section_pad: 28.0,
    gap_between: 20.0,
    ..HERO_CARD
};

/// One or two across, with the art shorter so a stacked card still shows its
/// face and its lines without scrolling.
pub const PICKER_NARROW: CardSize = CardSize {
    width: 300.0,
    art: 120.0,
    section_pad: 18.0,
    gap_between: 16.0,
    ..HERO_CARD
};

/// The card sizes itself to the window rather than to the design's one
/// screenshot: three abreast where there is room, fewer where there is not.
#[must_use]
pub fn picker_size(width: f32) -> CardSize {
    const THREE_ABREAST: f32 =
        PICKER_WIDE.width * 3.0 + PICKER_WIDE.gap_between * 2.0 + PICKER_WIDE.section_pad * 2.0;
    const TWO_ABREAST: f32 =
        PICKER_MEDIUM.width * 2.0 + PICKER_MEDIUM.gap_between + PICKER_MEDIUM.section_pad * 2.0;
    if width >= THREE_ABREAST {
        PICKER_WIDE
    } else if width >= TWO_ABREAST {
        PICKER_MEDIUM
    } else {
        PICKER_NARROW
    }
}

pub const TRIO_CARD: CardSize = CardSize {
    width: 280.0,
    art: 130.0,
    pad_x: 16.0,
    pad_top: 14.0,
    pad_bottom: 16.0,
    gap: 10.0,
    title: 13.0,
    status: 12.0,
    button: 32.0,
    badge: 9.0,
    section_pad: 20.0,
    gap_between: 28.0,
};

#[cfg(test)]
mod tests {
    use super::{
        CARD_STATES, CardState, GAMES, PICKER_MEDIUM, PICKER_NARROW, PICKER_WIDE, picker_size,
    };

    #[test]
    fn every_state_says_something_different_under_the_title() {
        // the status line is most of what tells the six apart at a glance, so
        // no two of them may read the same
        for palette in &GAMES {
            let mut lines = CARD_STATES
                .iter()
                .map(|state| palette.status(*state))
                .collect::<Vec<_>>();
            lines.sort();
            let before = lines.len();
            lines.dedup();
            assert_eq!(
                lines.len(),
                before,
                "{} repeats a status line",
                palette.name
            );
        }
    }

    #[test]
    fn the_verb_is_the_state() {
        assert_eq!(CardState::Idle.verb(), "CONNECT");
        assert_eq!(CardState::Connected.verb(), "ENTER");
        assert_eq!(CardState::NoAccount.verb(), "SIGN IN");
        assert_eq!(CardState::Unreachable.verb(), "RETRY NOW");
        assert!(CardState::Connected.shows_return_key());
        assert!(!CardState::Connecting.shows_return_key());
    }

    #[test]
    fn only_one_state_at_a_time_wears_each_mark() {
        // the badge, the beam and the ember are three different answers to
        // "what is this card doing", so they never appear together
        for state in CARD_STATES {
            let marks =
                u8::from(state.badged()) + u8::from(state.scanning()) + u8::from(state.ember());
            assert!(marks <= 1, "{state:?} wears more than one mark");
        }
        assert!(CardState::Connecting.progress().is_some());
        assert!(CardState::Connected.progress().is_none());
    }

    #[test]
    fn identity_reads_the_way_each_game_knows_you() {
        // the tag is only shown where the state names you at all, and only by
        // the games that have clans to name you with
        assert_eq!(GAMES[0].status_tag(CardState::Idle), Some("<MDGTN>"));
        assert_eq!(GAMES[1].status_tag(CardState::Idle), None);
        assert_eq!(GAMES[0].status_tag(CardState::Connected), None);

        assert_eq!(GAMES[0].title(), "STARCRAFT II");
        assert_eq!(GAMES[2].title(), "Warcraft III: Reforged");
    }

    #[test]
    fn the_row_gives_up_width_before_it_gives_up_its_row() {
        let same = |left: f32, right: f32| (left - right).abs() < f32::EPSILON;
        assert!(same(picker_size(1400.0).width, PICKER_WIDE.width));
        assert!(same(picker_size(900.0).width, PICKER_MEDIUM.width));
        assert!(same(picker_size(500.0).width, PICKER_NARROW.width));
        assert!(picker_size(500.0).section_pad < picker_size(1400.0).section_pad);
    }
}
