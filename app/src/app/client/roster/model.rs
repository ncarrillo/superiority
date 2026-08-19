use super::*;

#[derive(Clone, PartialEq)]
pub(in crate::app::client) struct UiUser {
    pub(in crate::app::client) handle: u32,
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) presence_id: Option<u32>,
    pub(in crate::app::client) clan_tag: Option<String>,
    pub(in crate::app::client) presence: PresenceKind,
    pub(in crate::app::client) portrait: Option<Arc<RenderImage>>,
    pub(in crate::app::client) tone: RosterUserTone,
    pub(in crate::app::client) segment: RosterSegment,
    /// the tag on this row is your own clan's, so it reads in bright gold.
    /// true for your clanmates and for you.
    pub(in crate::app::client) own_clan: bool,
}

impl UiUser {
    pub(in crate::app::client) fn fixture(index: usize) -> Self {
        let user = preview::USERS[index];
        Self {
            handle: u32::try_from(index).unwrap_or(u32::MAX),
            name: user.display_name(),
            presence_id: None,
            clan_tag: user.clan_tag.map(ToOwned::to_owned),
            presence: match user.presence {
                preview::FixturePresence::Available => PresenceKind::Available,
                preview::FixturePresence::Away => PresenceKind::Away,
                preview::FixturePresence::Busy => PresenceKind::Busy,
                preview::FixturePresence::InGame => PresenceKind::InGame,
            },
            portrait: None,
            tone: RosterUserTone::Normal,
            segment: RosterSegment::Everyone,
            own_clan: false,
        }
    }

    /// the name with the clan tag lifted off. the tag rides in `name` because
    /// that is how the transcript says it, but anywhere the tag is drawn
    /// separately — or addressed rather than displayed — wants the person.
    pub(in crate::app::client) fn bare_name(&self) -> &str {
        self.clan_tag
            .as_ref()
            .and_then(|tag| self.name.strip_prefix(&format!("<{tag}> ")))
            .unwrap_or(&self.name)
    }

    pub(in crate::app::client) fn live(user: &ChatUser, portraits: &mut PortraitRegistry) -> Self {
        Self {
            handle: user.handle,
            name: user.visible_name(),
            presence_id: user.presence_id,
            clan_tag: user.clan_tag.clone(),
            presence: presence_kind(user.presence),
            portrait: user.avatar.and_then(|avatar| portraits.image(avatar)),
            tone: RosterUserTone::Normal,
            segment: RosterSegment::Everyone,
            own_clan: false,
        }
    }
}

pub(in crate::app::client) fn shared_roster_user(user: &UiUser, assets: &UiAssets) -> RosterUser {
    RosterUser {
        handle: user.handle,
        name: user.name.clone(),
        presence_id: user.presence_id,
        presence: user.presence,
        presence_label: user.presence.label().to_owned(),
        presence_icon: assets.presence_icon(user.presence),
        portrait: user
            .portrait
            .clone()
            .map(|portrait| Portrait::Image(portrait.into())),
        clan_tag: user.clan_tag.clone(),
        clan_is_local: user.own_clan,
        tone: user.tone,
        dimmed: user.presence.absent(),
    }
}

/// one line of the member list: either a band label or somebody standing in
/// that band. headers ride in the same list as the rows so virtualization,
/// scrolling, and the join/leave animation all see a single flat sequence.
#[derive(Clone)]
pub(in crate::app::client) enum RosterEntry {
    Segment {
        segment: RosterSegment,
        count: usize,
    },
    User(UiUser),
}

impl RosterEntry {
    /// identity for the row animation. headers borrow the top of the handle
    /// space, which the chat service never hands out.
    pub(in crate::app::client) const fn handle(&self) -> u32 {
        match self {
            Self::Segment { segment, .. } => u32::MAX - segment.rank() as u32,
            Self::User(user) => user.handle,
        }
    }

    pub(in crate::app::client) const fn user(&self) -> Option<&UiUser> {
        match self {
            Self::Segment { .. } => None,
            Self::User(user) => Some(user),
        }
    }
}

#[derive(Default)]
struct RosterMembership {
    local_clan_tags: BTreeSet<String>,
    local_presence_ids: BTreeSet<u32>,
    local_names: BTreeSet<String>,
    party_presence_ids: BTreeSet<u32>,
    party_names: BTreeSet<String>,
}

impl RosterMembership {
    fn from_channels(channels: &[ChannelState]) -> Self {
        let mut membership = Self::default();
        for channel in channels {
            if let Some(local_handle) = channel.local_member_handle
                && let Some(local) = channel
                    .users
                    .iter()
                    .find(|user| user.handle == local_handle)
            {
                if let Some(presence_id) = local.presence_id {
                    membership.local_presence_ids.insert(presence_id);
                }
                membership.local_names.insert(identity_name(&local.name));
                if let Some(clan_tag) = &local.clan_tag {
                    membership.local_clan_tags.insert(clan_tag.to_lowercase());
                }
            }
            if channel.channel != Some(ChatChannel::Party) {
                continue;
            }
            for user in &channel.users {
                if let Some(presence_id) = user.presence_id {
                    membership.party_presence_ids.insert(presence_id);
                }
                membership.party_names.insert(identity_name(&user.name));
            }
        }
        membership
    }

    fn is_local(&self, user: &UiUser) -> bool {
        contains_user(&self.local_presence_ids, &self.local_names, user)
    }

    fn in_party(&self, user: &UiUser) -> bool {
        contains_user(&self.party_presence_ids, &self.party_names, user)
    }

    fn shares_clan(&self, user: &UiUser) -> bool {
        !self.is_local(user) && self.wears_local_clan(user)
    }

    fn wears_local_clan(&self, user: &UiUser) -> bool {
        user.clan_tag
            .as_ref()
            .is_some_and(|clan_tag| self.local_clan_tags.contains(&clan_tag.to_lowercase()))
    }
}

/// who a person is to you, resolved once and asked many times. clan tags and
/// avatars arrive after the join event that announced someone, so transcript
/// events resolve this at render rather than trusting what they were born with.
pub(in crate::app::client) struct RosterAffinity {
    membership: RosterMembership,
    friends: BTreeSet<String>,
}

impl RosterAffinity {
    pub(in crate::app::client) fn new(channels: &[ChannelState], friends: &[UiFriend]) -> Self {
        Self {
            membership: RosterMembership::from_channels(channels),
            friends: friends
                .iter()
                .map(|friend| identity_name(&friend.name))
                .collect(),
        }
    }

    pub(in crate::app::client) fn tone(&self, user: &UiUser) -> RosterUserTone {
        RosterPresentation::resolve(RosterRelationship {
            shared_clan: self.membership.shares_clan(user),
            own_clan: self.membership.wears_local_clan(user),
            shared_party: self.membership.in_party(user),
            friend: self.is_friend(user),
            away: user.presence == PresenceKind::Away,
        })
        .tone
    }

    pub(in crate::app::client) fn is_friend(&self, user: &UiUser) -> bool {
        self.friends.contains(&identity_name(&user.name))
    }

    /// the arrivals worth interrupting a digest for.
    pub(in crate::app::client) fn notable(&self, user: &UiUser) -> bool {
        self.tone(user) == RosterUserTone::Clan || self.is_friend(user)
    }
}

pub(in crate::app::client) fn presented_roster_users(
    channels: &[ChannelState],
    friends: &[UiFriend],
    channel: &ChannelState,
    filter: &str,
) -> Vec<UiUser> {
    let membership = RosterMembership::from_channels(channels);
    let friends = friends
        .iter()
        .map(|friend| identity_name(&friend.name))
        .collect::<BTreeSet<_>>();
    let kind = match channel.channel {
        Some(ChatChannel::Club(_)) => RosterChannelKind::Group,
        Some(ChatChannel::Party) => RosterChannelKind::Party,
        _ => RosterChannelKind::Standard,
    };
    let mut users = filtered_roster_users(&channel.users, filter)
        .into_iter()
        .cloned()
        .map(|mut user| {
            let own_clan = membership.wears_local_clan(&user);
            let presentation = RosterPresentation::resolve(RosterRelationship {
                shared_clan: membership.shares_clan(&user),
                own_clan,
                shared_party: membership.in_party(&user),
                friend: friends.contains(&identity_name(&user.name)),
                away: user.presence == PresenceKind::Away,
            });
            user.tone = presentation.tone;
            user.segment = presentation.segment;
            user.own_clan = own_clan;
            (user, presentation)
        })
        .collect::<Vec<_>>();
    if kind == RosterChannelKind::Party {
        // a party is small and its order is the order the game gave us;
        // banding three people into segments would say nothing.
        for (user, _) in &mut users {
            user.segment = RosterSegment::Party;
        }
        return users.into_iter().map(|(user, _)| user).collect();
    }
    // band first, present-before-away second, then alphabetical — so a name is
    // findable by eye without the panel reshuffling every presence change.
    // identity_name drops the clan tag, so `<AAA>` cannot drag a whole clan to
    // the top of a band it is already sorted into.
    users.sort_by(|(left, left_rank), (right, right_rank)| {
        left_rank
            .rank
            .cmp(&right_rank.rank)
            .then_with(|| identity_name(&left.name).cmp(&identity_name(&right.name)))
    });
    users.into_iter().map(|(user, _)| user).collect()
}

/// whether the member list draws band headers at all. a room where everybody
/// is a stranger is a single band, and labelling it "EVERYONE" says nothing.
fn segmented(channel: &ChannelState, users: &[UiUser]) -> bool {
    if channel.channel == Some(ChatChannel::Party) {
        return false;
    }
    users
        .first()
        .is_some_and(|first| users.iter().any(|user| user.segment != first.segment))
}

pub(in crate::app::client) fn presented_roster_entries(
    channels: &[ChannelState],
    friends: &[UiFriend],
    channel: &ChannelState,
    filter: &str,
) -> Vec<RosterEntry> {
    let users = presented_roster_users(channels, friends, channel, filter);
    if !segmented(channel, &users) {
        return users.into_iter().map(RosterEntry::User).collect();
    }
    let mut counts = BTreeMap::<RosterSegment, usize>::new();
    for user in &users {
        *counts.entry(user.segment).or_default() += 1;
    }
    let mut entries = Vec::with_capacity(users.len() + counts.len());
    let mut open = None;
    for user in users {
        if open != Some(user.segment) {
            open = Some(user.segment);
            entries.push(RosterEntry::Segment {
                segment: user.segment,
                count: counts.get(&user.segment).copied().unwrap_or_default(),
            });
        }
        entries.push(RosterEntry::User(user));
    }
    entries
}

pub(in crate::app::client) fn presented_roster_entry_count(
    channels: &[ChannelState],
    friends: &[UiFriend],
    channel: &ChannelState,
    filter: &str,
) -> usize {
    presented_roster_entries(channels, friends, channel, filter).len()
}

pub(in crate::app::client) fn presented_roster_range(
    channels: &[ChannelState],
    friends: &[UiFriend],
    channel: &ChannelState,
    filter: &str,
    range: Range<usize>,
) -> Vec<RosterEntry> {
    presented_roster_entries(channels, friends, channel, filter)
        .into_iter()
        .skip(range.start)
        .take(range.len())
        .collect()
}

fn contains_user(presence_ids: &BTreeSet<u32>, names: &BTreeSet<String>, user: &UiUser) -> bool {
    user.presence_id.map_or_else(
        || names.contains(&identity_name(&user.name)),
        |presence_id| presence_ids.contains(&presence_id),
    )
}

fn identity_name(name: &str) -> String {
    name.rsplit_once("> ")
        .map_or(name, |(_, name)| name)
        .to_lowercase()
}

pub(in crate::app::client) fn presence_kind(state: PresenceState) -> PresenceKind {
    match state {
        PresenceState::Available => PresenceKind::Available,
        PresenceState::Away => PresenceKind::Away,
        PresenceState::Busy => PresenceKind::Busy,
        PresenceState::InGame => PresenceKind::InGame,
        PresenceState::Offline => PresenceKind::Offline,
        PresenceState::Unknown => PresenceKind::Unknown,
    }
}

pub(in crate::app::client) fn filtered_roster_users<'a>(
    users: &'a [UiUser],
    filter: &str,
) -> Vec<&'a UiUser> {
    ui_roster::filtered_refs(users, filter, |user, filter| {
        ui_roster::filter_matches(&user.name, filter)
    })
}

pub(in crate::app::client) fn filtered_roster_count(users: &[UiUser], filter: &str) -> usize {
    ui_roster::filtered_count(users, filter, |user, filter| {
        ui_roster::filter_matches(&user.name, filter)
    })
}
