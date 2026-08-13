use gpui::ImageSource;

use crate::PresenceKind;

#[derive(Clone)]
pub struct UiAssets {
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

impl UiAssets {
    #[must_use]
    pub fn native() -> Self {
        Self {
            top_navigation_background: "images/curated/controls/top-nav-background.png".into(),
            top_navigation_divider: "images/curated/controls/top-nav-divider.png".into(),
            top_navigation_selected: "images/curated/controls/top-nav-selected.png".into(),
            top_navigation_selected_line: "images/curated/controls/top-nav-selected-line.png"
                .into(),
            top_navigation_selected_glow: "images/curated/controls/top-nav-selected-line-glow.png"
                .into(),
            top_navigation_selected_orange: "images/curated/controls/top-nav-selected-orange.png"
                .into(),
            top_navigation_selected_line_orange:
                "images/curated/controls/top-nav-selected-line-orange.png".into(),
            top_navigation_selected_glow_orange:
                "images/curated/controls/top-nav-selected-line-glow-orange.png".into(),
            top_navigation_selected_pink: "images/curated/controls/top-nav-selected-pink.png"
                .into(),
            top_navigation_selected_line_pink:
                "images/curated/controls/top-nav-selected-line-pink.png".into(),
            top_navigation_selected_glow_pink:
                "images/curated/controls/top-nav-selected-line-glow-pink.png".into(),
            portrait_frame: "images/nine-patch/portraits/frame.png".into(),
            portrait_placeholder: "images/icons/friend-placeholder.png".into(),
            tooltip_fill: "images/settings/tooltip-fill.png".into(),
            button_idle: "images/nine-patch/controls/button-idle.png".into(),
            button_active: "images/nine-patch/controls/button-active.png".into(),
            warning_button_idle: "images/nine-patch/controls/warning-button-idle.png".into(),
            warning_button_active: "images/nine-patch/controls/warning-button-active.png".into(),
            checkbox_mark: "images/settings/checkbox-mark.png".into(),
            modal_title_band: "images/dialogs/modal-title-band.png".into(),
            modal_hex: "images/dialogs/modal-hex.png".into(),
            modal_glow_left: "images/dialogs/modal-glow-left.png".into(),
            modal_glow_right: "images/dialogs/modal-glow-right.png".into(),
            modal_glow_top: "images/dialogs/modal-glow-top.png".into(),
            modal_glow_bottom: "images/dialogs/modal-glow-bottom.png".into(),
            status_available: "images/icons/status-available.png".into(),
            status_away: "images/icons/status-away.png".into(),
            status_busy: "images/icons/status-busy.png".into(),
            status_in_game: "images/icons/status-in-game.png".into(),
            status_offline: "images/icons/status-offline.png".into(),
        }
    }

    #[must_use]
    pub fn web(root: &str) -> Self {
        let asset = |path: &str| {
            format!(
                "{}/{}",
                root.trim_end_matches('/'),
                path.trim_start_matches('/')
            )
            .into()
        };
        Self {
            top_navigation_background: asset("ui/top-nav-background.png"),
            top_navigation_divider: asset("ui/top-nav-divider.png"),
            top_navigation_selected: asset("ui/top-nav-selected.png"),
            top_navigation_selected_line: asset("ui/top-nav-selected-line.png"),
            top_navigation_selected_glow: asset("ui/top-nav-selected-line-glow.png"),
            top_navigation_selected_orange: asset("ui/top-nav-selected-orange.png"),
            top_navigation_selected_line_orange: asset("ui/top-nav-selected-line-orange.png"),
            top_navigation_selected_glow_orange: asset("ui/top-nav-selected-line-glow-orange.png"),
            top_navigation_selected_pink: asset("ui/top-nav-selected-pink.png"),
            top_navigation_selected_line_pink: asset("ui/top-nav-selected-line-pink.png"),
            top_navigation_selected_glow_pink: asset("ui/top-nav-selected-line-glow-pink.png"),
            portrait_frame: asset("ui/portrait-frame.png"),
            portrait_placeholder: asset("ui/friend-placeholder.png"),
            tooltip_fill: asset("ui/tooltip-fill.png"),
            button_idle: asset("ui/button-idle.png"),
            button_active: asset("ui/button-active.png"),
            warning_button_idle: asset("ui/warning-button-idle.png"),
            warning_button_active: asset("ui/warning-button-active.png"),
            checkbox_mark: asset("ui/checkbox-mark.png"),
            modal_title_band: asset("ui/modal-title-band.png"),
            modal_hex: asset("ui/modal-hex.png"),
            modal_glow_left: asset("ui/modal-glow-left.png"),
            modal_glow_right: asset("ui/modal-glow-right.png"),
            modal_glow_top: asset("ui/modal-glow-top.png"),
            modal_glow_bottom: asset("ui/modal-glow-bottom.png"),
            status_available: asset("ui/status-available.png"),
            status_away: asset("ui/status-away.png"),
            status_busy: asset("ui/status-busy.png"),
            status_in_game: asset("ui/status-in-game.png"),
            status_offline: asset("ui/status-offline.png"),
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
