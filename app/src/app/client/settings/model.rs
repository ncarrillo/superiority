use super::*;

pub(in crate::app::client) struct SettingsPageTransition {
    pub(in crate::app::client) outgoing: usize,
    pub(in crate::app::client) started: Instant,
}

pub(in crate::app::client) type CheckboxAnimation = ui_animation::ScalarAnimation<Instant>;
