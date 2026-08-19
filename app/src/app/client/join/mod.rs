use super::*;

mod actions;
mod invitations;
mod model;

pub(in crate::app::client) use model::{
    InvitationKind, JoinRow, JoinSource, UiGroupSummary, UiInvitation, count_color,
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
    pub(super) awaiting_joins: Vec<(ChatChannel, Instant)>,
    pub(super) group_search_due: Option<(Instant, String)>,
    pub(super) public_channels: BTreeMap<u16, String>,
    pub(super) groups: BTreeMap<u32, UiGroupSummary>,
    pub(super) remembered_group_names: BTreeMap<u32, String>,
    pub(super) member_groups: BTreeSet<u32>,
    pub(super) group_search: Vec<u32>,
    pub(super) invitations: Vec<UiInvitation>,
    pub(super) next_invitation_id: u64,
    /// the conferences serving each catalogue channel, in the locale we join
    /// with. a channel the directory never mentions has no room behind it — it
    /// is in the name table but cannot be entered — so it is dropped from the
    /// list rather than offered, and a channel that is listed sums the live
    /// population of every conference serving it.
    pub(super) channel_conferences: BTreeMap<u16, Vec<u32>>,
    /// live head count per conference, from the most recent counts page.
    pub(super) conference_members: BTreeMap<u32, u16>,
    /// set once a complete directory has arrived. until then nothing is pruned,
    /// because an empty set would mean an empty list.
    pub(super) directory_complete: bool,
}

impl JoinComponent {
    pub(super) fn schedule_group_search(&mut self, query: &str) {
        let query = query.trim().to_owned();
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

    fn fallback_typed_target(query: &str) -> ChatChannel {
        query.parse::<u16>().map_or_else(
            |_| ChatChannel::Private(query.to_owned()),
            ChatChannel::Public,
        )
    }

    pub(super) fn target_for_query(&self, query: &str) -> ChatChannel {
        target_for_query(query, &self.public_channels)
    }

    /// what `/join <query>` can offer from the composer: the matching rooms,
    /// capped because the popup is a fast path rather than a browser, and the
    /// room the typed name would create when the catalogue holds nothing under
    /// it. both come off one catalogue walk, since the create offer is decided
    /// by the whole catalogue and not by the trimmed result list.
    pub(in crate::app::client) fn command_matches(
        &self,
        tabs: &[ChannelState],
        query: &str,
        limit: usize,
    ) -> (Vec<JoinRow>, Option<ChatChannel>) {
        let query = query.trim();
        let catalogue = self.catalogue(tabs, !query.is_empty());
        let mut rows = ranked(&catalogue, query);
        rows.truncate(limit);
        let create = (!query.is_empty())
            .then(|| self.target_for_query(query))
            .filter(|target| !catalogue.iter().any(|row| row.target == *target));
        (rows, create)
    }

    /// how many people are in a channel right now — the sum over the
    /// conferences serving it, since a busy channel is spread across several.
    fn channel_population(&self, name_id: u16) -> Option<usize> {
        let conferences = self.channel_conferences.get(&name_id)?;
        Some(
            conferences
                .iter()
                .filter_map(|conference| self.conference_members.get(conference))
                .map(|members| usize::from(*members))
                .sum(),
        )
    }

    fn catalogue(&self, tabs: &[ChannelState], searching: bool) -> Vec<JoinRow> {
        // rooms you are already in are not offered at all, so membership is
        // only ever a reason to drop a row.
        let joined = tabs
            .iter()
            .filter_map(|tab| tab.channel.clone())
            .collect::<Vec<_>>();
        // outside a group, the club-info lookup reports who is online; the
        // roster total is the fallback until it answers.
        let group_count = |group: &UiGroupSummary| {
            group
                .online
                .or(group.member_count)
                .map(|count| count as usize)
        };
        let mut rows = Vec::new();

        // your own clans and groups are offered whether or not you are
        // searching. typing the first letters of a clan you are already in is
        // the fastest way back into it, and dropping those rows the moment you
        // type would take exactly that away.
        for club_id in &self.member_groups {
            let Some(group) = self.groups.get(club_id) else {
                continue;
            };
            let target = ChatChannel::Club(*club_id);
            let Some(label) = group.label() else {
                continue;
            };
            if joined.contains(&target) {
                continue;
            }
            rows.push(JoinRow {
                name: group.name.clone(),
                note: Some(label.to_owned()),
                source: JoinSource::Group,
                target,
                icon: group.icon(),
                count: group_count(group),
            });
        }

        if searching {
            let mut listed = 0;
            for club_id in &self.group_search {
                if listed == COMMUNITY_RESULTS {
                    break;
                }
                // a group you belong to is already above, under your own rows
                if self.member_groups.contains(club_id) {
                    continue;
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
                        "Private".to_owned()
                    } else {
                        group.label().unwrap_or("Community").to_owned()
                    }),
                    source: JoinSource::Community,
                    target,
                    icon: group.icon(),
                    count: group_count(group),
                });
                listed += 1;
            }
        }

        let public = self
            .public_channels
            .iter()
            .map(|(identifier, name)| (ChatChannel::Public(*identifier), name.clone()))
            .collect::<Vec<_>>();
        for (target, name) in public {
            if joined.contains(&target)
                || !offers_channel(self.directory_complete, &self.channel_conferences, &target)
            {
                continue;
            }
            let ChatChannel::Public(name_id) = target else {
                continue;
            };
            rows.push(JoinRow {
                name,
                note: None,
                source: JoinSource::Public,
                target,
                icon: "images/icons/channel.png",
                count: self.channel_population(name_id),
            });
        }
        rows
    }
}

/// one flat list: groups first, then channels by how busy they are, so the
/// rooms worth entering rise to the top without needing headers. community
/// results are already a search answer, so the needle does not filter them
/// again.
pub(in crate::app::client) fn ranked(catalogue: &[JoinRow], query: &str) -> Vec<JoinRow> {
    let needle = query.trim().to_lowercase();
    let mut rows = catalogue
        .iter()
        .filter(|row| {
            row.source == JoinSource::Community
                || needle.is_empty()
                || row.name.to_lowercase().contains(&needle)
        })
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.source
            .rank()
            .cmp(&right.source.rank())
            // an empty room sinks under every room with somebody in it, and
            // under the ones we simply have no count for
            .then_with(|| left.dead().cmp(&right.dead()))
            .then_with(|| right.count.unwrap_or(0).cmp(&left.count.unwrap_or(0)))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    rows
}

/// whether a catalogued channel is worth offering.
///
/// a public channel with no conference behind it is a name in the table with
/// no room to enter — joining it does nothing — so once the directory has
/// arrived complete, those are dropped. before it arrives nothing is pruned,
/// since an empty set would empty the whole list.
pub(in crate::app::client) fn offers_channel(
    directory_complete: bool,
    channel_conferences: &BTreeMap<u16, Vec<u32>>,
    target: &ChatChannel,
) -> bool {
    let ChatChannel::Public(name_id) = target else {
        return true;
    };
    !directory_complete || channel_conferences.contains_key(name_id)
}

pub(in crate::app::client) fn target_for_query(
    query: &str,
    public_channels: &BTreeMap<u16, String>,
) -> ChatChannel {
    public_channels
        .iter()
        .find_map(|(identifier, name)| {
            name.eq_ignore_ascii_case(query)
                .then_some(ChatChannel::Public(*identifier))
        })
        .unwrap_or_else(|| JoinComponent::fallback_typed_target(query))
}
