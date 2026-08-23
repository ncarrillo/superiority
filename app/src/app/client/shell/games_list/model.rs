//! the card fixture, palette, and state enum now live in the shared crate so
//! the desktop shell and the browser viewer draw one card system rather than
//! two copies. See [`superiority_ui::products::games`].
//!
//! The only host-visible difference is [`GamePalette::art`], which is an
//! [`AssetPaths`](superiority_ui::foundation::assets::AssetPaths) pair (native
//! and web) rather than a bare path; desktop renderers read its `.native` side.

pub(in crate::app::client) use superiority_ui::products::games::{
    CARD_STATES, CardState, GAMES, GamePalette,
};
