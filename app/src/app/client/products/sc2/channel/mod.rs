use super::*;

mod animation;
mod drag;
mod membership;
mod model;
mod tabs;

pub(in crate::app::client) use model::{
    ChannelState, ChannelTransition, ChannelTransitionSnapshot, TabCloseAnimation, TabDragPayload,
    join_rejection_notice, retitle_notices,
};

pub(in crate::app::client) const TAB_CLOSE_DURATION: Duration = Duration::from_millis(150);
const JOIN_TIMEOUT: Duration = Duration::from_secs(15);

pub(in crate::app::client) struct ChannelComponent {
    pub(in crate::app::client) navigation: ui_workspace::NavigationState<u64, Instant>,
    pub(in crate::app::client) tabs: Vec<ChannelState>,
    pub(in crate::app::client) next_tab_id: u64,
    pub(in crate::app::client) active_tab: usize,
    pub(in crate::app::client) tab_close: Option<TabCloseAnimation>,
    pub(in crate::app::client) hovered_tab: Option<u64>,
    pub(in crate::app::client) tab_selection_started: Option<Instant>,
    pub(in crate::app::client) channel_transition: Option<ChannelTransition>,
    pub(in crate::app::client) chat_entry_reveal: Option<ChatEntryReveal>,
}

impl ChannelComponent {
    pub(in crate::app::client) fn active(&self) -> Option<&ChannelState> {
        self.tabs.get(self.active_tab)
    }

    pub(in crate::app::client) fn transition_progress(&self, now: Instant) -> Option<f32> {
        self.channel_transition
            .as_ref()
            .and_then(|transition| transition.progress(now))
    }
}
