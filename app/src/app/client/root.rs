use super::*;

impl SuperiorityView {
    pub(super) fn overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match self.overlays.active? {
            Overlay::Account => Some(self.overlays.account(
                self.local_account(),
                &self.connection.stage,
                &self.chrome,
                cx,
            )),
            Overlay::Friends => Some(
                self.social
                    .overlay(&self.chrome, &self.overlays, window, cx),
            ),
            Overlay::Settings => {
                let live_url = self.runtime.uplink.stats.feed_url();
                let live_error = self
                    .runtime
                    .uplink
                    .stats
                    .auth_failed
                    .load(std::sync::atomic::Ordering::Relaxed)
                    .then(|| {
                        self.runtime
                            .uplink
                            .stats
                            .last_error()
                            .unwrap_or_else(|| "Live sharing authentication failed.".to_owned())
                    });
                Some(self.settings.overlay(
                    &self.chrome,
                    &self.overlays,
                    &self.social.blocked_accounts,
                    live_url,
                    live_error,
                    window,
                    cx,
                ))
            }
        }
    }
}

impl Render for SuperiorityView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        platform::configure_window(window);
        self.sync_text_inputs(cx);
        self.sync_command_focus(window, cx);
        self.advance_tab_animations(window, cx);

        if self.chrome.modal_assets_warming && !self.chrome.modal_warmup_started {
            self.chrome.modal_warmup_started = true;
            let executor = cx.background_executor().clone();
            cx.spawn_in(window, async move |entity, cx| {
                executor.timer(Duration::from_millis(180)).await;
                entity
                    .update_in(cx, |this, _, cx| {
                        this.chrome.modal_assets_warming = false;
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
        }

        let mut root = ui_workspace::root()
            .id("superiority")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_drag_move::<TabDragPayload>(cx.listener(Self::update_tab_drag))
            .on_drop(cx.listener(Self::finish_tab_drag))
            .relative();
        if self.chrome.modal_assets_warming {
            root = root.child(self.chrome.modal_asset_warmup());
        }
        let online_friends = self
            .social
            .friends
            .iter()
            .filter(|friend| friend.is_online())
            .count();
        let account_portrait = self
            .local_account()
            .and_then(|(user, _)| user.portrait.clone());
        let navigation = self
            .channels
            .view(account_portrait, &self.chrome, window, cx);
        let invitations = self.join.invitation_stack(&self.chrome, cx);
        let affinity = RosterAffinity::new(&self.channels.tabs, &self.social.friends);
        let chat = self.chat.panel(
            &self.channels,
            &self.settings,
            &self.chrome,
            invitations,
            &affinity,
            cx,
        );
        let roster = self.roster.panel(
            &self.channels,
            &self.social.friends,
            self.selected_user(),
            &self.chrome.ui_assets,
            window,
            cx,
        );
        let command_results = self.command_results();
        let composer = self.composer.view(
            window,
            self.channels.active().is_some(),
            online_friends,
            command_results,
            &self.chrome.ui_assets,
            cx,
        );
        let chat_chrome = div().id("chat-chrome").absolute().inset_0().child(
            ui_workspace::ChannelWorkspace::new(navigation, chat, roster)
                .footer(composer)
                .background(self.settings.background),
        );
        let conceal_chat = self.connection.dialog_visible
            || self.updates.startup_connection_pending
            || matches!(
                self.warnings.warning_dialog,
                Some(WarningDialog::Disconnected { .. })
            );
        if conceal_chat {
            root = root.child(chat_chrome.opacity(0.0));
        } else if self.runtime.live_mode {
            root = root.child(chat_chrome.with_animation(
                "chat-chrome-reveal",
                Animation::new(Duration::from_millis(340)),
                |chrome, delta| chrome.opacity(delta),
            ));
        } else {
            root = root.child(chat_chrome);
        }
        if let Some(overlay) = self.overlay(window, cx) {
            root = root.child(overlay);
        }
        if self.runtime.live_mode && self.connection.dialog_visible {
            root = root.child(self.connection.overlay(&self.chrome, cx));
        }
        if self.warnings.warning_dialog.is_some() {
            root = root.child(self.warnings.overlay(&self.chrome, cx));
        }
        if self.updates.update_dialog_visible {
            root = root.child(self.updates.overlay(&self.chrome, window, cx));
        }
        root
    }
}
