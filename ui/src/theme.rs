use gpui::{BoxShadow, px, rgba};

pub const WINDOW_WIDTH: f32 = 1180.0;
pub const WINDOW_HEIGHT: f32 = 780.0;
pub const MARGIN: f32 = 22.0;
pub const ROSTER_WIDTH: f32 = 300.0;
pub const COMPOSER_HEIGHT: f32 = 42.0;
pub const TAB_BAR_HEIGHT: f32 = 43.0;
pub const TAB_HEIGHT: f32 = 36.0;
pub const TAB_TOP: f32 = TAB_BAR_HEIGHT - TAB_HEIGHT;
pub const TAB_LEADING_SPACE: f32 = 116.0;
pub const TAB_NAME_INSET: f32 = 38.0;
pub const TAB_NAME_LEAD: f32 = 14.0;
/// member rows are one line of chrome, not a card: a 28px portrait, a name,
/// and a status dot. everything that used to need a second line either moved
/// into the dot or into the segment header above the row.
pub const ROSTER_ROW_HEIGHT: f32 = 40.0;
pub const ROSTER_ROW_GAP: f32 = 1.0;
/// segment headers occupy a full row slot so the list stays uniform for
/// virtualization; the label sits at the bottom of it, which turns the leftover
/// space into the gap between one band and the next.
pub const ROSTER_SEGMENT_HEIGHT: f32 = 22.0;

pub const FONT_NAVIGATION: &str = "Eurostile Extd";
pub const FONT_INTERFACE: &str = "Eurostile";
pub const FONT_INTERNATIONAL: &str = "BlizzardGlobal";

pub const BACKGROUND: u32 = 0x000b_0f15;
pub const PANEL_BACKGROUND: u32 = 0x0006_0a0f;
pub const PANEL_BORDER: u32 = 0x0013_3e5b;
pub(crate) const PANEL_HEADER: u32 = 0x0009_1016;
pub(crate) const PANEL_SHELL: u32 = 0x000b_121a;
pub const TEXT: u32 = 0x00d6_e0f0;
pub const MUTED: u32 = 0x007d_8fa8;
pub(crate) const ACCENT: u32 = 0x0033_a8f0;
pub const NOTICE: u32 = 0x006b_c2f2;
pub const ONLINE: u32 = 0x0047_d184;

/// border tier 1 — structural chrome. window seams, panel strokes, list
/// dividers, and the settings content area all recede at this weight: 1px, no
/// glow. every surface that is merely *there* uses this and nothing brighter.
pub const BORDER_STRUCTURAL: u32 = 0x133e_5bb3;
/// border tier 2 — the focused surface. open popover, selected tab, selected
/// thumbnail, focused input: 1px bright stroke paired with one of the glows
/// below. only one surface on screen should be wearing it.
pub const BORDER_FOCUSED: u32 = 0x0033_a8f0;

/// tier 2 glow for small focused surfaces — a tab, a nav item, an input. outer
/// bloom only: an inset pass reads as a flood fill at this size, not a lift.
#[must_use]
pub fn focus_glow() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f04d).into()).blur_radius(px(14.0))]
}

/// tier 2 glow for image tiles, which have no fill to lift from the inside.
#[must_use]
pub fn selection_glow() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f073).into()).blur_radius(px(16.0))]
}

/// tier 2 glow for a single hairline edge — the popover's top and bottom rails.
#[must_use]
pub fn edge_glow() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.0), px(0.0), rgba(0x33a8_f0b3).into()).blur_radius(px(10.0))]
}

/// lift for a surface that floats over the chat — the /join popup rides above
/// the composer with nothing but shadow between it and the transcript, so the
/// shadow is thrown upward rather than spread evenly.
#[must_use]
pub fn popup_lift() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.0), px(-8.0), rgba(0x0000_0099).into()).blur_radius(px(30.0))]
}
