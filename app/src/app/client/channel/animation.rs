use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn advance_tab_animations(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = Instant::now();
        let elapsed = now
            .saturating_duration_since(self.connection.progress_updated)
            .as_secs_f32()
            .min(0.1);
        self.connection.progress_updated = now;
        if self.connection.dialog_visible
            && !self.connection.dialog_closing
            && self.connection.error.is_none()
        {
            let settled = 1.0 / CONNECTION_RAIL;
            if self.connection.floor - self.connection.fill > settled {
                let catch_up = 1.0 - 0.82_f32.powf(elapsed * 60.0);
                self.connection.fill += (self.connection.floor - self.connection.fill) * catch_up;
            } else if self.connection.fill < self.connection.floor {
                self.connection.fill = self.connection.floor;
            } else if self.connection.ceiling - self.connection.fill > settled {
                let creep = 1.0 - 0.978_f32.powf(elapsed * 60.0);
                self.connection.fill += (self.connection.ceiling - self.connection.fill) * creep;
            }
        }
        if self.connection.close_due.is_some_and(|due| now >= due) {
            self.connection.close_due = None;
            self.connection.dialog_closing = true;
            self.connection.hide_due = Some(now + MODAL_CLOSE_DURATION);
        }
        if self.connection.hide_due.is_some_and(|due| now >= due) {
            self.connection.hide_due = None;
            self.connection.dialog_visible = false;
            self.connection.dialog_closing = false;
        }
        if self.warnings.warning_hide_due.is_some_and(|due| now >= due) {
            self.warnings.warning_hide_due = None;
            let close_tab = match self.warnings.warning_dialog.take() {
                Some(WarningDialog::Channel { close_tab, .. }) => close_tab,
                _ => None,
            };
            self.warnings.warning_closing = false;
            if let Some(index) = close_tab.filter(|index| *index < self.channels.tabs.len()) {
                self.channels.tabs.remove(index);
                if self.channels.tabs.is_empty() {
                    let id = self.channels.next_tab_id;
                    self.channels.next_tab_id = self.channels.next_tab_id.wrapping_add(1);
                    self.channels.tabs.push(ChannelState::pending_live(
                        id,
                        ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL),
                    ));
                }
                if self.channels.active_tab >= index && self.channels.active_tab > 0 {
                    self.channels.active_tab -= 1;
                }
                self.channels.active_tab = self
                    .channels
                    .active_tab
                    .min(self.channels.tabs.len().saturating_sub(1));
                self.sync_roster_filter_input();
                self.persist_open_channels();
            }
        }
        if self.updates.update_hide_due.is_some_and(|due| now >= due) {
            self.updates.update_hide_due = None;
            self.updates.update_dialog_visible = false;
            self.updates.update_dialog_closing = false;
            self.updates.update_notes_selection.clear();
            self.begin_startup_connection(cx);
        }
        let finished_close = self.channels.tab_close.as_mut().and_then(|closing| {
            let started = *closing.started.get_or_insert(now);
            (now.saturating_duration_since(started) >= TAB_CLOSE_DURATION).then_some(closing.index)
        });
        if let Some(index) = finished_close {
            self.channels.tab_close = None;
            self.finish_tab_close(index, cx);
        }

        self.channels.navigation.tabs.retain_name_animations(now);
        if self.channels.tab_selection_started.is_some_and(|started| {
            now.saturating_duration_since(started) >= Duration::from_millis(235)
        }) {
            self.channels.tab_selection_started = None;
        }
        if self
            .channels
            .channel_transition
            .as_ref()
            .is_some_and(|transition| transition.is_complete(now))
        {
            self.channels.channel_transition = None;
        }
        if self
            .settings
            .settings_page_transition
            .as_ref()
            .is_some_and(|transition| {
                now.saturating_duration_since(transition.started)
                    >= SETTINGS_PAGE_CROSSFADE_DURATION
            })
        {
            self.settings.settings_page_transition = None;
        }
        self.roster.roster.finish_transition(now);
        if self
            .channels
            .chat_entry_reveal
            .as_ref()
            .is_some_and(|reveal| {
                now.saturating_duration_since(reveal.started) >= CHAT_ENTRY_REVEAL_DURATION
            })
        {
            self.channels.chat_entry_reveal = None;
        }
        if self
            .social
            .social_pane_transition
            .as_ref()
            .is_some_and(|transition| {
                now.saturating_duration_since(transition.started) >= SOCIAL_PANE_SLIDE_DURATION
            })
        {
            self.social.social_pane_transition = None;
            if !self.social.social_detail_open {
                self.social.conversation_peer = None;
                self.social.conversation_input.clear();
            }
        }
        for animation in &mut self.settings.checkbox_animations {
            if animation.is_some_and(|animation| !animation.is_running(now)) {
                *animation = None;
            }
        }

        let running = self.channels.tab_close.is_some()
            || self.channels.navigation.tabs.shift_is_running(now)
            || self.channels.navigation.tabs.name_animation_is_running(now)
            || self.channels.tab_selection_started.is_some()
            || self.channels.channel_transition.is_some()
            || self.settings.settings_page_transition.is_some()
            || self.roster.roster.animation.is_some()
            || self.channels.chat_entry_reveal.is_some()
            || self.social.social_pane_transition.is_some()
            || self
                .settings
                .checkbox_animations
                .iter()
                .any(Option::is_some)
            || self.connection.close_due.is_some()
            || self.connection.hide_due.is_some()
            || self.warnings.warning_hide_due.is_some()
            || self.updates.update_hide_due.is_some()
            || (self.connection.dialog_visible
                && !self.connection.dialog_closing
                && self.connection.error.is_none()
                && self.connection.fill + 1.0 / CONNECTION_RAIL < self.connection.ceiling);
        if running {
            window.request_animation_frame();
        }
    }
}
