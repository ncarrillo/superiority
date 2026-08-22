use gpui::ImageSource;

use crate::foundation::assets::{AssetPaths, AssetResolver};

use super::PresenceKind;

#[derive(Clone)]
pub struct Sc2Assets {
    pub top_navigation_background: ImageSource,
    pub top_navigation_divider: ImageSource,
    pub top_navigation_selected: ImageSource,
    pub top_navigation_selected_line: ImageSource,
    pub top_navigation_selected_glow: ImageSource,
    pub top_navigation_selected_orange: ImageSource,
    pub top_navigation_selected_line_orange: ImageSource,
    pub top_navigation_selected_glow_orange: ImageSource,
    pub top_navigation_selected_pink: ImageSource,
    pub top_navigation_selected_line_pink: ImageSource,
    pub top_navigation_selected_glow_pink: ImageSource,
    pub portrait_frame: ImageSource,
    pub portrait_placeholder: ImageSource,
    pub tooltip_fill: ImageSource,
    pub button_idle: ImageSource,
    pub button_active: ImageSource,
    pub warning_button_idle: ImageSource,
    pub warning_button_active: ImageSource,
    pub checkbox_mark: ImageSource,
    pub modal_title_band: ImageSource,
    pub modal_hex: ImageSource,
    pub modal_glow_left: ImageSource,
    pub modal_glow_right: ImageSource,
    pub modal_glow_top: ImageSource,
    pub modal_glow_bottom: ImageSource,
    pub status_available: ImageSource,
    pub status_away: ImageSource,
    pub status_busy: ImageSource,
    pub status_in_game: ImageSource,
    pub status_offline: ImageSource,
}

impl Sc2Assets {
    #[must_use]
    pub fn load(resolver: &impl AssetResolver) -> Self {
        let image = |native, web| resolver.image(AssetPaths::new(native, web));
        Self {
            top_navigation_background: image(
                "images/curated/controls/top-nav-background.png",
                "ui/top-nav-background.png",
            ),
            top_navigation_divider: image(
                "images/curated/controls/top-nav-divider.png",
                "ui/top-nav-divider.png",
            ),
            top_navigation_selected: image(
                "images/curated/controls/top-nav-selected.png",
                "ui/top-nav-selected.png",
            ),
            top_navigation_selected_line: image(
                "images/curated/controls/top-nav-selected-line.png",
                "ui/top-nav-selected-line.png",
            ),
            top_navigation_selected_glow: image(
                "images/curated/controls/top-nav-selected-line-glow.png",
                "ui/top-nav-selected-line-glow.png",
            ),
            top_navigation_selected_orange: image(
                "images/curated/controls/top-nav-selected-orange.png",
                "ui/top-nav-selected-orange.png",
            ),
            top_navigation_selected_line_orange: image(
                "images/curated/controls/top-nav-selected-line-orange.png",
                "ui/top-nav-selected-line-orange.png",
            ),
            top_navigation_selected_glow_orange: image(
                "images/curated/controls/top-nav-selected-line-glow-orange.png",
                "ui/top-nav-selected-line-glow-orange.png",
            ),
            top_navigation_selected_pink: image(
                "images/curated/controls/top-nav-selected-pink.png",
                "ui/top-nav-selected-pink.png",
            ),
            top_navigation_selected_line_pink: image(
                "images/curated/controls/top-nav-selected-line-pink.png",
                "ui/top-nav-selected-line-pink.png",
            ),
            top_navigation_selected_glow_pink: image(
                "images/curated/controls/top-nav-selected-line-glow-pink.png",
                "ui/top-nav-selected-line-glow-pink.png",
            ),
            portrait_frame: image(
                "images/nine-patch/portraits/frame.png",
                "ui/portrait-frame.png",
            ),
            portrait_placeholder: image(
                "images/icons/friend-placeholder.png",
                "ui/friend-placeholder.png",
            ),
            tooltip_fill: image("images/settings/tooltip-fill.png", "ui/tooltip-fill.png"),
            button_idle: image(
                "images/nine-patch/controls/button-idle.png",
                "ui/button-idle.png",
            ),
            button_active: image(
                "images/nine-patch/controls/button-active.png",
                "ui/button-active.png",
            ),
            warning_button_idle: image(
                "images/nine-patch/controls/warning-button-idle.png",
                "ui/warning-button-idle.png",
            ),
            warning_button_active: image(
                "images/nine-patch/controls/warning-button-active.png",
                "ui/warning-button-active.png",
            ),
            checkbox_mark: image("images/settings/checkbox-mark.png", "ui/checkbox-mark.png"),
            modal_title_band: image(
                "images/dialogs/modal-title-band.png",
                "ui/modal-title-band.png",
            ),
            modal_hex: image("images/dialogs/modal-hex.png", "ui/modal-hex.png"),
            modal_glow_left: image(
                "images/dialogs/modal-glow-left.png",
                "ui/modal-glow-left.png",
            ),
            modal_glow_right: image(
                "images/dialogs/modal-glow-right.png",
                "ui/modal-glow-right.png",
            ),
            modal_glow_top: image("images/dialogs/modal-glow-top.png", "ui/modal-glow-top.png"),
            modal_glow_bottom: image(
                "images/dialogs/modal-glow-bottom.png",
                "ui/modal-glow-bottom.png",
            ),
            status_available: image(
                "images/icons/status-available.png",
                "ui/status-available.png",
            ),
            status_away: image("images/icons/status-away.png", "ui/status-away.png"),
            status_busy: image("images/icons/status-busy.png", "ui/status-busy.png"),
            status_in_game: image("images/icons/status-in-game.png", "ui/status-in-game.png"),
            status_offline: image("images/icons/status-offline.png", "ui/status-offline.png"),
        }
    }

    #[must_use]
    pub fn presence_icon(&self, presence: PresenceKind) -> ImageSource {
        match presence {
            PresenceKind::Available => self.status_available.clone(),
            PresenceKind::Away => self.status_away.clone(),
            PresenceKind::Busy => self.status_busy.clone(),
            PresenceKind::InGame => self.status_in_game.clone(),
            PresenceKind::Offline | PresenceKind::Unknown => self.status_offline.clone(),
        }
    }
}
