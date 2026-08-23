use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn current_timestamp() -> String {
        platform::current_timestamp()
    }

    pub(in crate::app::client) fn poll_app_menu_events(&mut self, cx: &mut Context<Self>) -> bool {
        let events = self.runtime.app_menu_events.try_iter().collect::<Vec<_>>();
        if events.is_empty() {
            return false;
        }
        for event in events {
            match event {
                AppMenuCommand::CheckForUpdates => {
                    Self::trace("native Check for Updates menu action");
                    self.check_for_updates(cx);
                }
                AppMenuCommand::OpenSettings => {
                    Self::trace("native Settings menu action");
                    self.open_settings(cx);
                }
            }
        }
        true
    }

    pub(in crate::app::client) fn poll_client(&mut self, cx: &mut Context<Self>) {
        let menu_changed = self.poll_app_menu_events(cx);
        let update_changed = self.poll_update_events(cx);
        let events = self
            .session
            .events
            .as_ref()
            .map(|events| events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut event_changed = false;
        for event in events {
            event_changed |= self.handle_client_event(event);
        }
        // draining a background session swaps it into `self.session` and runs
        // its handlers, which moves the composer entity out from under the
        // window's keyboard focus and back — enough to drop it. While the user
        // is typing in the foreground, that showed up as characters landing on
        // an unfocused input and vanishing. So the background drain waits until
        // the foreground is not holding the keyboard; its cards catch up the
        // moment you stop typing.
        let foreground_typing = if let Some(wc3) = self.session.wc3() {
            wc3.composer_focused || wc3.roster.focused
        } else if let Some(scr) = self.session.scr() {
            scr.composer_focused
        } else {
            self.session.composer.composer_focused || self.session.roster.roster.focused
        };
        if !foreground_typing {
            event_changed |= self.poll_background_products();
        }
        self.advance_connect_queue();
        let (expired_joins, flushed_roster) = if self.session.is_sc2() {
            let expired_joins = self.expire_awaited_joins();
            let commands = self.session.commands.clone();
            self.session
                .join
                .flush_group_search_if_due(commands.as_ref());
            (expired_joins, self.flush_rosters_if_due())
        } else {
            (false, false)
        };
        let live_auth_failed = self
            .runtime
            .uplink
            .stats
            .auth_failed
            .load(std::sync::atomic::Ordering::Relaxed);
        let new_live_auth_failure =
            self.session.is_sc2() && live_auth_failed && !self.runtime.live_auth_notified;
        if new_live_auth_failure {
            self.runtime.live_auth_notified = true;
            if !self.session.channels.tabs.is_empty() {
                self.append_chat_line(
                    self.session.channels.active_tab,
                    ChatLine::Notice {
                        time: Self::current_timestamp(),
                        text: "Live sharing authentication failed — sharing is paused until the app restarts or a new link is made.".to_owned(),
                    },
                );
            }
        }
        let live_page_visible = self.overlays.active == Some(Overlay::Settings)
            && self.settings.active_settings_page == 3
            && self.settings.live_enabled;
        if menu_changed
            || update_changed
            || event_changed
            || expired_joins
            || flushed_roster
            || live_page_visible
            || new_live_auth_failure
        {
            cx.notify();
        }
    }
}

impl SuperiorityView {
    /// drains the products that are running but not on screen.
    ///
    /// Every playable product connects, so a session you are not looking at is
    /// still signing in, still joining, still being talked to. Without this its
    /// card would never leave the state it started in and its channel would
    /// stall behind a full queue.
    ///
    /// Each product's events are applied with that product focused, because
    /// every handler writes to `self.session` and reads `self.focused` — the
    /// swap is a move of one struct, and the focus is put back before anything
    /// draws.
    fn poll_background_products(&mut self) -> bool {
        let waiting: Vec<(Product, Vec<ClientEvent>)> = self
            .suspended
            .iter()
            .filter_map(|(product, session)| {
                let events: Vec<_> = session.events.as_ref()?.try_iter().collect();
                (!events.is_empty()).then_some((*product, events))
            })
            .collect();
        if waiting.is_empty() {
            return false;
        }

        let focused = self.focused;
        let mut changed = false;
        for (product, events) in waiting {
            if !self.focus_product(product) {
                continue;
            }
            for event in events {
                changed |= self.handle_client_event(event);
            }
        }
        // whatever was in front stays in front: this is bookkeeping, not a
        // switch the user asked for
        self.focus_product(focused);
        changed
    }
}
