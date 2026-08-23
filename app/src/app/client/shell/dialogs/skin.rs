//! a dialog's insides in the focused realm's own language — the update
//! dialog today. The shell, title, close glyph, buttons, and scrollbar already
//! dress per product; this is the ink inside the plate: fonts, the text ramp,
//! the notes panel, and the status line with its rail. One skin per
//! `ModalVariant`, the way the settings dialog has, so an update on the Terran
//! console reads like the console and one in the realm hall reads like stone.

use super::*;

/// colours are `0xRRGGBB` unless the name says `_edge`, which is `0xRRGGBBAA`.
#[derive(Clone, Copy)]
pub(in crate::app::client) struct DialogSkin {
    /// body, status, and captions.
    pub(in crate::app::client) interface_font: &'static str,
    pub(in crate::app::client) heading_font: &'static str,
    pub(in crate::app::client) text: u32,
    pub(in crate::app::client) muted: u32,
    /// the notes panel behind the release notes.
    pub(in crate::app::client) panel_fill: u32,
    pub(in crate::app::client) panel_edge: u32,
    /// the status line and its rail: done, under way, failed.
    pub(in crate::app::client) ok: u32,
    pub(in crate::app::client) busy: u32,
    pub(in crate::app::client) error: u32,
    /// the rail's unfilled length.
    pub(in crate::app::client) track: u32,
    pub(in crate::app::client) notes: ui_release_notes::NotesStyle,
}

impl DialogSkin {
    pub(in crate::app::client) const fn for_variant(
        variant: ui_shared_modal::ModalVariant,
    ) -> Self {
        match variant {
            ui_shared_modal::ModalVariant::Sc2 => Self {
                interface_font: FONT_INTERFACE,
                heading_font: FONT_INTERFACE,
                text: 0x00d6_e0f0,
                muted: 0x007d_8fa8,
                panel_fill: 0x0006_0a0f,
                panel_edge: BORDER_STRUCTURAL,
                ok: 0x0047_d185,
                busy: 0x0033_a8f0,
                error: 0x00f2_705c,
                track: 0x0009_1016,
                notes: ui_release_notes::NotesStyle::SC2,
            },
            // the Terran console: brightness carries the meaning, not hue.
            // done is the top of the rust ramp, under way the middle, and only
            // a failure gets the hot chrome red
            ui_shared_modal::ModalVariant::Remastered => Self {
                interface_font: ui_scr_theme::FONT_INTERFACE,
                heading_font: ui_scr_theme::FONT_INTERFACE,
                text: 0x00f0_e6da,
                muted: ui_scr_theme::MUTED,
                panel_fill: ui_scr_theme::PANEL_BACKGROUND,
                panel_edge: ui_scr_theme::BORDER_STRUCTURAL,
                ok: ui_scr_theme::TEXT,
                busy: 0x00d8_8070,
                error: ui_scr_theme::BORDER_FOCUSED,
                track: 0x002a_0c0a,
                notes: ui_release_notes::NotesStyle::REMASTERED,
            },
            // the realm hall: parchment for the words, gold while it works,
            // moss when it is done, blood when it is not
            ui_shared_modal::ModalVariant::Reforged => Self {
                interface_font: ui_wc3_theme::FONT_INTERFACE,
                heading_font: ui_wc3_theme::FONT_TITLE,
                text: ui_wc3_theme::PARCHMENT,
                muted: ui_wc3_theme::MUTED,
                panel_fill: ui_wc3_theme::PANEL,
                panel_edge: 0x5e4a_26ff,
                ok: ui_wc3_theme::MOSS,
                busy: ui_wc3_theme::GOLD,
                error: ui_wc3_theme::BLOOD,
                track: 0x002a_2012,
                notes: ui_release_notes::NotesStyle::REFORGED,
            },
        }
    }
}
