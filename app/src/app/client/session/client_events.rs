use super::*;

fn invalidates_immediately(event: &ClientEvent) -> bool {
    !matches!(
        event,
        ClientEvent::Chat(ChatEvent::Activity { .. } | ChatEvent::Roster(_))
    )
}

impl SuperiorityView {
    pub(in crate::app::client) fn handle_client_event(&mut self, event: ClientEvent) -> bool {
        let invalidates_immediately = invalidates_immediately(&event);
        match event {
            ClientEvent::Stage(stage) => {
                let previous = self.connection.stage;
                self.connection.stage = stage;
                self.connection.starting = false;
                if stage == ConnectionStage::WebAuthentication {
                    self.connection.sign_out_requested = false;
                    self.connection.signed_out = false;
                    self.open_connection_dialog();
                    self.connection.error = None;
                    self.channels.tabs.clear();
                    self.channels.active_tab = 0;
                    self.sync_roster_filter_input();
                    self.social.friends_snapshot.clear();
                    self.social.friends.clear();
                    self.social.blocked_accounts.clear();
                    self.social.conversations.clear();
                    self.social.conversation_peer = None;
                    self.social.conversation_input.clear();
                    self.social.social_detail_open = false;
                    self.social.social_pane_transition = None;
                    self.join.public_channels.clear();
                    self.join.groups.clear();
                    self.join.member_groups.clear();
                    self.join.group_search.clear();
                    self.join.invitations.clear();
                } else if stage == ConnectionStage::GameUtilities
                    && let Some(authenticator) = self.runtime.authenticator.take()
                {
                    authenticator.dismiss();
                } else if stage == ConnectionStage::Connected {
                    self.connection.error = None;
                    self.warnings.warning_dialog = None;
                    self.warnings.warning_closing = false;
                    self.warnings.warning_hide_due = None;
                    self.connection.dialog_visible = true;
                    self.connection.dialog_closing = false;
                    self.connection.close_due = Some(Instant::now() + CONNECTION_CONNECTED_HOLD);
                    self.connection.hide_due = None;
                } else if stage == ConnectionStage::Disconnected
                    && self.connection.sign_out_requested
                {
                    self.join.invitations.clear();
                    self.open_connection_dialog();
                } else if stage == ConnectionStage::Disconnected
                    && previous == ConnectionStage::Connected
                    && self.warnings.warning_dialog.is_none()
                {
                    self.join.invitations.clear();
                    self.show_disconnect_warning(
                        "The connection to Battle.net was lost. Reconnect or quit Superiority."
                            .to_owned(),
                    );
                } else if stage == ConnectionStage::Disconnected
                    && self.warnings.warning_dialog.is_none()
                {
                    self.join.invitations.clear();
                    self.open_connection_dialog();
                }
                self.set_connection_progress_stage(stage);
            }
            ClientEvent::Authentication { url, reply } => {
                if let Some(authenticator) = self.runtime.authenticator.take() {
                    authenticator.dismiss();
                }
                self.runtime.authenticator = Some(WebAuthenticator::present(&url, reply));
            }
            ClientEvent::Chat(event) => self.handle_live_chat(event),
            ClientEvent::CommandError(error) => {
                let line = ChatLine::Error {
                    time: Self::current_timestamp(),
                    text: error,
                };
                if self.channels.active_tab < self.channels.tabs.len() {
                    self.append_chat_line(self.channels.active_tab, line);
                }
            }
            ClientEvent::Error(error) => {
                if self.connection.sign_out_requested {
                    self.open_connection_dialog();
                } else if self.connection.stage == ConnectionStage::Connected
                    || matches!(
                        self.warnings.warning_dialog,
                        Some(WarningDialog::Disconnected { .. })
                    )
                {
                    self.show_disconnect_warning(
                        "The connection to Battle.net was lost. Reconnect or quit Superiority."
                            .to_owned(),
                    );
                    if self.channels.active_tab < self.channels.tabs.len() {
                        self.append_chat_line(
                            self.channels.active_tab,
                            ChatLine::Error {
                                time: Self::current_timestamp(),
                                text: "Battle.net connection lost.".to_owned(),
                            },
                        );
                    }
                } else {
                    self.connection.starting = false;
                    self.connection.error = Some(error.clone());
                    self.connection.ceiling = self.connection.fill;
                    self.open_connection_dialog();
                    if self.channels.active_tab < self.channels.tabs.len() {
                        self.append_chat_line(
                            self.channels.active_tab,
                            ChatLine::Error {
                                time: Self::current_timestamp(),
                                text: format!("Connection error: {error}"),
                            },
                        );
                    }
                }
            }
        }
        invalidates_immediately
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_and_noop_chat_events_do_not_invalidate_immediately() {
        assert!(!invalidates_immediately(&ClientEvent::Chat(
            ChatEvent::Activity { route: (None, 0) }
        )));
        assert!(!invalidates_immediately(&ClientEvent::Chat(
            ChatEvent::Roster(RosterSnapshot {
                channel_index: 1,
                initial_complete: true,
                users: Vec::new(),
            })
        )));
        assert!(invalidates_immediately(&ClientEvent::Stage(
            ConnectionStage::Connected
        )));
    }
}
