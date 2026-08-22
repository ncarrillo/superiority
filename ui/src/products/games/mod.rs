//! The game-picker card system, shared by the desktop shell and the browser
//! viewer. A [`GamePalette`] dresses one card definition as three games; a
//! [`CardLook`] is that card resolved mid-transition; the [`view`] renderers
//! paint it. Everything here is host-neutral: a host adapts its session into
//! [`CardData`]/[`LiveCard`] and resolves [`GamePalette::art`] with its own
//! [`AssetResolver`](crate::foundation::assets::AssetResolver), then hands the
//! resolved [`ImageSource`](gpui::ImageSource) to the renderers.

mod look;
mod model;
pub mod motion;
mod picker_state;
mod view;

pub use look::{CardLook, ROOM, eased};
pub use model::{
    CARD_STATES, CardData, CardSize, CardState, GAMES, GamePalette, HERO_CARD, LiveCard,
    PICKER_MEDIUM, PICKER_NARROW, PICKER_WIDE, TRIO_CARD, picker_size, region_name,
};
pub use motion::{CardMotion, Motion, StageMotion};
pub use picker_state::{Advance, PickerState};
pub use view::{
    card_art, card_button, card_shell, destination_line, destination_parts, ember_beacon, fade,
    game_card, identity_parts, identity_region_line, marker_colour, masthead, personality,
    progress_rail, realm_flood, room_layer, scan_beam, status_line, titlecase, tracked,
};
