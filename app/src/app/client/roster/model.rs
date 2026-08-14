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
    pub(in crate::app::client) segment_start: bool,
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
            segment_start: false,
        }
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
            segment_start: false,
        }
    }
}

pub(in crate::app::client) fn shared_roster_user(user: &UiUser, assets: &UiAssets) -> RosterUser {
    RosterUser {
        handle: user.handle,
        name: user.name.clone(),
        presence_id: user.presence_id,
        presence_label: user.presence.label().to_owned(),
        presence_icon: assets.presence_icon(user.presence),
        portrait: user
            .portrait
            .clone()
            .map(|portrait| Portrait::Image(portrait.into())),
        tone: user.tone,
        dimmed: user.presence == PresenceKind::Away,
        segment_start: user.segment_start,
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
        !self.is_local(user)
            && user
                .clan_tag
                .as_ref()
                .is_some_and(|clan_tag| self.local_clan_tags.contains(&clan_tag.to_lowercase()))
    }
}

pub(in crate::app::client) fn presented_roster_users(
    channels: &[ChannelState],
    channel: &ChannelState,
    filter: &str,
) -> Vec<UiUser> {
    let membership = RosterMembership::from_channels(channels);
    let kind = match channel.channel {
        Some(ChatChannel::Club(_)) => RosterChannelKind::Group,
        Some(ChatChannel::Party) => RosterChannelKind::Party,
        _ => RosterChannelKind::Standard,
    };
    let mut users = filtered_roster_users(&channel.users, filter)
        .into_iter()
        .cloned()
        .map(|mut user| {
            let shares_clan = membership.shares_clan(&user);
            let shared_party = membership.in_party(&user);
            let presentation = RosterPresentation::resolve(RosterRelationship {
                shared_clan: shares_clan,
                shared_party,
                away: user.presence == PresenceKind::Away,
            });
            user.tone = presentation.tone;
            (user, presentation)
        })
        .collect::<Vec<_>>();
    if kind != RosterChannelKind::Party {
        users.sort_by_key(|(_, presentation)| presentation.rank);
        let mut previous = None;
        for (user, presentation) in &mut users {
            user.segment_start = previous.is_some_and(|previous| previous != presentation.rank);
            previous = Some(presentation.rank);
        }
    } else {
        for (user, _) in &mut users {
            user.segment_start = false;
        }
    }
    users.into_iter().map(|(user, _)| user).collect()
}

pub(in crate::app::client) fn presented_roster_range(
    channels: &[ChannelState],
    channel: &ChannelState,
    filter: &str,
    range: Range<usize>,
) -> Vec<UiUser> {
    presented_roster_users(channels, channel, filter)
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
