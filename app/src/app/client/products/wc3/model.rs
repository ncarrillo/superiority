use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::client) enum Wc3Presence {
    Offline,
    Online,
    Away,
    Busy,
}

impl From<WarcraftChatPresence> for Wc3Presence {
    fn from(presence: WarcraftChatPresence) -> Self {
        match presence {
            WarcraftChatPresence::Offline => Self::Offline,
            WarcraftChatPresence::Online => Self::Online,
            WarcraftChatPresence::Away => Self::Away,
            WarcraftChatPresence::Busy => Self::Busy,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::client) struct Wc3Member {
    pub(in crate::app::client) handle: u64,
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) presence: Wc3Presence,
    pub(in crate::app::client) portrait: String,
    pub(in crate::app::client) clan_abbreviation: Option<String>,
}

impl From<&WarcraftChatMember> for Wc3Member {
    fn from(member: &WarcraftChatMember) -> Self {
        Self {
            handle: member.handle,
            name: member.name.clone(),
            presence: member.presence.into(),
            portrait: avatar::source(member.avatar_id.as_deref()),
            clan_abbreviation: member.clan_abbreviation.clone(),
        }
    }
}

impl Wc3Member {
    /// WC3's callbacks identify an account as `Name#1234`. The discriminator
    /// remains on `name` for filtering and stable protocol identity, while the
    /// member list reads like the in-game roster.
    pub(in crate::app::client) fn visible_name(&self) -> &str {
        strip_character_code(&self.name)
    }

    const fn absent(&self) -> bool {
        matches!(self.presence, Wc3Presence::Away | Wc3Presence::Offline)
    }
}

pub(in crate::app::client) struct Wc3ChannelState {
    pub(in crate::app::client) id: u8,
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) members: Vec<Wc3Member>,
    pub(in crate::app::client) transcript: Vec<Wc3TranscriptLine>,
    pub(in crate::app::client) roster_filter: String,
    pub(in crate::app::client) unread: bool,
}

pub(in crate::app::client) struct Wc3SessionUi {
    pub(in crate::app::client) composer: ui_text_input::TextInput,
    pub(in crate::app::client) composer_focused: bool,
    pub(in crate::app::client) roster_input: ui_text_input::TextInput,
    pub(in crate::app::client) transcript: ui_wc3_chat::TranscriptState,
    pub(in crate::app::client) roster:
        ui_workspace_pattern::RosterState<String, Wc3Member, Instant, u64>,
    /// the realm directory advertised by the gateway. `/join` resolves a name
    /// from this authoritative list; joined rooms then receive stable local
    /// ids from the WC3 session and live independently in `channels`.
    pub(in crate::app::client) public_channels: Vec<String>,
    pub(in crate::app::client) channels: Vec<Wc3ChannelState>,
    pub(in crate::app::client) active_channel: usize,
    /// the strip's own motion — drag reorder and the long-name marquee —
    /// keyed by channel id, the way StarCraft II's is keyed by tab id.
    pub(in crate::app::client) navigation: ui_workspace::NavigationState<u64, Instant>,
    pub(in crate::app::client) hovered_tab: Option<u64>,
    pub(in crate::app::client) tab_selection_started: Option<Instant>,
    pub(in crate::app::client) tab_close: Option<Wc3TabClose>,
    pub(in crate::app::client) clan: WarcraftClanSnapshot,
    pub(in crate::app::client) clan_scroll: ScrollHandle,
}

/// a tab on its way out: the strip folds it over `TAB_CLOSE_DURATION`, then
/// the hall is left.
pub(in crate::app::client) struct Wc3TabClose {
    pub(in crate::app::client) index: usize,
    pub(in crate::app::client) started: Option<Instant>,
}

impl Wc3SessionUi {
    pub(in crate::app::client) fn new(
        composer: ui_text_input::TextInput,
        roster_input: ui_text_input::TextInput,
    ) -> Self {
        Self {
            composer,
            composer_focused: false,
            roster_input,
            transcript: ui_wc3_chat::TranscriptState::default(),
            roster: ui_workspace_pattern::RosterState::default(),
            public_channels: Vec::new(),
            channels: Vec::new(),
            active_channel: 0,
            navigation: ui_workspace::NavigationState::default(),
            hovered_tab: None,
            tab_selection_started: None,
            tab_close: None,
            clan: WarcraftClanSnapshot::default(),
            clan_scroll: ScrollHandle::new(),
        }
    }

    pub(in crate::app::client) fn clear(&mut self) {
        self.composer.clear();
        self.roster_input.clear();
        self.composer_focused = false;
        self.roster.clear_interaction();
        self.roster.selections.clear();
        self.transcript.selection.clear();
        self.public_channels.clear();
        self.channels.clear();
        self.active_channel = 0;
        self.navigation = ui_workspace::NavigationState::default();
        self.hovered_tab = None;
        self.tab_selection_started = None;
        self.tab_close = None;
        self.clan = WarcraftClanSnapshot::default();
    }

    pub(in crate::app::client) fn set_public_channels(&mut self, mut channels: Vec<String>) {
        channels.sort_unstable_by_key(|name| name.to_ascii_lowercase());
        channels.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        self.public_channels = channels;
    }

    pub(in crate::app::client) fn apply_channel(
        &mut self,
        channel: &WarcraftChatChannel,
        time: String,
    ) {
        let members = channel
            .members
            .iter()
            .map(Wc3Member::from)
            .collect::<Vec<_>>();
        let scope = roster_scope(channel.id);
        let selected = self
            .roster
            .selection(&scope)
            .filter(|selected| members.iter().any(|member| member.handle == *selected));
        self.roster.set_selection(scope.clone(), selected);
        if let Some(index) = self
            .channels
            .iter()
            .position(|current| current.id == channel.id)
        {
            // only the channel in front is being watched: a change behind
            // another tab lands without a transition of its own, and must not
            // cut short one the front channel is running
            let previous = (index == self.active_channel).then(|| {
                visible_members_in(&self.channels[index])
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            });
            let current = &mut self.channels[index];
            current.name.clone_from(&channel.name);
            current.members = members;
            if let Some(previous) = previous {
                self.begin_roster_transition(previous);
            }
            return;
        }
        let previous = self.visible_snapshot();
        self.channels.push(Wc3ChannelState {
            id: channel.id,
            name: channel.name.clone(),
            members,
            transcript: vec![Wc3TranscriptLine::SessionStart {
                time,
                channel: channel.name.clone(),
                online: channel.members.len(),
            }],
            roster_filter: String::new(),
            unread: false,
        });
        self.active_channel = self.channels.len() - 1;
        self.transcript.selection.clear();
        self.transcript.scroll.scroll_to_bottom();
        self.begin_roster_transition(previous);
    }

    pub(in crate::app::client) fn append_event(&mut self, event: &WarcraftChatEvent, time: String) {
        match event {
            WarcraftChatEvent::ChannelJoined {
                channel_id,
                name,
                member_count,
            } => {
                let previous = self.visible_snapshot();
                if let Some(index) = self
                    .channels
                    .iter()
                    .position(|channel| channel.id == *channel_id)
                {
                    self.active_channel = index;
                    self.channels[index].name.clone_from(name);
                    self.channels[index].unread = false;
                } else {
                    self.transcript.selection.clear();
                    self.channels.push(Wc3ChannelState {
                        id: *channel_id,
                        name: name.clone(),
                        members: Vec::new(),
                        transcript: vec![Wc3TranscriptLine::SessionStart {
                            time,
                            channel: name.clone(),
                            online: *member_count,
                        }],
                        roster_filter: String::new(),
                        unread: false,
                    });
                    self.active_channel = self.channels.len() - 1;
                }
                self.begin_roster_transition(previous);
            }
            WarcraftChatEvent::ChannelLeft { channel_id } => {
                if let Some(index) = self
                    .channels
                    .iter()
                    .position(|channel| channel.id == *channel_id)
                {
                    let _ = self.remove_channel(index);
                }
            }
            WarcraftChatEvent::MemberJoined { channel_id, name } => {
                if let Some(channel) = self.channel_mut(*channel_id) {
                    channel.transcript.push(Wc3TranscriptLine::Notice {
                        time,
                        text: format!("{} entered the channel.", strip_character_code(name)),
                    });
                }
            }
            WarcraftChatEvent::MemberLeft { channel_id, name } => {
                if let Some(channel) = self.channel_mut(*channel_id) {
                    let text = name.as_ref().map_or_else(
                        || "A player left the channel.".into(),
                        |name| format!("{} left the channel.", strip_character_code(name)),
                    );
                    channel
                        .transcript
                        .push(Wc3TranscriptLine::Notice { time, text });
                }
            }
            WarcraftChatEvent::Message { channel_id, body } => {
                let inactive = self
                    .active()
                    .is_none_or(|channel| channel.id != *channel_id);
                if let Some(channel) = self.channel_mut(*channel_id) {
                    channel.transcript.push(Wc3TranscriptLine::Message {
                        time,
                        sender: None,
                        text: body.clone(),
                    });
                    channel.unread |= inactive;
                }
            }
            WarcraftChatEvent::Whisper { .. } => {}
            WarcraftChatEvent::Notice { text } => {
                if let Some(channel) = self.active_mut() {
                    channel.transcript.push(Wc3TranscriptLine::Notice {
                        time,
                        text: text.clone(),
                    });
                }
            }
        }
        self.transcript.scroll.scroll_to_bottom();
    }

    pub(in crate::app::client) fn append_error(&mut self, time: String, text: String) {
        if let Some(channel) = self.active_mut() {
            channel
                .transcript
                .push(Wc3TranscriptLine::Error { time, text });
            self.transcript.scroll.scroll_to_bottom();
        }
    }

    pub(in crate::app::client) fn set_roster_filter(&mut self, filter: String) {
        let previous = self.visible_snapshot();
        if let Some(channel) = self.active_mut() {
            channel.roster_filter = filter;
        }
        if self.selected_member().is_some_and(|selected| {
            !self
                .visible_members()
                .iter()
                .any(|member| member.handle == selected)
        }) {
            self.set_selected_member(None);
        }
        self.begin_roster_transition(previous);
    }

    pub(in crate::app::client) fn set_selected_member(&mut self, selected: Option<u64>) {
        let selected = selected.filter(|selected| {
            self.active().is_some_and(|channel| {
                channel
                    .members
                    .iter()
                    .any(|member| member.handle == *selected)
            })
        });
        if let Some(channel) = self.active() {
            self.roster
                .set_selection(roster_scope(channel.id), selected);
        }
    }

    pub(in crate::app::client) fn selected_member(&self) -> Option<u64> {
        let channel = self.active()?;
        self.roster.selection(&roster_scope(channel.id))
    }

    pub(in crate::app::client) fn visible_members(&self) -> Vec<&Wc3Member> {
        self.active().map(visible_members_in).unwrap_or_default()
    }

    /// what the roster is showing right now, cloned, so a change can be
    /// animated from it.
    fn visible_snapshot(&self) -> Vec<Wc3Member> {
        self.visible_members().into_iter().cloned().collect()
    }

    /// animates the roster from `previous` — the rows it showed before a
    /// change — to what the channel in front shows now: rows that left fold
    /// away, rows that arrived grow in, and a wholesale change reveals. the
    /// same mechanics the other two realms' rosters use; until this the
    /// Reforged list simply snapped.
    fn begin_roster_transition(&mut self, previous: Vec<Wc3Member>) {
        let Some(channel) = self.active() else {
            self.roster.animation = None;
            return;
        };
        let scope = roster_scope(channel.id);
        let next = visible_members_in(channel)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        self.roster
            .begin_transition(scope, previous, &next, Instant::now(), |member| {
                member.handle
            });
    }

    /// the roster's running transition, if it belongs to the channel in front.
    pub(in crate::app::client) fn roster_animation(
        &self,
        now: Instant,
    ) -> Option<&ui_roster_pattern::TimedTransition<String, Wc3Member, Instant, u64>> {
        let channel = self.active()?;
        self.roster.animation.as_ref().filter(|animation| {
            animation.scope == roster_scope(channel.id) && animation.is_running(now)
        })
    }

    pub(in crate::app::client) fn active(&self) -> Option<&Wc3ChannelState> {
        self.channels.get(self.active_channel)
    }

    pub(in crate::app::client) fn active_mut(&mut self) -> Option<&mut Wc3ChannelState> {
        self.channels.get_mut(self.active_channel)
    }

    pub(in crate::app::client) fn channel_mut(&mut self, id: u8) -> Option<&mut Wc3ChannelState> {
        self.channels.iter_mut().find(|channel| channel.id == id)
    }

    pub(in crate::app::client) fn select_channel(&mut self, index: usize) -> bool {
        if index >= self.channels.len() || index == self.active_channel {
            return false;
        }
        let previous = self.visible_snapshot();
        self.active_channel = index;
        self.channels[index].unread = false;
        self.roster_input
            .set_content(self.channels[index].roster_filter.clone());
        self.transcript.selection.clear();
        self.transcript.scroll.scroll_to_bottom();
        self.begin_roster_transition(previous);
        true
    }

    pub(in crate::app::client) fn remove_channel(&mut self, index: usize) -> Option<u8> {
        let id = self.channels.get(index)?.id;
        let previous = self.visible_snapshot();
        self.channels.remove(index);
        self.roster.selections.remove(&roster_scope(id));
        if self.channels.is_empty() {
            self.active_channel = 0;
            self.roster_input.clear();
        } else if self.active_channel > index || self.active_channel == self.channels.len() {
            self.active_channel = self.active_channel.saturating_sub(1);
        }
        let filter = self
            .active()
            .map_or_else(String::new, |channel| channel.roster_filter.clone());
        self.roster_input.set_content(filter);
        self.transcript.selection.clear();
        self.transcript.scroll.scroll_to_bottom();
        self.begin_roster_transition(previous);
        Some(id)
    }
}

/// the rows one channel shows: its members through its filter, present people
/// first.
fn visible_members_in(channel: &Wc3ChannelState) -> Vec<&Wc3Member> {
    let mut members = ui_roster_pattern::filtered_refs(
        &channel.members,
        &channel.roster_filter,
        |member, query| {
            ui_roster_pattern::filter_matches(&member.name, query)
                || member
                    .clan_abbreviation
                    .as_ref()
                    .is_some_and(|clan| ui_roster_pattern::filter_matches(clan, query))
        },
    );
    members.sort_by(|left, right| {
        ui_roster_pattern::present_before_absent(
            left.absent(),
            left.visible_name(),
            right.absent(),
            right.visible_name(),
        )
    });
    members
}

fn roster_scope(channel_id: u8) -> String {
    format!("wc3:{channel_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, presence: Wc3Presence) -> Wc3Member {
        Wc3Member {
            handle: 1,
            name: name.into(),
            presence,
            portrait: "wc3://portrait/p003".into(),
            clan_abbreviation: None,
        }
    }

    #[test]
    fn visible_name_hides_only_a_numeric_battletag_discriminator() {
        assert_eq!(
            member("NelsonTest91#1458", Wc3Presence::Online).visible_name(),
            "NelsonTest91"
        );
        assert_eq!(
            member("A#Team", Wc3Presence::Online).visible_name(),
            "A#Team"
        );
    }

    #[test]
    fn roster_order_keeps_present_people_before_absent_people() {
        let mut members = [
            member("Aaron#1000", Wc3Presence::Away),
            member("Zulu#1000", Wc3Presence::Online),
            member("Beta#1000", Wc3Presence::Offline),
            member("Alpha#1000", Wc3Presence::Busy),
        ];
        members.sort_by(|left, right| {
            ui_roster_pattern::present_before_absent(
                left.absent(),
                left.visible_name(),
                right.absent(),
                right.visible_name(),
            )
        });
        assert_eq!(
            members
                .iter()
                .map(Wc3Member::visible_name)
                .collect::<Vec<_>>(),
            vec!["Alpha", "Zulu", "Aaron", "Beta"]
        );
    }
}
