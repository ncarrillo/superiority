use super::*;

/// the settings pages. Named rather than numbered because the sidebar and the
/// page dispatch used to be two positional lists that had to agree — dropping
/// one page from a bare array silently renumbered the rest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum SettingsPage {
    Appearance,
    Chat,
    Live,
}

impl SettingsPage {
    /// what the sidebar offers, in order. There is no `Privacy`: the blocked
    /// list is not a setting and is moving to the social pane.
    pub(in crate::app::client) const SHOWN: &'static [Self] =
        &[Self::Appearance, Self::Chat, Self::Live];

    pub(in crate::app::client) fn shown(index: usize) -> Self {
        Self::SHOWN.get(index).copied().unwrap_or(Self::Appearance)
    }

    pub(in crate::app::client) const fn title(self) -> &'static str {
        match self {
            Self::Appearance => "APPEARANCE",
            Self::Chat => "CHAT",
            Self::Live => "LIVE",
        }
    }
}

pub(in crate::app::client) struct SettingsPageTransition {
    pub(in crate::app::client) outgoing: usize,
    pub(in crate::app::client) started: Instant,
}

pub(in crate::app::client) type CheckboxAnimation = ui_animation::ScalarAnimation<Instant>;
