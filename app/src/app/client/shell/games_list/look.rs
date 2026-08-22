//! A card's look mid-transition now lives in the shared crate, resolved over a
//! generic animation clock so the desktop (`Instant`) and the browser viewer
//! (`f64` ms) paint one card system. See [`superiority_ui::products::games`].

pub(in crate::app::client) use superiority_ui::products::games::CardLook;
