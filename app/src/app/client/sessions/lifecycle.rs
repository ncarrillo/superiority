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
        if self.session.connection.error.is_some()
            || self.session.connection.stage == ConnectionStage::Disconnected
        {
            self.reconnect(cx);
            return;
        }
        if let Some(authenticator) = self.runtime.authenticator.take() {
            authenticator.dismiss();
        }
        if let Some(commands) = &self.session.commands {
            let _ = commands.send(ClientCommand::Disconnect);
        }
        self.session.connection.starting = false;
        self.session.connection.error = None;
        self.session.connection.stage = ConnectionStage::Disconnected;
        self.set_connection_progress_stage(ConnectionStage::Disconnected);
        self.open_connection_dialog();
        cx.notify();
    }

    /// Goes back out to the games list.
    ///
    /// Not a sign-out: every product stays connected, and this session is still
    /// running when you come back to it. It is navigation, so it animates the
    /// way arriving does rather than cutting.
    pub(in crate::app::client) fn show_games_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_overlay(window, cx);
        self.games.return_to_list(Instant::now());
        cx.notify();
    }

    /// Refreshes the authoritative product catalogue and every provisioned
    /// product connection without changing accounts. Workers receive
    /// `Disconnect` followed by a serialized reconnect: the account guard is
    /// retained, cached product tickets are reused, and the SC2 account service
    /// is queried again before the secondary products resume.
    pub(in crate::app::client) fn refresh_games(&mut self, cx: &mut Context<Self>) {
        if !self.runtime.live_mode {
            // nothing to poll: the chip still shows the round trip it made
            self.games.begin_refresh(0, Instant::now());
            self.sync_live_games();
            cx.notify();
            return;
        }
        if let Some(authenticator) = self.runtime.authenticator.take() {
            authenticator.dismiss();
        }

        let expected_account_id = self.runtime.authoritative_account_id;
        let expected_battle_tag = self.runtime.authoritative_battle_tag.clone();
        let mut products = Product::ALL
            .into_iter()
            .filter(|product| {
                *product == Product::StarCraft2 || self.runtime.provisioned.contains(product)
            })
            .collect::<Vec<_>>();
        products.retain(|product| self.product_session(*product).is_some());
        // the bar counts these back in as each one settles
        self.games.begin_refresh(products.len(), Instant::now());

        // Let every live loop close cleanly. A subsequent Connect stays queued
        // behind Disconnect in that worker's channel, so no second worker and
        // no overlapping browser authorization is created.
        for product in &products {
            if let Some(commands) = self
                .product_session(*product)
                .and_then(|session| session.commands.as_ref())
            {
                let _ = commands.send(ClientCommand::Disconnect);
            }
        }
        for product in &products {
            if let Some(session) = self.product_session_mut(*product) {
                session.account_id = None;
                session.account_battle_tag = None;
                session.account_region = None;
                session.connection.starting = false;
                session.connection.error = None;
                session.connection.stage = ConnectionStage::Disconnected;
                session.connection.fill = 0.0;
                session.connection.floor = 0.0;
                session.connection.ceiling = 0.0;
                session.connection.progress_updated = Instant::now();
            }
        }

        self.runtime.connect_queue.clear();
        self.runtime.connecting = None;
        self.games.live.clear();

        let Some(sc2) = self.product_session(Product::StarCraft2) else {
            cx.notify();
            return;
        };
        let Some(commands) = sc2.commands.clone() else {
            cx.notify();
            return;
        };
        let mut channels = sc2
            .channels
            .tabs
            .iter()
            .filter_map(|tab| tab.channel.clone())
            .collect::<Vec<_>>();
        if channels.is_empty() {
            channels.push(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL));
        }
        let _ = commands.send(ClientCommand::Connect {
            force_interactive: false,
            expected_account_id,
            expected_battle_tag,
            channels,
        });
        if let Some(sc2) = self.product_session_mut(Product::StarCraft2) {
            sc2.connection.starting = true;
        }
        self.runtime.connecting = Some(Product::StarCraft2);
        self.runtime.connect_queue.extend(
            products
                .into_iter()
                .filter(|product| *product != Product::StarCraft2),
        );
        self.sync_live_games();
        cx.notify();
    }

    /// Signs the authoritative Battle.net account out everywhere. Product
    /// credentials are distinct tokens, so every worker must delete its own;
    /// the next SC2 sign-in then establishes a new authority and catalogue.
    pub(in crate::app::client) fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(authenticator) = self.runtime.authenticator.take() {
            authenticator.dismiss();
        }
        for session in std::iter::once(&self.session).chain(self.suspended.values()) {
            if let Some(commands) = &session.commands {
                let _ = commands.send(ClientCommand::SignOut);
            }
        }
        for session in std::iter::once(&mut self.session).chain(self.suspended.values_mut()) {
            session.account_id = None;
            session.account_battle_tag = None;
            session.account_region = None;
            session.connection.sign_out_requested = true;
            session.connection.signed_out = true;
            session.connection.starting = false;
            session.connection.error = None;
            session.connection.stage = ConnectionStage::Disconnected;
        }
        self.runtime.authoritative_account_id = None;
        self.runtime.authoritative_battle_tag = None;
        self.runtime.authoritative_region = None;
        self.runtime.provisioned.clear();
        self.runtime.connect_queue.clear();
        self.runtime.connecting = None;
        self.games.owned = None;
        self.games.live.clear();
        self.dismiss_overlay(window, cx);
        // Account selection always restarts at the authority session. With the
        // licence set cleared there is deliberately no product card to enter.
        let _ = self.focus_product(Product::StarCraft2);
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.clear();
            self.session.connection.stage = ConnectionStage::Disconnected;
            self.set_connection_progress_stage(ConnectionStage::Disconnected);
            self.session.connection.dialog_visible = true;
            self.session.connection.dialog_closing = false;
            self.session.connection.close_due = None;
            self.session.connection.hide_due = None;
            cx.notify();
            return;
        }
        if let Some(scr) = self.session.scr_mut() {
            scr.clear();
            self.session.connection.stage = ConnectionStage::Disconnected;
            self.set_connection_progress_stage(ConnectionStage::Disconnected);
            self.session.connection.dialog_visible = true;
            self.session.connection.dialog_closing = false;
            self.session.connection.close_due = None;
            self.session.connection.hide_due = None;
            cx.notify();
            return;
        }
        let mut channels = self
            .session
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
                self.session.channels.next_tab_id,
                channel_title(&ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL)),
                ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL),
            ));
            self.session.channels.next_tab_id += 1;
        }
        self.session.channels.tabs = channels
            .into_iter()
            .map(|(id, title, channel)| {
                let mut tab = ChannelState::pending_live(id, channel);
                tab.title = title;
                tab
            })
            .collect();
        self.session.channels.active_tab = 0;
        self.session.channels.navigation.tabs.clear();
        self.session.channels.tab_close = None;
        self.session.channels.channel_transition = None;
        self.session.channels.chat_entry_reveal = None;
        self.session.roster.roster.selections.clear();
        self.session.roster.pending_rosters.clear();
        self.session.roster.roster_batch_started = None;
        self.session.roster.roster_flush_at = None;
        self.session.roster.roster_debounce_window = ROSTER_DEBOUNCE_BASE;
        self.session.roster.roster.animation = None;
        self.session.roster.roster_input.clear();
        self.session.roster.roster.focused = false;
        self.session.composer.composer.clear();
        self.session.composer.composer_focused = false;
        self.session.social.clear();
        self.session.join.awaiting_joins.clear();
        self.session.connection.stage = ConnectionStage::Disconnected;
        self.set_connection_progress_stage(ConnectionStage::Disconnected);
        self.session.connection.dialog_visible = true;
        self.session.connection.dialog_closing = false;
        self.session.connection.close_due = None;
        self.session.connection.hide_due = None;
        // Sign Out is account switching, not a dead end. Queue a clean SC2
        // authorization behind the worker's SignOut command; its new account
        // response will atomically repopulate the picker and queue only the
        // products provisioned to that identity.
        self.reconnect(cx);
    }

    pub(in crate::app::client) fn quit_after_disconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(commands) = &self.session.commands {
            let _ = commands.send(ClientCommand::Quit);
        }
        cx.quit();
    }
}
