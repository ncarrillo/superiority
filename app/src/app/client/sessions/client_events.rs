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
        // background events are handled while their session is temporarily
        // focused, so this is also the product that emitted the event.
        let event_product = self.focused;
        match event {
            // Remastered's chat arrives as itself rather than as StarCraft II's
            // ChatEvent and is projected into its own product state.
            ClientEvent::Classic(event) => {
                self.append_classic_line(&event);
            }
            ClientEvent::ClassicChannel(channel) => {
                self.apply_classic_channel(&channel);
            }
            ClientEvent::ClassicFriends(friends) => {
                self.apply_classic_friends(friends);
            }
            ClientEvent::ClassicWhisperSent { peer, body } => {
                self.record_classic_whisper(
                    peer.clone(),
                    WhisperTarget::Name(peer),
                    body,
                    true,
                    Self::current_timestamp(),
                );
            }
            ClientEvent::WarcraftChannel(channel) => {
                self.apply_warcraft_channel(&channel);
            }
            ClientEvent::Warcraft(event) => {
                self.append_warcraft_event(&event);
            }
            ClientEvent::WarcraftFriends(friends) => {
                self.apply_warcraft_friends(friends);
            }
            ClientEvent::WarcraftClan(clan) => {
                self.apply_warcraft_clan(clan);
            }
            ClientEvent::WarcraftChannels(channels) => {
                self.apply_warcraft_channels(channels);
            }
            ClientEvent::ProductCredential {
                product,
                credential,
            } => {
                // only the authoritative SC2 Front session mints credentials
                // for its queued product workers. Install before Account is
                // handled below; Account is the event that starts the queue.
                if event_product == Product::StarCraft2
                    && product != Product::StarCraft2
                    && let Some(commands) = self
                        .product_session(product)
                        .and_then(|session| session.commands.as_ref())
                {
                    let _ = commands.send(ClientCommand::InstallCredential(credential));
                }
            }
            ClientEvent::Stage(stage) => {
                if self.session.connection.sign_out_requested
                    && stage != ConnectionStage::Disconnected
                {
                    return invalidates_immediately;
                }
                let previous = self.session.connection.stage;
                self.session.connection.stage = stage;
                self.session.connection.starting = false;
                if stage == ConnectionStage::WebAuthentication {
                    self.session.connection.sign_out_requested = false;
                    self.session.connection.signed_out = false;
                    self.open_connection_dialog();
                    self.session.connection.error = None;
                    self.session.social.clear();
                    if let Some(wc3) = self.session.wc3_mut() {
                        wc3.clear();
                    } else if let Some(scr) = self.session.scr_mut() {
                        scr.clear();
                    } else {
                        self.session.channels.tabs.clear();
                        self.session.channels.active_tab = 0;
                        self.sync_roster_filter_input();
                        self.session.join.public_channels.clear();
                        self.session.join.groups.clear();
                        self.session.join.member_groups.clear();
                        self.session.join.group_search.clear();
                        self.session.join.invitations.clear();
                    }
                } else if stage == ConnectionStage::GameUtilities
                    && let Some(authenticator) = self.runtime.authenticator.take()
                {
                    authenticator.dismiss();
                } else if stage == ConnectionStage::Connected {
                    self.session.connection.error = None;
                    self.warnings.warning_dialog = None;
                    self.warnings.warning_closing = false;
                    self.warnings.warning_hide_due = None;
                    self.session.connection.dialog_visible = true;
                    self.session.connection.dialog_closing = false;
                    self.session.connection.close_due =
                        Some(Instant::now() + CONNECTION_CONNECTED_HOLD);
                    self.session.connection.hide_due = None;
                } else if stage == ConnectionStage::Disconnected
                    && self.session.connection.sign_out_requested
                {
                    if self.session.is_sc2() {
                        self.session.join.invitations.clear();
                    }
                    self.open_connection_dialog();
                } else if stage == ConnectionStage::Disconnected
                    && previous == ConnectionStage::Connected
                    && self.warnings.warning_dialog.is_none()
                {
                    if self.session.is_sc2() {
                        self.session.join.invitations.clear();
                    }
                    self.show_disconnect_warning(
                        "The connection to Battle.net was lost. Reconnect or quit Superiority."
                            .to_owned(),
                    );
                } else if stage == ConnectionStage::Disconnected
                    && self.warnings.warning_dialog.is_none()
                {
                    if self.session.is_sc2() {
                        self.session.join.invitations.clear();
                    }
                    self.open_connection_dialog();
                }
                self.set_connection_progress_stage(stage);
            }
            ClientEvent::Authentication {
                url,
                reply,
                product,
                fresh_account,
            } => {
                if self.session.connection.sign_out_requested {
                    let _ = reply.send(Err(crate::Error::Authentication(
                        "Battle.net authentication was cancelled by sign-out".into(),
                    )));
                    return invalidates_immediately;
                }
                if let Some(authenticator) = self.runtime.authenticator.take() {
                    authenticator.dismiss();
                }
                self.runtime.authenticator = Some(WebAuthenticator::present(
                    &url,
                    reply,
                    product,
                    fresh_account,
                ));
            }
            ClientEvent::Account(account) => {
                if !self.session.connection.sign_out_requested {
                    if event_product == Product::StarCraft2
                        && let Some(account_id) = account.account_id
                        && let Some(games) = account.games.clone()
                    {
                        Self::trace(format_args!("account licenses: {}", games.join(", ")));
                        self.adopt_authoritative_account(
                            account_id,
                            account.battle_tag.clone(),
                            account.region,
                            games,
                        );
                    }
                    self.session.account_id = account.account_id;
                    self.session.account_battle_tag = account.battle_tag;
                    self.session.account_region = account.region;
                }
            }
            ClientEvent::Chat(event) => {
                if self.session.is_sc2() {
                    self.handle_live_chat(event);
                }
            }
            ClientEvent::CommandError(error) => {
                self.append_product_error(error);
            }
            ClientEvent::Error(error) => {
                if self.session.connection.sign_out_requested {
                    self.open_connection_dialog();
                } else if self.session.connection.stage == ConnectionStage::Connected
                    || matches!(
                        self.warnings.warning_dialog,
                        Some(WarningDialog::Disconnected { .. })
                    )
                {
                    self.show_disconnect_warning(
                        "The connection to Battle.net was lost. Reconnect or quit Superiority."
                            .to_owned(),
                    );
                    self.append_product_error("Battle.net connection lost.".to_owned());
                } else {
                    self.session.connection.starting = false;
                    self.session.connection.error = Some(error.clone());
                    self.session.connection.ceiling = self.session.connection.fill;
                    self.open_connection_dialog();
                    self.append_product_error(format!("Connection error: {error}"));
                }
            }
        }
        invalidates_immediately
    }

    fn append_product_error(&mut self, text: String) {
        let time = Self::current_timestamp();
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.append_error(time, text);
            return;
        }
        if let Some(scr) = self.session.scr_mut() {
            scr.append_error(time, text);
            return;
        }
        let index = self.session.channels.active_tab;
        if index < self.session.channels.tabs.len() {
            self.append_chat_line(index, ChatLine::Error { time, text });
        }
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
