use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn begin_channel_transition(
        &mut self,
        outgoing: Option<ChannelState>,
        outgoing_selected_user: Option<u32>,
    ) {
        self.chat.transcript.selection.clear();
        self.channels.channel_transition = Some(ChannelTransition::started(
            ChannelTransitionSnapshot {
                outgoing,
                outgoing_selected_user,
            },
            Instant::now(),
        ));
    }

    pub(in crate::app::client) fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.channels.tabs.len()
            || index == self.channels.active_tab
            || self.overlays.active.is_some()
        {
            return;
        }
        let outgoing = self.channels.active().cloned();
        let outgoing_selected_user = self.selected_user();
        self.channels.active_tab = index;
        self.sync_roster_filter_input();
        self.channels.tab_selection_started = Some(Instant::now());
        self.channels.tabs[index].unread = false;
        self.composer.composer_focused = false;
        self.overlays.active = None;
        self.channels.chat_entry_reveal = None;
        self.chat.transcript.scroll.scroll_to_bottom();
        self.begin_channel_transition(outgoing, outgoing_selected_user);
        Self::trace(format_args!("selected tab {index}"));
        cx.notify();
    }

    pub(in crate::app::client) fn finish_tab_close(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if index >= self.channels.tabs.len() {
            return;
        }
        let closing_active = self.channels.active_tab == index;
        let outgoing = closing_active.then(|| self.channels.tabs[index].clone());
        let outgoing_selected_user = closing_active.then(|| self.selected_user()).flatten();
        let tab = self.channels.tabs.remove(index);
        if let (Some(commands), Some(channel_index)) = (&self.runtime.commands, tab.channel_index) {
            let _ = commands.send(ClientCommand::LeaveChannel { channel_index });
        }
        self.channels.navigation.tabs.remove_name(&tab.id);
        self.roster.roster.selections.remove(&tab.id);
        if self.channels.tabs.is_empty() {
            self.channels.active_tab = 0;
        } else if self.channels.active_tab > index
            || self.channels.active_tab == self.channels.tabs.len()
        {
            self.channels.active_tab = self
                .channels
                .active_tab
                .saturating_sub(1)
                .min(self.channels.tabs.len() - 1);
        }
        self.sync_roster_filter_input();
        self.channels.tab_selection_started = Some(Instant::now());
        self.composer.composer_focused = false;
        if closing_active {
            self.begin_channel_transition(outgoing, outgoing_selected_user);
        }
        self.persist_open_channels();
        Self::trace(format_args!("closed tab {index}"));
        cx.notify();
    }

    pub(in crate::app::client) fn begin_tab_close(
        &mut self,
        index: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.channels.tabs.len() || self.channels.tab_close.is_some() {
            return;
        }
        self.channels.navigation.tabs.cancel_pointer();
        self.channels.tab_close = Some(TabCloseAnimation {
            index,
            started: None,
        });
        cx.notify();
    }

    pub(in crate::app::client) fn set_tab_name_hover(
        &mut self,
        id: u64,
        hovered: bool,
        travel: f32,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if hovered {
            self.channels.hovered_tab = Some(id);
        } else if self.channels.hovered_tab == Some(id) {
            self.channels.hovered_tab = None;
        }
        self.channels
            .navigation
            .tabs
            .set_name_hover(id, hovered, travel, Instant::now());
        cx.notify();
    }
}
