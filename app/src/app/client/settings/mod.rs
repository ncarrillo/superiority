use super::*;

mod model;
mod pages;
mod tooltip;
mod view;

pub(in crate::app::client) use model::{CheckboxAnimation, SettingsPageTransition};

pub(in crate::app::client) const SETTINGS_PAGE_CROSSFADE_DURATION: Duration =
    Duration::from_millis(240);
const SETTINGS_TOOLTIP_TIMESTAMPS: usize = 2;
const SETTINGS_TOOLTIP_MEMBERSHIP: usize = 3;
const SETTINGS_TOOLTIP_LIVE_ENABLED: usize = 4;
const SETTINGS_TOOLTIP_BACKGROUND_START: usize = 10;
const CHAT_SETTING_TIMESTAMPS: usize = 0;
const CHAT_SETTING_MEMBERSHIP: usize = 1;
const LIVE_SETTING_ENABLED: usize = 2;
const CHECKBOX_REVEAL_DURATION: Duration = Duration::from_millis(170);
const CHECKBOX_HIDE_DURATION: Duration = Duration::from_millis(120);

pub(super) struct SettingsComponent {
    pub(super) show_timestamps: bool,
    pub(super) show_membership: bool,
    pub(super) live_enabled: bool,
    pub(super) background: &'static str,
    pub(super) active_settings_page: usize,
    pub(super) settings_page_transition: Option<SettingsPageTransition>,
    pub(super) settings_tooltip: Option<usize>,
    pub(super) checkbox_animations: [Option<CheckboxAnimation>; 3],
    pub(super) privacy_scroll: ScrollHandle,
}

impl SettingsComponent {
    pub(super) fn begin_checkbox_animation(
        &mut self,
        index: usize,
        from_checked: bool,
        to_checked: bool,
    ) {
        let now = Instant::now();
        let from = self
            .checkbox_animations
            .get(index)
            .and_then(Option::as_ref)
            .map_or(if from_checked { 1.0 } else { 0.0 }, |animation| {
                animation.value(now)
            });
        let to = if to_checked { 1.0 } else { 0.0 };
        let base_duration = if to_checked {
            CHECKBOX_REVEAL_DURATION
        } else {
            CHECKBOX_HIDE_DURATION
        };
        let distance = (to - from).abs().max(0.35);
        self.checkbox_animations[index] = Some(CheckboxAnimation {
            from,
            to,
            started: now,
            duration: base_duration.mul_f32(distance),
        });
    }

    pub(super) fn push_live_config(&self, uplink: &uplink::UplinkControl) {
        let enabled = self.live_enabled;
        uplink.update_config(move |config| {
            config.enabled = enabled;
        });
    }

    pub(super) fn toggle_live(&mut self, uplink: &uplink::UplinkControl) {
        let enabled = !self.live_enabled;
        self.begin_checkbox_animation(LIVE_SETTING_ENABLED, self.live_enabled, enabled);
        self.live_enabled = enabled;
        preferences::save_live_enabled(self.live_enabled);
        self.push_live_config(uplink);
    }
}
