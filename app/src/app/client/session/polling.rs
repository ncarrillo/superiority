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
            .runtime
            .events
            .as_ref()
            .map(|events| events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut event_changed = false;
        for event in events {
            event_changed |= self.handle_client_event(event);
        }
        let expired_joins = self.expire_awaited_joins();
        self.join
            .flush_group_search_if_due(self.runtime.commands.as_ref());
        let flushed_roster = self.flush_rosters_if_due();
        let live_auth_failed = self
            .runtime
            .uplink
            .stats
            .auth_failed
            .load(std::sync::atomic::Ordering::Relaxed);
        let new_live_auth_failure = live_auth_failed && !self.runtime.live_auth_notified;
        if new_live_auth_failure {
            self.runtime.live_auth_notified = true;
            if !self.channels.tabs.is_empty() {
                self.append_chat_line(
                    self.channels.active_tab,
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
