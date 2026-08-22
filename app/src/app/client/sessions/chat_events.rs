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
                self.session
                    .join
                    .awaiting_joins
                    .retain(|(awaited, _)| awaited != &channel);
                let title = self.channel_label(&channel);
                let index = self
                    .session
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel.as_ref() == Some(&channel))
                    .unwrap_or_else(|| {
                        let id = self.session.channels.next_tab_id;
                        self.session.channels.next_tab_id =
                            self.session.channels.next_tab_id.wrapping_add(1);
                        self.session
                            .channels
                            .tabs
                            .push(ChannelState::pending_live(id, channel.clone()));
                        self.session.channels.tabs.len() - 1
                    });
                let transcript_was_empty = self.session.channels.tabs[index].transcript.is_empty();
                self.session.channels.tabs[index].title.clone_from(&title);
                self.session.channels.tabs[index].channel = Some(channel);
                self.session.channels.tabs[index].channel_index = Some(channel_index);
                self.session.channels.tabs[index].shard_index = shard_index;
                self.session.channels.tabs[index].local_member_handle = Some(local_member_handle);
                if transcript_was_empty {
                    self.append_chat_line(
                        index,
                        ChatLine::SessionStart {
                            time: Self::current_timestamp(),
                            channel: title.clone(),
                        },
                    );
                }
                if self.session.channels.tabs.len() == 1 {
                    self.session.channels.active_tab = 0;
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
                    .session
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    let member = UiUser::live(&user, &mut self.session.roster.portraits);
                    self.append_membership(index, MembershipKind::Joined, member);
                }
            }
            ChatEvent::MemberLeft {
                channel_index,
                user,
                ..
            } => {
                if let Some(index) = self
                    .session
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    let member = UiUser::live(&user, &mut self.session.roster.portraits);
                    self.append_membership(index, MembershipKind::Left, member);
                }
            }
            ChatEvent::Removed {
                channel_index,
                reason,
            } => {
                if let Some(index) = self
                    .session
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    let label = self.session.channels.tabs[index].title.clone();
                    let detail = match reason {
                        Some(CHAT_LEAVE_BANNED) => format!("You were banned from {label}."),
                        Some(code) if code != 0 => {
                            let reason = superiority_core::native::errors::description(code);
                            format!(
                                "You were removed from {label}: {}.",
                                reason.trim_end_matches('.')
                            )
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
                // the party has no tab of its own, and one would take its talk
                // out of the room you are reading. party lines go into every
                // channel instead — the party travels with you, so wherever you
                // are is where the conversation is.
                if self.party_channel_index() == Some(channel_index) {
                    let mut sender = UiUser::live(&sender, &mut self.session.roster.portraits);
                    sender.tone = RosterUserTone::Party;
                    let time = Self::current_timestamp();
                    for index in 0..self.session.channels.tabs.len() {
                        if self.session.channels.tabs[index].channel.as_ref()
                            == Some(&ChatChannel::Party)
                        {
                            continue;
                        }
                        self.append_chat_line(
                            index,
                            ChatLine::Message {
                                time: time.clone(),
                                sender: sender.clone(),
                                text: body.clone(),
                            },
                        );
                    }
                    // it is in front of you in every one of them, so none of
                    // them is holding anything you have not seen
                    return;
                }
                if let Some(index) = self
                    .session
                    .channels
                    .tabs
                    .iter()
                    .position(|tab| tab.channel_index == Some(channel_index))
                {
                    let sender = UiUser::live(&sender, &mut self.session.roster.portraits);
                    self.append_chat_line(
                        index,
                        ChatLine::Message {
                            time: Self::current_timestamp(),
                            sender,
                            text: body,
                        },
                    );
                    if index != self.session.channels.active_tab {
                        self.session.channels.tabs[index].unread = true;
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
                member_count,
                online,
            } => {
                if member {
                    self.session.join.member_groups.insert(club_id);
                }
                if let Some(name) = name {
                    for invitation in &mut self.session.join.invitations {
                        if matches!(invitation.kind, InvitationKind::Group { club_id: id } if id == club_id)
                            && invitation.destination.is_none()
                        {
                            invitation.destination = Some(name.clone());
                        }
                    }
                    // the summary announces a group before the club-info
                    // lookup answers, so a later event fills the online count
                    // in rather than clearing what is already known.
                    let known = self.session.join.groups.get(&club_id);
                    let member_count = member_count.or_else(|| known.and_then(|g| g.member_count));
                    let online = online.or_else(|| known.and_then(|group| group.online));
                    self.session.join.groups.insert(
                        club_id,
                        UiGroupSummary {
                            name: name.clone(),
                            private,
                            kind,
                            category,
                            member_count,
                            online,
                        },
                    );
                    self.session
                        .join
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
                    self.session.join.public_channels = channels
                        .into_iter()
                        .map(|channel| (channel.identifier, channel.name))
                        .collect();
                    self.retitle_public_tabs();
                }
            }
            ChatEvent::ConferenceDescriptions {
                conferences,
                complete,
            } => {
                Self::trace(format_args!(
                    "conference directory entries={} complete={complete} channels={}",
                    conferences.len(),
                    conferences
                        .iter()
                        .map(|conference| conference.channel_name_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                ));
                // we always join with enUS, so those are the rooms we would
                // actually land in; other locales are other people's channels
                for conference in conferences
                    .iter()
                    .filter(|conference| conference.locale_tag() == JOIN_LOCALE)
                {
                    self.session
                        .join
                        .channel_conferences
                        .entry(conference.channel_name_id)
                        .or_default()
                        .push(conference.conference_id);
                }
                if complete {
                    self.session.join.directory_complete = true;
                }
            }
            ChatEvent::ConferenceMemberCounts { counts, complete } => {
                // live per-conference head counts. nothing SC2 asks for maps a
                // conference back to the channel it serves, so these are kept
                // for the protocol viewer and traced, not shown in the UI.
                Self::trace(format_args!(
                    "conference member counts entries={} complete={complete} occupied={}",
                    counts.len(),
                    counts.iter().filter(|count| count.members > 0).count()
                ));
                self.session.join.conference_members = counts
                    .iter()
                    .map(|count| (count.conference_id, count.members))
                    .collect();
            }
            ChatEvent::Friends(friends) => {
                if self.session.social.friends_snapshot != friends {
                    Self::trace(format_args!("friends snapshot={}", friends.len()));
                    self.session.social.friends = friends
                        .iter()
                        .map(|friend| UiFriend::live(friend, &mut self.session.roster.portraits))
                        .collect();
                    self.session.social.friends_snapshot = friends;
                }
            }
            ChatEvent::GroupSearch { club_ids } => {
                Self::trace(format_args!("group search results={}", club_ids.len()));
                self.session.join.group_search = club_ids;
            }
            ChatEvent::Whisper {
                peer,
                body,
                outgoing,
            } => {
                self.session
                    .social
                    .record_whisper(peer, body, outgoing, Self::current_timestamp());
            }
            ChatEvent::GroupInvitation { club_id } => self.present_group_invitation(club_id),
            ChatEvent::PartyInvitation {
                inviter,
                channel_index,
            } => self.present_party_invitation(inviter, channel_index),
            ChatEvent::BlockedAccounts(accounts) => self.session.social.blocked_accounts = accounts,
            ChatEvent::Activity { .. } => {}
            ChatEvent::WhisperFailed { peer, reason } => {
                if self.session.channels.active_tab < self.session.channels.tabs.len() {
                    self.append_chat_line(
                        self.session.channels.active_tab,
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
