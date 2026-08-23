//! WC3:R product tokens, corrected to the design language (`Realm Chat
//! Windows`, WC3:R SHELL): the same stone, gold, parchment, and ember the
//! shared modal and the game card wear — no palette of its own, and zero
//! web-blue.

pub const FONT_INTERFACE: &str = "BlizzardGlobal";
/// Friz Quadrata, Reforged's own UI face, extracted from the WC3:R client and
/// embedded by both hosts — the game's type, not a host-dependent Palatino.
pub const FONT_TITLE: &str = "Friz Quadrata TT";

/// the window itself. The stone lives inside a plain OS border.
pub const SHELL: u32 = 0x000b_0805;
/// the panels' ground: the roster field and the input wells.
pub const PANEL: u32 = 0x000a_0704;

/// bronze-gold structure, brightest first: rank-tile and plaque edges, the
/// composer's resting border, and the quietest wells.
pub const STONE_BRIGHT: u32 = 0x008a_6d3b;
pub const STONE: u32 = 0x005e_4a26;
pub const STONE_DIM: u32 = 0x003a_2d18;

/// the gold voice, brightest last.
pub const GOLD: u32 = 0x00e8_c874;
pub const GOLD_BRIGHT: u32 = 0x00ff_e9a8;
/// gold at rest: the popout rows before the pointer finds them.
pub const GOLD_DIM: u32 = 0x00c8_b088;

pub const PARCHMENT: u32 = 0x00f2_e8d0;
pub const MUTED: u32 = 0x009c_8a6e;
/// the hall's own silence: empty-state words.
pub const QUIET: u32 = 0x0054_462e;

/// ember, for severing things.
pub const EMBER: u32 = 0x00c8_8a6a;
pub const EMBER_BRIGHT: u32 = 0x00ff_d0b0;

/// the presence dot.
pub const MOSS: u32 = 0x007c_c46a;
/// errors in the transcript.
pub const BLOOD: u32 = 0x00a6_322b;

/// the titlebar is the channel-tab strip: the same 43px StarCraft II's bar
/// stands (`Channel Tabs` design), in stone.
pub const TITLEBAR_HEIGHT: f32 = crate::products::sc2::theme::TAB_BAR_HEIGHT;
pub const COMPOSER_HEIGHT: f32 = 34.0;
pub const ROSTER_WIDTH: f32 = 250.0;
pub const ROSTER_ROW_HEIGHT: f32 = 42.0;
pub const ROSTER_ROW_GAP: f32 = 1.0;
pub const ROSTER_SEGMENT_HEIGHT: f32 = 22.0;
