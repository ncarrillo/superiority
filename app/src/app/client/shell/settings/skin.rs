//! the settings dialog in the focused realm's own language.
//!
//! The shell, title, buttons, and checkboxes already dress per product; this
//! is everything inside the plate — fonts, the ink ramp, the page rail, the
//! page frame and footer, the background tiles' selection, and the tooltip
//! card. One skin per `ModalVariant`, read from the same variant the root
//! hands every dialog, so settings on the Terran console reads like the
//! console and settings in the realm hall reads like stone.

use super::*;

/// how a glow is cast around a focused or chosen thing: the realm's light at
/// the given alpha.
fn glow(colour: u32, blur: f32) -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow::new(px(0.0), px(0.0), rgba(colour).into()).blur_radius(px(blur))]
}

/// the dialog's dressing for one realm. Colours are `0xRRGGBB` unless the
/// name says `_fill`/`_edge`/`_wash`, which are `0xRRGGBBAA`.
#[derive(Clone, Copy)]
pub(in crate::app::client) struct SettingsSkin {
    pub(in crate::app::client) variant: ui_shared_modal::ModalVariant,
    /// body and labels.
    pub(in crate::app::client) interface_font: &'static str,
    /// page headings ("Appearance", "Chat", "Live").
    pub(in crate::app::client) heading_font: &'static str,
    /// the page rail's words.
    pub(in crate::app::client) nav_font: &'static str,
    /// the tooltip's words.
    pub(in crate::app::client) tooltip_font: &'static str,
    pub(in crate::app::client) text: u32,
    pub(in crate::app::client) muted: u32,
    /// the option labels beside checkboxes and the section captions.
    pub(in crate::app::client) label: u32,
    pub(in crate::app::client) link: u32,
    pub(in crate::app::client) link_lit: u32,
    pub(in crate::app::client) structural_edge: u32,
    pub(in crate::app::client) focused: u32,
    pub(in crate::app::client) rail_text: u32,
    pub(in crate::app::client) rail_active_text: u32,
    pub(in crate::app::client) rail_active_fill: u32,
    pub(in crate::app::client) rail_hover_fill: u32,
    pub(in crate::app::client) rail_bar: u32,
    pub(in crate::app::client) footer_fill: u32,
    pub(in crate::app::client) tile_fill: u32,
    pub(in crate::app::client) tile_selected_fill: u32,
    pub(in crate::app::client) tile_selected_edge: u32,
    pub(in crate::app::client) tile_hover_edge: u32,
    pub(in crate::app::client) tooltip_fill: u32,
    pub(in crate::app::client) tooltip_edge: u32,
    pub(in crate::app::client) tooltip_title: u32,
    pub(in crate::app::client) tooltip_detail: u32,
    /// the light behind a focused rail item and a chosen tile.
    pub(in crate::app::client) focus_wash: u32,
    pub(in crate::app::client) selection_wash: u32,
}

impl SettingsSkin {
    pub(in crate::app::client) const fn for_variant(
        variant: ui_shared_modal::ModalVariant,
    ) -> Self {
        match variant {
            ui_shared_modal::ModalVariant::Sc2 => Self {
                variant,
                interface_font: FONT_INTERFACE,
                heading_font: FONT_INTERFACE,
                nav_font: FONT_NAVIGATION,
                tooltip_font: FONT_INTERNATIONAL,
                text: 0x00d6_e0f0,
                muted: 0x007d_8fa8,
                label: 0x006b_c2f2,
                link: 0x0033_a8f0,
                link_lit: 0x0085_d1ff,
                structural_edge: BORDER_STRUCTURAL,
                focused: BORDER_FOCUSED,
                rail_text: 0x007d_8fa8,
                rail_active_text: 0x00e6_f9ff,
                rail_active_fill: 0x1231_5e8c,
                rail_hover_fill: 0x1231_5e47,
                rail_bar: 0x006b_c2f2,
                footer_fill: 0x0208_0dfc,
                tile_fill: 0x0006_0c11,
                tile_selected_fill: 0x0009_1c2b,
                tile_selected_edge: 0x006b_c2f2,
                tile_hover_edge: BORDER_FOCUSED,
                tooltip_fill: 0x0a18_2af2,
                tooltip_edge: 0x33a8_f080,
                tooltip_title: 0x00d6_e0f0,
                tooltip_detail: 0x0085_d1ff,
                focus_wash: 0x33a8_f04d,
                selection_wash: 0x33a8_f073,
            },
            // the Terran console: red is the chrome, the rust ramp is the ink
            ui_shared_modal::ModalVariant::Remastered => Self {
                variant,
                interface_font: ui_scr_theme::FONT_INTERFACE,
                heading_font: ui_scr_theme::FONT_INTERFACE,
                nav_font: ui_scr_theme::FONT_INTERFACE,
                tooltip_font: ui_scr_theme::FONT_INTERNATIONAL,
                text: 0x00f0_e6da,
                muted: ui_scr_theme::MUTED,
                label: ui_scr_theme::ACCENT,
                link: ui_scr_theme::ACCENT,
                link_lit: ui_scr_theme::TEXT,
                structural_edge: ui_scr_theme::BORDER_STRUCTURAL,
                focused: ui_scr_theme::BORDER_FOCUSED,
                rail_text: ui_scr_theme::MUTED,
                rail_active_text: ui_scr_theme::TEXT,
                rail_active_fill: 0x4a14_0ecc,
                rail_hover_fill: 0x3d12_0e66,
                rail_bar: ui_scr_theme::ACCENT,
                footer_fill: 0x0603_02fc,
                tile_fill: 0x000a_0505,
                tile_selected_fill: 0x002a_0c0a,
                tile_selected_edge: ui_scr_theme::ACCENT,
                tile_hover_edge: ui_scr_theme::BORDER_FOCUSED,
                tooltip_fill: 0x0a05_05f5,
                tooltip_edge: 0xc93a_2c99,
                tooltip_title: 0x00f0_e6da,
                tooltip_detail: 0x00d8_8070,
                focus_wash: 0xff6a_5840,
                selection_wash: 0xff8a_7859,
            },
            // the realm hall: stone and gold, parchment for the words
            ui_shared_modal::ModalVariant::Reforged => Self {
                variant,
                interface_font: ui_wc3_theme::FONT_INTERFACE,
                heading_font: ui_wc3_theme::FONT_TITLE,
                nav_font: ui_wc3_theme::FONT_TITLE,
                tooltip_font: ui_wc3_theme::FONT_INTERFACE,
                text: ui_wc3_theme::PARCHMENT,
                muted: ui_wc3_theme::MUTED,
                label: ui_wc3_theme::GOLD,
                link: ui_wc3_theme::GOLD,
                link_lit: ui_wc3_theme::GOLD_BRIGHT,
                structural_edge: 0x5e4a_26ff,
                focused: ui_wc3_theme::GOLD,
                rail_text: ui_wc3_theme::MUTED,
                rail_active_text: ui_wc3_theme::GOLD_BRIGHT,
                rail_active_fill: 0x2a20_12d9,
                rail_hover_fill: 0xe8c8_7412,
                rail_bar: ui_wc3_theme::GOLD,
                footer_fill: 0x0b08_05fc,
                tile_fill: 0x000b_0805,
                tile_selected_fill: 0x0024_1808,
                tile_selected_edge: ui_wc3_theme::GOLD,
                tile_hover_edge: ui_wc3_theme::GOLD_BRIGHT,
                tooltip_fill: 0x130d_07f7,
                tooltip_edge: 0x8a6d_3bff,
                tooltip_title: ui_wc3_theme::PARCHMENT,
                tooltip_detail: ui_wc3_theme::GOLD_DIM,
                focus_wash: 0xe8c8_7440,
                selection_wash: 0xe8c8_7459,
            },
        }
    }

    /// the glow behind the chosen rail item.
    pub(in crate::app::client) fn focus_glow(&self) -> Vec<gpui::BoxShadow> {
        glow(self.focus_wash, 14.0)
    }

    /// the glow behind the chosen background tile.
    pub(in crate::app::client) fn selection_glow(&self) -> Vec<gpui::BoxShadow> {
        glow(self.selection_wash, 16.0)
    }
}
