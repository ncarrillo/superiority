use super::*;

impl SuperiorityView {
    pub(super) fn overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match self.overlays.active? {
            Overlay::Account if self.focused == Product::Remastered => {
                Some(self.overlays.scr_account(self.scr_account_identity(), cx))
            }
            // Reforged's identity plaque is the menu; the popout behind it
            // carries what the header buttons used to
            Overlay::Account if self.focused == Product::Warcraft3 => {
                Some(self.wc3_account_popout(cx))
            }
            Overlay::Account => Some(self.overlays.account(
                self.local_account(),
                &self.session.connection.stage,
                cx,
            )),
            Overlay::Clan if self.focused == Product::Warcraft3 => {
                Some(self.wc3_clan_overlay(window, cx))
            }
            Overlay::Clan => None,
            Overlay::Friends => Some(self.session.social.overlay(
                overlays::modal_variant(self.focused),
                &self.chrome,
                &self.overlays,
                window,
                cx,
            )),
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
                    self.focused,
                    overlays::modal_variant(self.focused),
                    &self.chrome,
                    &self.overlays,
                    live_url,
                    live_error,
                    window,
                    cx,
                ))
            }
        }
    }
}

impl SuperiorityView {
    /// a realm's own window, arriving the way StarCraft II's chrome does: a
    /// short reveal when it first comes on screen, in live mode. Remastered
    /// and Reforged used to simply appear where the picker had been.
    fn realm_window_reveal(
        &self,
        seat_id: &'static str,
        reveal_id: &'static str,
        window: AnyElement,
    ) -> AnyElement {
        let seat = div().id(seat_id).absolute().inset_0().child(window);
        if self.runtime.live_mode {
            seat.with_animation(
                reveal_id,
                Animation::new(Duration::from_millis(340)),
                gpui::Styled::opacity,
            )
            .into_any_element()
        } else {
            seat.into_any_element()
        }
    }

    /// the picker, with the state walk driven by the frame clock. It has no
    /// session to poll, so the walk is wound on here rather than by the client
    /// loop that does not run in this mode.
    ///
    /// `None` on the frame the picker hands over: the walk is wound on before
    /// anything is drawn, and a handoff means the realm is drawn this frame —
    /// not a settled picker for one frame and the realm the next, which read
    /// as a flash of the list between the flood and the chat.
    fn games_view(
        &mut self,
        screen: GamesScreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let now = Instant::now();
        match screen {
            GamesScreen::States => {
                self.games.advance_if_due();
            }
            GamesScreen::Picker => {
                // only the design screens land a card on a timer. A live card's
                // state is its session's, and letting the clock declare it
                // connected mid-handshake took the progress rail away — the
                // rail is drawn only while the card is connecting — and gave it
                // back at the next step.
                if !self.games.live_mode {
                    self.games.land_connections(now);
                }
                self.games.advance_motion(now);
                if self.games.has_entered() {
                    return None;
                }
                // the top bar's clocks: a refresh lands when nothing is left in
                // flight, an arm lapses, a severed plaque lets go; while any
                // of them runs the bar is read again next frame
                let in_flight = self.realms_in_flight();
                let signed_in = self.runtime.authoritative_battle_tag.is_some();
                let reconnecting = self.session.connection.starting || in_flight > 0;
                self.games
                    .settle_titlebar(now, in_flight, signed_in, reconnecting);
                if self.games.titlebar_live() {
                    window.request_animation_frame();
                }
            }
        }
        let sheet = match screen {
            GamesScreen::States => self.games_screen(window, cx),
            GamesScreen::Picker => self.picker_screen(window, cx),
        };
        let view = ui_workspace::root()
            .id("superiority-games")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                match screen {
                    // the walk is driven by any key, so the states can be
                    // stepped without finding the pointer
                    GamesScreen::States => {
                        if !matches!(event.keystroke.key.as_str(), "escape") {
                            this.games.step();
                        }
                    }
                    GamesScreen::Picker => match event.keystroke.key.as_str() {
                        // backing out of a realm is the one thing that answers
                        // while the picker is busy becoming one
                        "escape" if this.games.snap_back(now) => {}
                        _ if !this.games.answering() => return,
                        "left" => this.games.move_choice(-1),
                        "right" => this.games.move_choice(1),
                        // the digits jump straight in, per the key legend
                        // under the row
                        "1" | "2" | "3" => {
                            let jumped = event
                                .keystroke
                                .key
                                .parse::<usize>()
                                .ok()
                                .and_then(|digit| digit.checked_sub(1));
                            if let Some(index) = jumped {
                                this.take_game_card(index, now, cx);
                            }
                        }
                        "enter" | "return" | "space" => {
                            let chosen = this.games.picker.selected;
                            this.take_game_card(chosen, now, cx);
                        }
                        "escape" => {
                            let chosen = this.games.picker.selected;
                            this.games.set_card(chosen, this.games.resting(chosen));
                        }
                        _ => return,
                    },
                }
                cx.notify();
            }))
            .child(sheet)
            // the card system is nothing but transitions, so the sheet is
            // repainted on the frame clock rather than on a timer coarse enough
            // to make a 600ms crossfade look like four steps
            .with_animation(
                "games-clock",
                Animation::new(STATE_DWELL).repeat(),
                |sheet, _| sheet,
            )
            .into_any_element();
        Some(view)
    }
}

impl Render for SuperiorityView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        platform::configure_window(window);
        // Connection, warning, and update deadlines belong to the client, not
        // to one product's surface. Keep their clock ahead of every early
        // return below: Remastered otherwise leaves its completed-connection
        // dialog logically visible forever, and the invisible modal consumes
        // every composer keystroke.
        self.advance_tab_animations(window, cx);
        // the shared modal demo replaces the client the same way the game
        // previews do: fixture data on a stage, no session behind it
        if self.modal_preview.showing {
            return self.modal_preview_view(cx);
        }
        // the picker replaces the client rather than sitting inside it: it is
        // dressed in design-time data, and there is no session behind it
        // the picker is the front door, and it steps aside once you are
        // through it
        if let Some(screen) = self.games.showing
            && !self.games.has_entered()
        {
            self.sync_live_games();
            if screen == GamesScreen::Picker && self.updates.update_dialog_visible {
                return self.update_stage(window, cx);
            }
            if let Some(view) = self.games_view(screen, window, cx) {
                return view;
            }
        }
        let afterglow = self.entered_afterglow(window, cx);
        // Remastered has its own window. It is a single-channel realm with no
        // tab strip and its own palette, and dressing StarCraft II's window in
        // green would not have made it one.
        if self.focused == Product::Remastered {
            let scr_window = self.scr_window(window, cx);
            let mut root = div()
                .id("superiority-scr-shell")
                .size_full()
                .relative()
                .children(afterglow)
                .child(self.realm_window_reveal(
                    "scr-window-reveal",
                    "scr-chrome-reveal",
                    scr_window,
                ));
            if let Some(overlay) = self.overlay(window, cx) {
                root = root.child(overlay);
            }
            if self.runtime.legacy_connect_dialog
                && self.runtime.live_mode
                && self.session.connection.dialog_visible
            {
                root = root.child(
                    self.session
                        .connection
                        .overlay(self.focused, &self.chrome, cx),
                );
            }
            if self.warnings.warning_dialog.is_some() {
                root = root.child(self.warnings.overlay(
                    overlays::modal_variant(self.focused),
                    &self.chrome.modal_textures,
                    cx,
                ));
            }
            if self.updates.update_dialog_visible {
                root = root.child(self.updates.overlay(
                    overlays::modal_variant(self.focused),
                    &self.chrome,
                    window,
                    cx,
                ));
            }
            return root.into_any_element();
        }
        // Reforged keeps its own stone-and-gold presentation while sharing
        // SC2's multi-channel tab interaction contract.
        if self.focused == Product::Warcraft3 {
            let wc3_window = self.wc3_window(window, cx);
            let mut root = div()
                .id("superiority-wc3-shell")
                .size_full()
                .relative()
                // the strip's drags are released on the shell, the way the
                // shared window's are
                .on_drag_move::<TabDragPayload>(cx.listener(Self::update_wc3_tab_drag))
                .on_drop(cx.listener(Self::finish_wc3_tab_drag))
                .children(afterglow)
                .child(self.realm_window_reveal(
                    "wc3-window-reveal",
                    "wc3-chrome-reveal",
                    wc3_window,
                ));
            if let Some(overlay) = self.overlay(window, cx) {
                root = root.child(overlay);
            }
            if self.runtime.legacy_connect_dialog
                && self.runtime.live_mode
                && self.session.connection.dialog_visible
            {
                root = root.child(
                    self.session
                        .connection
                        .overlay(self.focused, &self.chrome, cx),
                );
            }
            if self.warnings.warning_dialog.is_some() {
                root = root.child(self.warnings.overlay(
                    overlays::modal_variant(self.focused),
                    &self.chrome.modal_textures,
                    cx,
                ));
            }
            if self.updates.update_dialog_visible {
                root = root.child(self.updates.overlay(
                    overlays::modal_variant(self.focused),
                    &self.chrome,
                    window,
                    cx,
                ));
            }
            return root.into_any_element();
        }
        self.sync_text_inputs(cx);
        self.sync_command_focus(window, cx);
        self.sync_party_scope();

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
            .relative()
            .children(afterglow);
        if self.chrome.modal_assets_warming {
            root = root.child(self.chrome.modal_asset_warmup());
        }
        let online_friends = self
            .session
            .social
            .friends
            .iter()
            .filter(|friend| friend.is_online())
            .count();
        let account_portrait = self
            .local_account()
            .and_then(|(user, _)| user.portrait.clone());
        let navigation = self
            .session
            .channels
            .view(account_portrait, &self.chrome, window, cx);
        let invitations = self.session.join.invitation_stack(cx);
        let affinity =
            RosterAffinity::new(&self.session.channels.tabs, &self.session.social.friends);
        let chat = self.session.chat.panel(
            &self.session.channels,
            &self.settings,
            &self.chrome,
            invitations,
            &affinity,
            cx,
        );
        let roster = self.session.roster.panel(
            &self.session.channels,
            &self.session.social.friends,
            self.selected_user(),
            &self.chrome.ui_assets,
            window,
            cx,
        );
        let command_results = self.command_results();
        let composer = self.session.composer.view(
            window,
            self.session.channels.active().is_some(),
            online_friends,
            command_results,
            self.party_members(),
            &self.chrome.ui_assets,
            cx,
        );
        let chat_chrome = div().id("chat-chrome").absolute().inset_0().child(
            ui_workspace::ChannelWorkspace::new(navigation, chat, roster)
                .footer(composer)
                .background(self.settings.background(self.focused)),
        );
        let conceal_chat = self.session.connection.dialog_visible
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
                gpui::Styled::opacity,
            ));
        } else {
            root = root.child(chat_chrome);
        }
        if let Some(overlay) = self.overlay(window, cx) {
            root = root.child(overlay);
        }
        if self.runtime.legacy_connect_dialog
            && self.runtime.live_mode
            && self.session.connection.dialog_visible
        {
            root = root.child(
                self.session
                    .connection
                    .overlay(self.focused, &self.chrome, cx),
            );
        }
        if self.warnings.warning_dialog.is_some() {
            root = root.child(self.warnings.overlay(
                overlays::modal_variant(self.focused),
                &self.chrome.modal_textures,
                cx,
            ));
        }
        if self.updates.update_dialog_visible {
            root = root.child(self.updates.overlay(
                overlays::modal_variant(self.focused),
                &self.chrome,
                window,
                cx,
            ));
        }
        root.into_any_element()
    }
}
