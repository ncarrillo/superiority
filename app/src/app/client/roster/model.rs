use super::*;

#[derive(Clone, PartialEq)]
pub(in crate::app::client) struct UiUser {
    pub(in crate::app::client) handle: u32,
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) presence_id: Option<u32>,
    pub(in crate::app::client) presence: PresenceKind,
    pub(in crate::app::client) portrait: Option<Arc<RenderImage>>,
}

impl UiUser {
    pub(in crate::app::client) fn fixture(index: usize) -> Self {
        let user = preview::USERS[index];
        Self {
            handle: u32::try_from(index).unwrap_or(u32::MAX),
            name: user.display_name(),
            presence_id: None,
            presence: match user.presence {
                preview::FixturePresence::Available => PresenceKind::Available,
                preview::FixturePresence::Away => PresenceKind::Away,
                preview::FixturePresence::Busy => PresenceKind::Busy,
                preview::FixturePresence::InGame => PresenceKind::InGame,
            },
            portrait: None,
        }
    }

    pub(in crate::app::client) fn live(user: &ChatUser, portraits: &mut PortraitRegistry) -> Self {
        Self {
            handle: user.handle,
            name: user.visible_name(),
            presence_id: user.presence_id,
            presence: presence_kind(user.presence),
            portrait: user.avatar.and_then(|avatar| portraits.image(avatar)),
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
    }
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

pub(in crate::app::client) fn filtered_roster_range(
    users: &[UiUser],
    filter: &str,
    range: Range<usize>,
) -> Vec<UiUser> {
    ui_roster::filtered_range(users, filter, range, |user, filter| {
        ui_roster::filter_matches(&user.name, filter)
    })
}
