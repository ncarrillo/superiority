use super::*;

mod actions;
mod invitations;
mod model;
mod view;

pub(in crate::app::client) use model::{
    InvitationKind, JoinRow, JoinSource, UiGroupSummary, UiInvitation,
};

const GROUP_SEARCH_MINIMUM: usize = 3;
const GROUP_SEARCH_DELAY: Duration = Duration::from_millis(350);
const COMMUNITY_RESULTS: usize = 5;
const INVITATION_WIDTH: f32 = 412.0;
const INVITATION_HEIGHT: f32 = 105.0;
const INVITATION_GAP: f32 = 12.0;
pub(in crate::app::client) const INVITATION_LIMIT: usize = 3;
const INVITATION_TRAVEL: f32 = 34.0;
const INVITATION_REVEAL_DURATION: Duration = Duration::from_millis(220);
pub(in crate::app::client) const INVITATION_CLOSE_DURATION: Duration = Duration::from_millis(160);

pub(super) struct JoinComponent {
    pub(super) join_focused: bool,
    pub(super) join_input: ui_text_input::TextInput,
    pub(super) join_query: String,
    pub(super) join_selected: usize,
    pub(super) join_scroll: ScrollHandle,
    pub(super) awaiting_joins: Vec<(ChatChannel, Instant)>,
    pub(super) group_search_due: Option<(Instant, String)>,
    pub(super) conference_channels: Vec<u32>,
    pub(super) groups: BTreeMap<u32, UiGroupSummary>,
    pub(super) remembered_group_names: BTreeMap<u32, String>,
    pub(super) member_groups: BTreeSet<u32>,
    pub(super) group_search: Vec<u32>,
    pub(super) invitations: Vec<UiInvitation>,
    pub(super) next_invitation_id: u64,
    pub(super) join_assets_warming: bool,
    pub(super) join_warmup_started: bool,
}

impl JoinComponent {
    pub(super) fn schedule_group_search(&mut self) {
        let query = self.join_query.trim().to_owned();
        if query.chars().count() < GROUP_SEARCH_MINIMUM {
            self.group_search_due = None;
            self.group_search.clear();
        } else {
            self.group_search_due = Some((Instant::now() + GROUP_SEARCH_DELAY, query));
        }
    }

    pub(super) fn flush_group_search_if_due(&mut self, commands: Option<&Sender<ClientCommand>>) {
        let Some((deadline, _)) = self.group_search_due.as_ref() else {
            return;
        };
        if Instant::now() < *deadline {
            return;
        }
        let Some((_, query)) = self.group_search_due.take() else {
            return;
        };
        if let Some(commands) = commands {
            let _ = commands.send(ClientCommand::SearchGroups { query });
        }
    }

    pub(super) fn typed_target(query: &str) -> ChatChannel {
        query.parse::<u16>().map_or_else(
            |_| ChatChannel::Private(query.to_owned()),
            ChatChannel::Public,
        )
    }

    pub(super) fn rows(&self, tabs: &[ChannelState]) -> Vec<JoinRow> {
        let query = self.join_query.trim();
        let needle = query.to_lowercase();
        let catalogue = self.catalogue(tabs, !query.is_empty());
        let mut rows = catalogue
            .iter()
            .filter(|row| {
                row.source == JoinSource::Community
                    || needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !query.is_empty() {
            let target = Self::typed_target(query);
            if !catalogue.iter().any(|row| row.target == target) {
                rows.insert(
                    0,
                    JoinRow {
                        name: query.to_owned(),
                        note: Some(
                            if matches!(target, ChatChannel::Public(_)) {
                                "public channel id"
                            } else {
                                "join by name"
                            }
                            .to_owned(),
                        ),
                        source: JoinSource::Typed,
                        target,
                        icon: "images/icons/channel.png",
                    },
                );
            }
        }
        rows
    }

    fn catalogue(&self, tabs: &[ChannelState], searching: bool) -> Vec<JoinRow> {
        let joined = tabs
            .iter()
            .filter_map(|tab| tab.channel.clone())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();

        if !searching {
            for club_id in &self.member_groups {
                let Some(group) = self.groups.get(club_id) else {
                    continue;
                };
                let target = ChatChannel::Club(*club_id);
                let Some(label) = group.label() else {
                    continue;
                };
                if !joined.contains(&target) {
                    rows.push(JoinRow {
                        name: group.name.clone(),
                        note: Some(label.to_owned()),
                        source: JoinSource::Group,
                        target,
                        icon: group.icon(),
                    });
                }
            }
        } else {
            let mut listed = 0;
            for club_id in &self.group_search {
                if listed == COMMUNITY_RESULTS {
                    break;
                }
                let target = ChatChannel::Club(*club_id);
                let Some(group) = self.groups.get(club_id) else {
                    continue;
                };
                if joined.contains(&target) {
                    continue;
                }
                rows.push(JoinRow {
                    name: group.name.clone(),
                    note: Some(if group.private {
                        "Private — needs an invitation".to_owned()
                    } else {
                        group.label().unwrap_or("Community").to_owned()
                    }),
                    source: JoinSource::Community,
                    target,
                    icon: group.icon(),
                });
                listed += 1;
            }
        }

        let mut public = self
            .conference_channels
            .iter()
            .filter_map(|identifier| u16::try_from(*identifier).ok())
            .map(ChatChannel::Public)
            .collect::<Vec<_>>();
        if public.is_empty() {
            public.push(ChatChannel::Public(DEFAULT_PUBLIC_CHANNEL));
        }
        for target in public {
            if joined.contains(&target) {
                continue;
            }
            let ChatChannel::Public(identifier) = target else {
                continue;
            };
            if identifier != DEFAULT_PUBLIC_CHANNEL && public_channel_name(identifier).is_none() {
                continue;
            }
            rows.push(JoinRow {
                name: channel_title(&target),
                note: None,
                source: JoinSource::Public,
                target,
                icon: "images/icons/channel.png",
            });
        }
        rows
    }
}
