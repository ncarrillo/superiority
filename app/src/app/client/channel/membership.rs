use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn join_channel(&mut self, title: String, cx: &mut Context<Self>) {
        let target = JoinComponent::typed_target(title.trim());
        self.join_channel_target(target, cx);
    }

    pub(in crate::app::client) fn channel_label(&self, channel: &ChatChannel) -> String {
        if let ChatChannel::Club(club_id) = channel {
            if let Some(group) = self.join.groups.get(club_id) {
                return group.name.clone();
            }
            if let Some(name) = self.join.remembered_group_names.get(club_id) {
                return name.clone();
            }
        }
        channel_title(channel)
    }

    pub(in crate::app::client) fn persist_open_channels(&self) {
        if !self.runtime.live_mode {
            return;
        }
        preferences::save_open_channels(
            &self
                .channels
                .tabs
                .iter()
                .filter_map(|tab| tab.channel.clone())
                .collect::<Vec<_>>(),
        );
    }

    pub(in crate::app::client) fn reject_pending_join(
        &mut self,
        channel: Option<&ChatChannel>,
        reason: Option<u16>,
    ) {
        if let Some(channel) = channel {
            self.join
                .awaiting_joins
                .retain(|(awaited, _)| awaited != channel);
        }
        let name = channel.map_or_else(
            || "that channel".to_owned(),
            |channel| self.channel_label(channel),
        );
        let pending = self
            .channels
            .tabs
            .iter()
            .position(|tab| tab.channel.as_ref() == channel && tab.channel_index.is_none());
        self.show_channel_warning("CHANNEL", join_rejection_notice(&name, reason), None);
        if let Some(position) = pending {
            self.channels.tabs.remove(position);
            if self.channels.tabs.is_empty() {
                let id = self.channels.next_tab_id;
                self.channels.next_tab_id = self.channels.next_tab_id.wrapping_add(1);
                self.channels.tabs.push(ChannelState::pending_live(
                    id,
                    ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL),
                ));
            }
            if self.channels.active_tab >= position && self.channels.active_tab > 0 {
                self.channels.active_tab -= 1;
            }
            self.channels.active_tab = self
                .channels
                .active_tab
                .min(self.channels.tabs.len().saturating_sub(1));
            self.persist_open_channels();
        }
    }

    pub(in crate::app::client) fn expire_awaited_joins(&mut self) -> bool {
        let now = Instant::now();
        let expired = self
            .join
            .awaiting_joins
            .iter()
            .filter(|(_, started)| now.saturating_duration_since(*started) >= JOIN_TIMEOUT)
            .map(|(channel, _)| channel.clone())
            .collect::<Vec<_>>();
        for channel in &expired {
            self.reject_pending_join(Some(channel), None);
        }
        !expired.is_empty()
    }

    pub(in crate::app::client) fn retitle_club_tabs(&mut self, club_id: u32, name: &str) {
        for tab in self
            .channels
            .tabs
            .iter_mut()
            .filter(|tab| tab.channel.as_ref() == Some(&ChatChannel::Club(club_id)))
        {
            if tab.title == name {
                continue;
            }
            retitle_notices(&mut tab.transcript, &tab.title, name);
            tab.title = name.to_owned();
        }
    }

    pub(in crate::app::client) fn join_channel_target(
        &mut self,
        target: ChatChannel,
        cx: &mut Context<Self>,
    ) {
        let title = self.channel_label(&target);
        if self.runtime.live_mode {
            if let Some(index) = self
                .channels
                .tabs
                .iter()
                .position(|tab| tab.channel.as_ref() == Some(&target))
            {
                self.channels.active_tab = index;
            } else {
                if self.channels.tabs.len() >= MAX_JOINED_CHANNELS {
                    self.show_channel_warning(
                        "CHANNEL",
                        format!(
                            "You can be in {MAX_JOINED_CHANNELS} channels at once. Close one to join another."
                        ),
                        None,
                    );
                    cx.notify();
                    return;
                }
                let id = self.channels.next_tab_id;
                self.channels.next_tab_id = self.channels.next_tab_id.wrapping_add(1);
                let mut tab = ChannelState::pending_live(id, target.clone());
                tab.title.clone_from(&title);
                self.channels.tabs.push(tab);
                self.channels.active_tab = self.channels.tabs.len() - 1;
                self.join
                    .awaiting_joins
                    .push((target.clone(), Instant::now()));
                self.persist_open_channels();
                if let Some(commands) = &self.runtime.commands {
                    if commands
                        .send(ClientCommand::JoinChannel(target.clone()))
                        .is_err()
                    {
                        self.reject_pending_join(Some(&target), None);
                    }
                }
            }
            self.overlays.active = None;
            self.overlays.closing = false;
            self.channels.tab_selection_started = Some(Instant::now());
            cx.notify();
            return;
        }
        let previous = self.channels.active().map(|channel| channel.id);
        let outgoing = self.channels.active().cloned();
        let outgoing_selected_user = self.selected_user();
        if let Some(index) = self.channels.tabs.iter().position(|tab| tab.title == title) {
            self.channels.active_tab = index;
        } else {
            let id = self.channels.next_tab_id;
            self.channels.next_tab_id = self.channels.next_tab_id.wrapping_add(1);
            self.channels
                .tabs
                .push(ChannelState::fixture_joined(id, title));
            self.channels.active_tab = self.channels.tabs.len() - 1;
        }
        self.channels.tab_selection_started = Some(Instant::now());
        if self.channels.active().map(|channel| channel.id) != previous {
            self.begin_channel_transition(outgoing, outgoing_selected_user);
        }
        self.overlays.active = None;
        self.overlays.closing = false;
        Self::trace(format_args!("joined tab {}", self.channels.active_tab));
        cx.notify();
    }
}
