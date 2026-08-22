use super::*;

fn advance_connection_dialog(connection: &mut ConnectionComponent, now: Instant) {
    if connection.close_due.is_some_and(|due| now >= due) {
        connection.close_due = None;
        connection.dialog_closing = true;
        connection.hide_due = Some(now + MODAL_CLOSE_DURATION);
    }
    if connection.hide_due.is_some_and(|due| now >= due) {
        connection.hide_due = None;
        connection.dialog_visible = false;
        connection.dialog_closing = false;
    }
}

fn social_pane_transition_complete(transition: &SocialPaneTransition, now: Instant) -> bool {
    now.saturating_duration_since(transition.started) >= SOCIAL_PANE_SLIDE_DURATION
}

impl SuperiorityView {
    pub(in crate::app::client) fn advance_tab_animations(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = Instant::now();
        let elapsed = now
            .saturating_duration_since(self.session.connection.progress_updated)
            .as_secs_f32()
            .min(0.1);
        self.session.connection.progress_updated = now;
        if self.session.connection.dialog_visible
            && !self.session.connection.dialog_closing
            && self.session.connection.error.is_none()
        {
            let settled = 1.0 / CONNECTION_RAIL;
            if self.session.connection.floor - self.session.connection.fill > settled {
                let catch_up = 1.0 - 0.82_f32.powf(elapsed * 60.0);
                self.session.connection.fill +=
                    (self.session.connection.floor - self.session.connection.fill) * catch_up;
            } else if self.session.connection.fill < self.session.connection.floor {
                self.session.connection.fill = self.session.connection.floor;
            } else if self.session.connection.ceiling - self.session.connection.fill > settled {
                let creep = 1.0 - 0.978_f32.powf(elapsed * 60.0);
                self.session.connection.fill +=
                    (self.session.connection.ceiling - self.session.connection.fill) * creep;
            }
        }
        advance_connection_dialog(&mut self.session.connection, now);
        if self.warnings.warning_hide_due.is_some_and(|due| now >= due) {
            self.warnings.warning_hide_due = None;
            let close_tab = match self.warnings.warning_dialog.take() {
                Some(WarningDialog::Channel { close_tab, .. }) => close_tab,
                _ => None,
            };
            self.warnings.warning_closing = false;
            if let Some(index) = close_tab
                .filter(|index| self.session.is_sc2() && *index < self.session.channels.tabs.len())
            {
                self.session.channels.tabs.remove(index);
                if self.session.channels.tabs.is_empty() {
                    let id = self.session.channels.next_tab_id;
                    self.session.channels.next_tab_id =
                        self.session.channels.next_tab_id.wrapping_add(1);
                    self.session.channels.tabs.push(ChannelState::pending_live(
                        id,
                        ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL),
                    ));
                }
                if self.session.channels.active_tab >= index && self.session.channels.active_tab > 0
                {
                    self.session.channels.active_tab -= 1;
                }
                self.session.channels.active_tab = self
                    .session
                    .channels
                    .active_tab
                    .min(self.session.channels.tabs.len().saturating_sub(1));
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
        for animation in &mut self.settings.checkbox_animations {
            if animation.is_some_and(|animation| !animation.is_running(now)) {
                *animation = None;
            }
        }
        // Social is shared by every product. Keeping its clock below the SC2
        // branch made SCR/WC3 redraw only when some unrelated event happened,
        // which turned the otherwise smooth pane slide into a few large jumps.
        if self
            .session
            .social
            .social_pane_transition
            .as_ref()
            .is_some_and(|transition| social_pane_transition_complete(transition, now))
        {
            self.session.social.social_pane_transition = None;
            if !self.session.social.social_detail_open {
                self.session.social.conversation_peer = None;
                self.session.social.conversation_input.clear();
            }
        }
        if !self.session.is_sc2() {
            let wc3_tabs_running = self.advance_wc3_tab_animations(now, cx);
            let scr_animation_running = self.session.scr_mut().is_some_and(|scr| {
                scr.roster.finish_transition(now);
                if scr.transcript_reveal.as_ref().is_some_and(|reveal| {
                    now.saturating_duration_since(reveal.started) >= SCR_CHAT_ENTRY_REVEAL_DURATION
                }) {
                    scr.transcript_reveal = None;
                }
                scr.roster.animation.is_some() || scr.transcript_reveal.is_some()
            });
            let running = self.settings.settings_page_transition.is_some()
                || self
                    .settings
                    .checkbox_animations
                    .iter()
                    .any(Option::is_some)
                || self.session.connection.close_due.is_some()
                || self.session.connection.hide_due.is_some()
                || self.warnings.warning_hide_due.is_some()
                || self.updates.update_hide_due.is_some()
                || self.session.social.social_pane_transition.is_some()
                || scr_animation_running
                || wc3_tabs_running
                || (self.session.connection.dialog_visible
                    && !self.session.connection.dialog_closing
                    && self.session.connection.error.is_none()
                    && self.session.connection.fill + 1.0 / CONNECTION_RAIL
                        < self.session.connection.ceiling);
            if running {
                window.request_animation_frame();
            }
            return;
        }
        let finished_close = self
            .session
            .channels
            .tab_close
            .as_mut()
            .and_then(|closing| {
                let started = *closing.started.get_or_insert(now);
                (now.saturating_duration_since(started) >= TAB_CLOSE_DURATION)
                    .then_some(closing.index)
            });
        if let Some(index) = finished_close {
            self.session.channels.tab_close = None;
            self.finish_tab_close(index, cx);
        }

        self.session
            .channels
            .navigation
            .tabs
            .retain_name_animations(now);
        if self
            .session
            .channels
            .tab_selection_started
            .is_some_and(|started| {
                now.saturating_duration_since(started) >= Duration::from_millis(235)
            })
        {
            self.session.channels.tab_selection_started = None;
        }
        if self
            .session
            .channels
            .channel_transition
            .as_ref()
            .is_some_and(|transition| transition.is_complete(now))
        {
            self.session.channels.channel_transition = None;
        }
        self.session.roster.roster.finish_transition(now);
        if self
            .session
            .channels
            .chat_entry_reveal
            .as_ref()
            .is_some_and(|reveal| {
                now.saturating_duration_since(reveal.started) >= CHAT_ENTRY_REVEAL_DURATION
            })
        {
            self.session.channels.chat_entry_reveal = None;
        }
        if self
            .session
            .chat
            .expanded_digest
            .as_ref()
            .is_some_and(|expanded| expanded.is_finished_closing(now))
        {
            self.session.chat.expanded_digest = None;
        }
        let running = self.session.channels.tab_close.is_some()
            || self.session.channels.navigation.tabs.shift_is_running(now)
            || self
                .session
                .channels
                .navigation
                .tabs
                .name_animation_is_running(now)
            || self.session.channels.tab_selection_started.is_some()
            || self.session.channels.channel_transition.is_some()
            || self.settings.settings_page_transition.is_some()
            || self.session.roster.roster.animation.is_some()
            || self.session.channels.chat_entry_reveal.is_some()
            || self
                .session
                .chat
                .expanded_digest
                .as_ref()
                .is_some_and(|expanded| expanded.is_running(now))
            || self.session.social.social_pane_transition.is_some()
            || self
                .settings
                .checkbox_animations
                .iter()
                .any(Option::is_some)
            || self.session.connection.close_due.is_some()
            || self.session.connection.hide_due.is_some()
            || self.warnings.warning_hide_due.is_some()
            || self.updates.update_hide_due.is_some()
            || (self.session.connection.dialog_visible
                && !self.session.connection.dialog_closing
                && self.session.connection.error.is_none()
                && self.session.connection.fill + 1.0 / CONNECTION_RAIL
                    < self.session.connection.ceiling);
        if running {
            window.request_animation_frame();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_social_transition_uses_the_animation_frame_clock() {
        let now = Instant::now();
        let running = SocialPaneTransition {
            forward: true,
            started: now - SOCIAL_PANE_SLIDE_DURATION + Duration::from_millis(1),
        };
        let complete = SocialPaneTransition {
            forward: false,
            started: now - SOCIAL_PANE_SLIDE_DURATION,
        };

        assert!(!social_pane_transition_complete(&running, now));
        assert!(social_pane_transition_complete(&complete, now));
    }

    fn connection(now: Instant) -> ConnectionComponent {
        ConnectionComponent {
            stage: ConnectionStage::Connected,
            error: None,
            starting: false,
            signed_out: false,
            sign_out_requested: false,
            dialog_visible: true,
            dialog_closing: true,
            close_due: None,
            hide_due: Some(now),
            fill: 1.0,
            floor: 1.0,
            ceiling: 1.0,
            progress_updated: now,
        }
    }

    #[test]
    fn elapsed_connection_dialog_hide_releases_modal_input_gate() {
        let now = Instant::now();
        let mut connection = connection(now);

        advance_connection_dialog(&mut connection, now);

        assert!(!connection.dialog_visible);
        assert!(!connection.dialog_closing);
        assert!(connection.hide_due.is_none());
    }
}
