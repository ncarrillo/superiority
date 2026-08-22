//! StarCraft: Remastered's Terran-console roster tokens.
//!
//! The terminal pass (`Canvas-3`) retired the phosphor green: red IS the
//! chrome. Structure is drawn in the redline hairline, voices in a rust
//! temperature ramp — brightness carries meaning, not hue.

pub const ROSTER_WIDTH: f32 = 300.0;
pub const ROSTER_ROW_HEIGHT: f32 = 40.0;
pub const ROSTER_ROW_GAP: f32 = 1.0;
pub const ROSTER_SEGMENT_HEIGHT: f32 = 22.0;

pub const FONT_INTERFACE: &str = "Eurostile";
pub const FONT_INTERNATIONAL: &str = "BlizzardGlobal";

pub const PANEL_BACKGROUND: u32 = 0x0006_0302;
pub const PANEL_HEADER: u32 = 0x000a_0404;
pub const PANEL_SHELL: u32 = 0x0004_0302;
pub const BORDER_STRUCTURAL: u32 = 0xc93a_2c80;
pub const BORDER_FOCUSED: u32 = 0x00ff_6a58;
pub const TEXT: u32 = 0x00ff_d0c8;
pub const MUTED: u32 = 0x008a_5a50;
pub const ACCENT: u32 = 0x00ff_8a78;
