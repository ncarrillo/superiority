use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn begin_startup_connection(&mut self, cx: &mut Context<Self>) {
        if !std::mem::replace(&mut self.updates.startup_connection_pending, false) {
            return;
        }
        Self::trace("starting deferred Battle.net connection");
        self.reconnect(cx);
    }

    pub(in crate::app::client) fn cancel_connection(&mut self, cx: &mut Context<Self>) {
        if self.connection.error.is_some() || self.connection.stage == ConnectionStage::Disconnected
        {
            self.reconnect(cx);
            return;
        }
        if let Some(authenticator) = self.runtime.authenticator.take() {
            authenticator.dismiss();
        }
        if let Some(commands) = &self.runtime.commands {
            let _ = commands.send(ClientCommand::Disconnect);
        }
        self.connection.starting = false;
        self.connection.error = None;
        self.connection.stage = ConnectionStage::Disconnected;
        self.set_connection_progress_stage(ConnectionStage::Disconnected);
        self.open_connection_dialog();
        cx.notify();
    }

    pub(in crate::app::client) fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.connection.sign_out_requested = true;
        self.connection.signed_out = true;
        self.connection.starting = false;
        self.connection.error = None;
        if let Some(authenticator) = self.runtime.authenticator.take() {
            authenticator.dismiss();
        }
        if let Some(commands) = &self.runtime.commands {
            let _ = commands.send(ClientCommand::SignOut);
        }
        self.dismiss_overlay(window, cx);
        let mut channels = self
            .channels
            .tabs
            .iter()
            .filter_map(|tab| {
                tab.channel
                    .clone()
                    .map(|channel| (tab.id, tab.title.clone(), channel))
            })
            .collect::<Vec<_>>();
        if channels.is_empty() {
            channels.push((
                self.channels.next_tab_id,
                channel_title(&ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)),
                ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL),
            ));
            self.channels.next_tab_id += 1;
        }
        self.channels.tabs = channels
            .into_iter()
            .map(|(id, title, channel)| {
                let mut tab = ChannelState::pending_live(id, channel);
                tab.title = title;
                tab
            })
            .collect();
        self.channels.active_tab = 0;
        self.channels.navigation.tabs.clear();
        self.channels.tab_close = None;
        self.channels.channel_transition = None;
        self.channels.chat_entry_reveal = None;
        self.roster.roster.selections.clear();
        self.roster.pending_rosters.clear();
        self.roster.roster_batch_started = None;
        self.roster.roster_flush_at = None;
        self.roster.roster_debounce_window = ROSTER_DEBOUNCE_BASE;
        self.roster.roster.animation = None;
        self.roster.roster_input.clear();
        self.roster.roster.focused = false;
        self.composer.composer.clear();
        self.composer.composer_focused = false;
        self.social.friends_snapshot.clear();
        self.social.friends.clear();
        self.social.blocked_accounts.clear();
        self.join.awaiting_joins.clear();
        self.social.conversations.clear();
        self.social.whisper_unread.clear();
        self.social.conversation_peer = None;
        self.social.conversation_input.clear();
        self.connection.stage = ConnectionStage::Disconnected;
        self.set_connection_progress_stage(ConnectionStage::Disconnected);
        self.connection.dialog_visible = true;
        self.connection.dialog_closing = false;
        self.connection.close_due = None;
        self.connection.hide_due = None;
        cx.notify();
    }

    pub(in crate::app::client) fn quit_after_disconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(commands) = &self.runtime.commands {
            let _ = commands.send(ClientCommand::Quit);
        }
        cx.quit();
    }
}
