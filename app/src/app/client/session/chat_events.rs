use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn handle_live_chat(&mut self, event: ChatEvent) {
        match event {
            ChatEvent::Joined {
                channel_index,
                channel,
                local_member_handle,
                shard_index,
            } => {
                self.join
                    .awaiting_joins
                    .retain(|(awaited, _)| awaited != &channel);
                let title = self.channel_label(&channel);
                let index = self
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel.as_ref() == Some(&channel))
                    .unwrap_or_else(|| {
                        let id = self.channels.next_tab_id;
                        self.channels.next_tab_id = self.channels.next_tab_id.wrapping_add(1);
                        self.channels
                            .tabs
                            .push(ChannelState::pending_live(id, channel.clone()));
                        self.channels.tabs.len() - 1
                    });
                let transcript_was_empty = self.channels.tabs[index].transcript.is_empty();
                self.channels.tabs[index].title.clone_from(&title);
                self.channels.tabs[index].channel = Some(channel);
                self.channels.tabs[index].channel_index = Some(channel_index);
                self.channels.tabs[index].shard_index = shard_index;
                self.channels.tabs[index].local_member_handle = Some(local_member_handle);
                if transcript_was_empty {
                    self.append_chat_line(
                        index,
                        ChatLine::Notice {
                            time: Self::current_timestamp(),
                            text: format!("Joined {title}."),
                        },
                    );
                }
                if self.channels.tabs.len() == 1 {
                    self.channels.active_tab = 0;
                    self.sync_roster_filter_input();
                }
                self.persist_open_channels();
            }
            ChatEvent::JoinRejected { channel, reason } => {
                self.reject_pending_join(channel.as_ref(), reason);
            }
            ChatEvent::Roster(snapshot) => self.queue_roster(snapshot),
            ChatEvent::MemberJoined {
                channel_index,
                user,
            } => {
                if let Some(index) = self
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    self.append_chat_line(
                        index,
                        ChatLine::Membership {
                            time: Self::current_timestamp(),
                            text: format!("{} joined the channel.", user.visible_name()),
                        },
                    );
                }
            }
            ChatEvent::MemberLeft {
                channel_index,
                user,
                ..
            } => {
                if let Some(index) = self
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    self.append_chat_line(
                        index,
                        ChatLine::Membership {
                            time: Self::current_timestamp(),
                            text: format!("{} left the channel.", user.visible_name()),
                        },
                    );
                }
            }
            ChatEvent::Removed {
                channel_index,
                reason,
            } => {
                if let Some(index) = self
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    let label = self.channels.tabs[index].title.clone();
                    let detail = match reason {
                        Some(CHAT_LEAVE_BANNED) => format!("You were banned from {label}."),
                        Some(code) if code != 0 => {
                            format!("You were removed from {label} (Battle.net reason {code}).")
                        }
                        _ => format!("You were removed from {label}."),
                    };
                    self.show_channel_warning("CHANNEL", detail, Some(index));
                }
            }
            ChatEvent::Message {
                channel_index,
                sender,
                body,
            } => {
                if let Some(index) = self
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    let sender = UiUser::live(&sender, &mut self.roster.portraits);
                    self.append_chat_line(
                        index,
                        ChatLine::Message {
                            time: Self::current_timestamp(),
                            sender,
                            text: body,
                        },
                    );
                    if index != self.channels.active_tab {
                        self.channels.tabs[index].unread = true;
                    }
                }
            }
            ChatEvent::GroupSummary {
                club_id,
                name,
                kind,
                category,
                private,
                member,
            } => {
                if member {
                    self.join.member_groups.insert(club_id);
                }
                if let Some(name) = name {
                    for invitation in &mut self.join.invitations {
                        if matches!(invitation.kind, InvitationKind::Group { club_id: id } if id == club_id)
                            && invitation.destination.is_none()
                        {
                            invitation.destination = Some(name.clone());
                        }
                    }
                    self.join.groups.insert(
                        club_id,
                        UiGroupSummary {
                            name: name.clone(),
                            private,
                            kind,
                            category,
                        },
                    );
                    self.join
                        .remembered_group_names
                        .insert(club_id, name.clone());
                    preferences::remember_group_name(club_id, &name);
                    self.retitle_club_tabs(club_id, &name);
                }
            }
            ChatEvent::PublicChannelCatalog(channels) => {
                Self::trace(format_args!(
                    "public channel catalog entries={}",
                    channels.len()
                ));
                if !channels.is_empty() {
                    self.join.public_channels = channels
                        .into_iter()
                        .map(|channel| (channel.identifier, channel.name))
                        .collect();
                    self.retitle_public_tabs();
                }
            }
            ChatEvent::ConferenceDirectory { .. } => {}
            ChatEvent::Friends(friends) => {
                if self.social.friends_snapshot != friends {
                    Self::trace(format_args!("friends snapshot={}", friends.len()));
                    self.social.friends = friends
                        .iter()
                        .map(|friend| UiFriend::live(friend, &mut self.roster.portraits))
                        .collect();
                    self.social.friends_snapshot = friends;
                }
            }
            ChatEvent::GroupSearch { club_ids } => {
                Self::trace(format_args!("group search results={}", club_ids.len()));
                self.join.group_search = club_ids;
                self.join.join_selected = 0;
            }
            ChatEvent::Whisper {
                peer,
                body,
                outgoing,
            } => self
                .social
                .record_whisper(peer, body, outgoing, Self::current_timestamp()),
            ChatEvent::GroupInvitation { club_id } => self.present_group_invitation(club_id),
            ChatEvent::PartyInvitation {
                inviter,
                channel_index,
            } => self.present_party_invitation(inviter, channel_index),
            ChatEvent::BlockedAccounts(accounts) => self.social.blocked_accounts = accounts,
            ChatEvent::Activity { .. } => {}
            ChatEvent::WhisperFailed { peer, reason } => {
                if self.channels.active_tab < self.channels.tabs.len() {
                    self.append_chat_line(
                        self.channels.active_tab,
                        ChatLine::Error {
                            time: Self::current_timestamp(),
                            text: format!("whisper to {peer} failed: {reason}"),
                        },
                    );
                }
            }
        }
    }
}
