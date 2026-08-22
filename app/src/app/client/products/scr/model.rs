use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum ScrMessageKind {
    Talk,
    Whisper,
    Emote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum ScrNoticeKind {
    Broadcast,
    Information,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::client) struct ScrMember {
    pub(in crate::app::client) handle: u32,
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) battle_tag: Option<String>,
    pub(in crate::app::client) is_operator: bool,
    pub(in crate::app::client) presence: ScrPresence,
    /// A resolved ToonProfile/Url result. LegacyChat supplies the identity and
    /// presence attributes; SC:R's profile adapter hydrates this separately.
    pub(in crate::app::client) avatar_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum ScrPresence {
    Online,
    Away,
    Busy,
    InGame,
    InLobby,
}

impl ScrPresence {
    /// The protocol layer reads the presence attributes; this only names the
    /// result in the client's own enum, so the Live publisher and the window
    /// cannot read the same member two ways.
    pub(in crate::app::client) fn from_member(member: &ClassicChatUser) -> Self {
        use superiority_core::games::scr::chat::MemberPresence;
        match member.presence() {
            MemberPresence::Online => Self::Online,
            MemberPresence::Away => Self::Away,
            MemberPresence::Busy => Self::Busy,
            MemberPresence::InGame => Self::InGame,
            MemberPresence::InLobby => Self::InLobby,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum ScrLine {
    SessionStart {
        time: String,
        channel: String,
    },
    Message {
        time: String,
        kind: ScrMessageKind,
        sender: String,
        text: String,
    },
    Notice {
        time: String,
        kind: ScrNoticeKind,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
}

pub(in crate::app::client) struct ScrChannelState {
    pub(in crate::app::client) id: u32,
    pub(in crate::app::client) title: String,
    pub(in crate::app::client) transcript: Vec<ScrLine>,
    pub(in crate::app::client) members: Vec<ScrMember>,
    pub(in crate::app::client) roster_filter: String,
}

pub(in crate::app::client) struct ScrTranscriptReveal {
    pub(in crate::app::client) channel_id: u32,
    pub(in crate::app::client) line_index: usize,
    pub(in crate::app::client) started: Instant,
}

pub(in crate::app::client) struct ScrSessionUi {
    pub(in crate::app::client) composer: ui_text_input::TextInput,
    pub(in crate::app::client) composer_focused: bool,
    pub(in crate::app::client) roster_input: ui_text_input::TextInput,
    pub(in crate::app::client) roster: ui_workspace_pattern::RosterState<u32, ScrMember, Instant>,
    pub(in crate::app::client) transcript: ui_scr_chat::TranscriptState,
    pub(in crate::app::client) transcript_reveal: Option<ScrTranscriptReveal>,
    member_handles: BTreeMap<String, u32>,
    next_member_handle: u32,
    pub(in crate::app::client) channel: Option<ScrChannelState>,
}

impl ScrSessionUi {
    pub(in crate::app::client) fn new(
        composer: ui_text_input::TextInput,
        roster_input: ui_text_input::TextInput,
    ) -> Self {
        Self {
            composer,
            composer_focused: false,
            roster_input,
            roster: ui_workspace_pattern::RosterState::default(),
            transcript: ui_scr_chat::TranscriptState::default(),
            transcript_reveal: None,
            member_handles: BTreeMap::new(),
            next_member_handle: 1,
            channel: None,
        }
    }

    pub(in crate::app::client) fn clear(&mut self) {
        self.composer.clear();
        self.roster_input.clear();
        self.composer_focused = false;
        self.roster.clear_interaction();
        self.roster.selections.clear();
        self.transcript.selection.clear();
        self.transcript_reveal = None;
        self.channel = None;
    }

    pub(in crate::app::client) fn apply_channel(
        &mut self,
        channel: &ClassicChatChannel,
        time: String,
    ) {
        let title = channel.label().to_owned();
        let previous_avatars = self
            .channel
            .as_ref()
            .filter(|current| current.id == channel.channel_id)
            .into_iter()
            .flat_map(|current| current.members.iter())
            .filter_map(|member| {
                member
                    .avatar_url
                    .as_ref()
                    .map(|source| (member.name.to_ascii_lowercase(), source.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let members = channel
            .users
            .iter()
            .map(|member| {
                let identity = member.name.to_ascii_lowercase();
                let handle = self
                    .member_handles
                    .get(&identity)
                    .copied()
                    .unwrap_or_else(|| {
                        let handle = self.next_member_handle;
                        self.next_member_handle = self.next_member_handle.wrapping_add(1).max(1);
                        self.member_handles.insert(identity.clone(), handle);
                        handle
                    });
                ScrMember {
                    handle,
                    name: member.name.clone(),
                    battle_tag: member.attribute("battle_tag").map(str::to_owned),
                    is_operator: member.is_operator,
                    presence: ScrPresence::from_member(member),
                    avatar_url: avatar::from_member(member)
                        .or_else(|| previous_avatars.get(&identity).cloned()),
                }
            })
            .collect::<Vec<_>>();
        if self
            .channel
            .as_ref()
            .is_some_and(|current| current.id == channel.channel_id)
        {
            let (channel_id, filter, previous) = self
                .channel
                .as_ref()
                .map(|current| {
                    (
                        current.id,
                        current.roster_filter.clone(),
                        filtered_members(&current.members, &current.roster_filter),
                    )
                })
                .expect("same Remastered channel checked above");
            let next = filtered_members(&members, &filter);
            self.roster
                .begin_transition(channel_id, previous, &next, Instant::now(), |member| {
                    member.handle
                });
            let current = self
                .channel
                .as_mut()
                .expect("same Remastered channel checked above");
            current.title = title;
            current.members = members;
            let selected = self.roster.selection(&current.id);
            if selected.is_some_and(|selected| {
                !current
                    .members
                    .iter()
                    .any(|member| member.handle == selected)
            }) {
                self.roster.set_selection(current.id, None);
            }
            return;
        }
        self.roster_input.clear();
        self.roster.clear_interaction();
        self.roster.set_selection(channel.channel_id, None);
        self.roster
            .retain_selections(|channel_id| *channel_id == channel.channel_id);
        self.transcript.selection.clear();
        self.transcript.scroll.scroll_to_bottom();
        self.transcript_reveal = None;
        self.channel = Some(ScrChannelState {
            id: channel.channel_id,
            title: title.clone(),
            transcript: vec![ScrLine::SessionStart {
                time,
                channel: title,
            }],
            members,
            roster_filter: String::new(),
        });
    }

    pub(in crate::app::client) fn append_event(&mut self, event: &ClassicChatEvent, time: String) {
        let Some(channel_id) = self.channel.as_ref().map(|channel| channel.id) else {
            return;
        };
        if event
            .channel_id
            .is_some_and(|event_channel_id| event_channel_id != channel_id)
        {
            return;
        }
        let text = event.text.clone().unwrap_or_default();
        let line = match event.kind {
            ClassicEventKind::Talk | ClassicEventKind::Whisper | ClassicEventKind::Emote => {
                let Some(sender) = event.sender.clone() else {
                    return;
                };
                ScrLine::Message {
                    time,
                    kind: match event.kind {
                        ClassicEventKind::Talk => ScrMessageKind::Talk,
                        ClassicEventKind::Whisper => ScrMessageKind::Whisper,
                        ClassicEventKind::Emote => ScrMessageKind::Emote,
                        _ => unreachable!("matched message kinds above"),
                    },
                    sender,
                    text,
                }
            }
            ClassicEventKind::Broadcast | ClassicEventKind::Information => ScrLine::Notice {
                time,
                kind: if event.kind == ClassicEventKind::Broadcast {
                    ScrNoticeKind::Broadcast
                } else {
                    ScrNoticeKind::Information
                },
                text,
            },
            ClassicEventKind::Error => ScrLine::Error { time, text },
        };
        self.push_line(line);
    }

    pub(in crate::app::client) fn append_error(&mut self, time: String, text: String) {
        self.push_line(ScrLine::Error { time, text });
    }

    pub(in crate::app::client) fn selected_member(&self) -> Option<u32> {
        let channel = self.channel.as_ref()?;
        self.roster.selection(&channel.id)
    }

    pub(in crate::app::client) fn local_member(
        &self,
        account_battle_tag: Option<&str>,
    ) -> Option<&ScrMember> {
        let account_battle_tag = account_battle_tag?.trim();
        self.channel
            .as_ref()?
            .members
            .iter()
            .find(|member| member_matches_account(member, account_battle_tag))
    }

    pub(in crate::app::client) fn set_selected_member(&mut self, selected: Option<u32>) {
        let Some(channel) = self.channel.as_ref() else {
            return;
        };
        let selected = selected.filter(|selected| {
            channel
                .members
                .iter()
                .any(|member| member.handle == *selected)
        });
        self.roster.set_selection(channel.id, selected);
    }

    pub(in crate::app::client) fn set_roster_filter(&mut self, next: String) {
        if self
            .channel
            .as_ref()
            .is_none_or(|channel| channel.roster_filter == next)
        {
            return;
        }
        let (channel_id, previous, members) = self
            .channel
            .as_ref()
            .map(|channel| {
                (
                    channel.id,
                    filtered_members(&channel.members, &channel.roster_filter),
                    channel.members.clone(),
                )
            })
            .expect("Remastered channel checked above");
        let next_members = filtered_members(&members, &next);
        self.channel
            .as_mut()
            .expect("Remastered channel checked above")
            .roster_filter = next;
        let selected = self.roster.selection(&channel_id);
        if selected
            .is_some_and(|selected| !next_members.iter().any(|member| member.handle == selected))
        {
            self.roster.set_selection(channel_id, None);
        }
        self.roster.begin_transition(
            channel_id,
            previous,
            &next_members,
            Instant::now(),
            |member| member.handle,
        );
    }

    pub(in crate::app::client) fn visible_members(&self) -> Vec<&ScrMember> {
        let Some(channel) = self.channel.as_ref() else {
            return Vec::new();
        };
        let mut members = ui_roster_pattern::filtered_refs(
            &channel.members,
            &channel.roster_filter,
            |member, filter| ui_roster_pattern::filter_matches(&member.name, filter),
        );
        sort_member_refs(&mut members);
        members
    }

    pub(in crate::app::client) fn select_member_index(&mut self, index: usize) -> bool {
        let Some(channel_id) = self.channel.as_ref().map(|channel| channel.id) else {
            return false;
        };
        let handles = self
            .visible_members()
            .into_iter()
            .map(|member| member.handle)
            .collect::<Vec<_>>();
        let positions = (1..=handles.len()).collect::<Vec<_>>();
        self.roster
            .select_index(
                channel_id,
                &handles,
                &positions,
                index,
                ScrollStrategy::Center,
            )
            .is_some()
    }

    pub(in crate::app::client) fn move_member_selection(&mut self, delta: isize) -> bool {
        let Some(channel_id) = self.channel.as_ref().map(|channel| channel.id) else {
            return false;
        };
        let handles = self
            .visible_members()
            .into_iter()
            .map(|member| member.handle)
            .collect::<Vec<_>>();
        let positions = (1..=handles.len()).collect::<Vec<_>>();
        self.roster
            .move_selection(
                channel_id,
                &handles,
                &positions,
                delta,
                ScrollStrategy::Center,
            )
            .is_some()
    }

    #[must_use]
    pub(in crate::app::client) fn transcript_follows_bottom(&self) -> bool {
        let offset = -f32::from(self.transcript.scroll.offset().y);
        let maximum = f32::from(self.transcript.scroll.max_offset().y);
        maximum <= 6.0 || (maximum - offset).abs() <= 6.0
    }

    fn push_line(&mut self, line: ScrLine) {
        let follows_bottom = self.transcript_follows_bottom();
        let Some(channel) = self.channel.as_mut() else {
            return;
        };
        channel.transcript.push(line);
        if channel.transcript.len() > 2_000 {
            channel.transcript.drain(..500);
        }
        self.transcript_reveal = Some(ScrTranscriptReveal {
            channel_id: channel.id,
            line_index: channel.transcript.len().saturating_sub(1),
            started: Instant::now(),
        });
        if follows_bottom {
            self.transcript.scroll.scroll_to_bottom();
        }
    }
}

fn filtered_members(members: &[ScrMember], filter: &str) -> Vec<ScrMember> {
    let mut members = ui_roster_pattern::filtered_refs(members, filter, |member, normalized| {
        ui_roster_pattern::filter_matches(&member.name, normalized)
    })
    .into_iter()
    .cloned()
    .collect::<Vec<_>>();
    members.sort_by(scr_member_order);
    members
}

fn sort_member_refs(members: &mut [&ScrMember]) {
    members.sort_by(|left, right| scr_member_order(left, right));
}

fn scr_member_order(left: &ScrMember, right: &ScrMember) -> std::cmp::Ordering {
    ui_roster_pattern::present_before_absent(
        left.presence == ScrPresence::Away,
        &left.name,
        right.presence == ScrPresence::Away,
        &right.name,
    )
}

fn member_matches_account(member: &ScrMember, account_battle_tag: &str) -> bool {
    member
        .battle_tag
        .as_deref()
        .is_some_and(|tag| tag.trim().eq_ignore_ascii_case(account_battle_tag.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use superiority_core::games::scr::chat::ChatAttribute;

    fn member(attributes: &[(&str, &str)]) -> ClassicChatUser {
        ClassicChatUser {
            name: "Raynor".into(),
            flags: Some(0),
            is_operator: false,
            avatar: None,
            attributes: attributes
                .iter()
                .map(|(name, value)| ChatAttribute {
                    name: (*name).into(),
                    value: (*value).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn remastered_presence_comes_from_member_attributes() {
        assert_eq!(ScrPresence::from_member(&member(&[])), ScrPresence::Online);
        assert_eq!(
            ScrPresence::from_member(&member(&[("status", "away")])),
            ScrPresence::Away
        );
        assert_eq!(
            ScrPresence::from_member(&member(&[("presence", "DND")])),
            ScrPresence::Busy
        );
        assert_eq!(
            ScrPresence::from_member(&member(&[("game_info", "Brood War")])),
            ScrPresence::InGame
        );
        assert_eq!(
            ScrPresence::from_member(&member(&[("lobby_info", "1v1")])),
            ScrPresence::InLobby
        );
    }

    #[test]
    fn battle_tag_resolves_the_local_toon_without_assuming_the_names_match() {
        let local = ScrMember {
            handle: 7,
            name: "RaynorTheGreat".into(),
            battle_tag: Some("Commander#1234".into()),
            is_operator: false,
            presence: ScrPresence::Online,
            avatar_url: None,
        };
        assert!(member_matches_account(&local, "commander#1234"));
        assert!(!member_matches_account(&local, "RaynorTheGreat"));
    }

    #[test]
    fn remastered_roster_also_collects_away_members_at_the_bottom() {
        let make_member = |handle: u32, name: &str, presence: ScrPresence| ScrMember {
            handle,
            name: name.into(),
            battle_tag: None,
            is_operator: false,
            presence,
            avatar_url: None,
        };
        let members = vec![
            make_member(1, "Aaron", ScrPresence::Away),
            make_member(2, "Zulu", ScrPresence::Online),
            make_member(3, "Alpha", ScrPresence::Busy),
        ];
        assert_eq!(
            filtered_members(&members, "")
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Zulu", "Aaron"]
        );
    }
}
