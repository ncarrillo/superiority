//! presentation owned by an individual game product.

/// the modern button, dressed per product — the legacy nine-patch
/// `action_button` stays until this one is validated.
pub mod buttons;
/// the game-picker card system, shared by the desktop shell and the browser
/// viewer — one card definition dressed as three games.
pub mod games;
/// the modern input field and checkbox, dressed per product.
pub mod inputs;
/// the shared modal shell, dressed per product — it belongs to all of them
/// at once, so it sits beside them rather than under any one.
pub mod modal;
pub mod sc2;
pub mod scr;
pub mod wc3;
