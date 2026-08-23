use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn begin_channel_transition(
        &mut self,
        outgoing: Option<ChannelState>,
        outgoing_selected_user: Option<u32>,
    ) {
        self.session.chat.transcript.selection.clear();
        self.session.channels.channel_transition = Some(ChannelTransition::started(
            ChannelTransitionSnapshot {
                outgoing,
                outgoing_selected_user,
            },
            Instant::now(),
        ));
    }

    pub(in crate::app::client) fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.session.channels.tabs.len()
            || index == self.session.channels.active_tab
            || self.overlays.active.is_some()
        {
            return;
        }
        let outgoing = self.session.channels.active().cloned();
        let outgoing_selected_user = self.selected_user();
        self.session.channels.active_tab = index;
        self.sync_roster_filter_input();
        self.session.channels.tab_selection_started = Some(Instant::now());
        self.session.channels.tabs[index].unread = false;
        self.session.composer.composer_focused = false;
        self.overlays.active = None;
        self.session.channels.chat_entry_reveal = None;
        self.begin_channel_transition(outgoing, outgoing_selected_user);
        Self::trace(format_args!("selected tab {index}"));
        cx.notify();
    }

    pub(in crate::app::client) fn finish_tab_close(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if index >= self.session.channels.tabs.len() {
            return;
        }
        let closing_active = self.session.channels.active_tab == index;
        let outgoing = closing_active.then(|| self.session.channels.tabs[index].clone());
        let outgoing_selected_user = closing_active.then(|| self.selected_user()).flatten();
        let tab = self.session.channels.tabs.remove(index);
        if let (Some(commands), Some(channel_index)) = (&self.session.commands, tab.channel_index) {
            let _ = commands.send(ClientCommand::LeaveChannel { channel_index });
        }
        self.session.channels.navigation.tabs.remove_name(&tab.id);
        self.session.roster.roster.selections.remove(&tab.id);
        if self.session.channels.tabs.is_empty() {
            self.session.channels.active_tab = 0;
        } else if self.session.channels.active_tab > index
            || self.session.channels.active_tab == self.session.channels.tabs.len()
        {
            self.session.channels.active_tab = self
                .session
                .channels
                .active_tab
                .saturating_sub(1)
                .min(self.session.channels.tabs.len() - 1);
        }
        self.sync_roster_filter_input();
        self.session.channels.tab_selection_started = Some(Instant::now());
        self.session.composer.composer_focused = false;
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
        if index >= self.session.channels.tabs.len() || self.session.channels.tab_close.is_some() {
            return;
        }
        self.session.channels.navigation.tabs.cancel_pointer();
        self.session.channels.tab_close = Some(TabCloseAnimation {
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
            self.session.channels.hovered_tab = Some(id);
        } else if self.session.channels.hovered_tab == Some(id) {
            self.session.channels.hovered_tab = None;
        }
        self.session
            .channels
            .navigation
            .tabs
            .set_name_hover(id, hovered, travel, Instant::now());
        cx.notify();
    }
}
