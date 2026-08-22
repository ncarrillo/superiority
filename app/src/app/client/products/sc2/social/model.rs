use super::*;

#[derive(Clone)]
pub(in crate::app::client) struct UiFriend {
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) presence: PresenceState,
    pub(in crate::app::client) portrait: Option<Portrait>,
    pub(in crate::app::client) target: WhisperTarget,
}

impl UiFriend {
    pub(in crate::app::client) fn live(
        friend: &ChatFriend,
        portraits: &mut PortraitRegistry,
    ) -> Self {
        Self {
            name: strip_character_code(&friend.name).to_owned(),
            presence: friend.presence,
            portrait: friend
                .avatar
                .and_then(|avatar| portraits.image(avatar))
                .map(|portrait| Portrait::Image(portrait.into())),
            target: friend.target.clone(),
        }
    }

    /// somebody you have a whisper thread with who is not on your friends
    /// list — enough of a person to draw a row for.
    pub(in crate::app::client) fn unknown(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            presence: PresenceState::Unknown,
            portrait: None,
            target: WhisperTarget::Name(name.to_owned()),
        }
    }

    pub(in crate::app::client) fn is_online(&self) -> bool {
        !matches!(
            self.presence,
            PresenceState::Offline | PresenceState::Unknown
        )
    }

    /// the friend dressed as a member-list row, so the social panel and the
    /// channel roster spend the same pixels on the same person. `dimmed` stays
    /// false because the panel dims the whole row itself, per refinement E.
    pub(in crate::app::client) fn roster_user(&self, assets: &Sc2Assets) -> RosterUser {
        let presence = presence_kind(self.presence);
        RosterUser {
            handle: 0,
            name: self.name.clone(),
            presence_id: None,
            presence,
            presence_label: presence.label().to_owned(),
            presence_icon: assets.presence_icon(presence),
            portrait: self.portrait.clone(),
            clan_tag: None,
            clan_is_local: false,
            tone: RosterUserTone::Normal,
            dimmed: false,
        }
    }
}

/// friends in reading order for the social panel: online first, then offline,
/// each alphabetical. refinement N deleted the online/offline sub-headers, so
/// this ordering plus the dimming is the only thing left saying who is around.
/// anybody already listed under whispers is left out — they are on screen once.
pub(in crate::app::client) fn friend_order<'a>(
    friends: &'a [UiFriend],
    listed_elsewhere: &[String],
) -> Vec<&'a UiFriend> {
    let mut order = friends
        .iter()
        .filter(|friend| !listed_elsewhere.iter().any(|peer| peer == &friend.name))
        .collect::<Vec<_>>();
    order.sort_by_key(|friend| {
        (
            u8::from(!friend.is_online()),
            friend.name.to_ascii_lowercase(),
        )
    });
    order
}

/// "1 of 6 online" — the count the FRIENDS header carries now that the rows
/// below it no longer label themselves.
pub(in crate::app::client) fn online_summary(friends: &[&UiFriend]) -> String {
    let online = friends.iter().filter(|friend| friend.is_online()).count();
    format!("{online} of {} online", friends.len())
}

#[derive(Clone)]
pub(in crate::app::client) struct ConversationLine {
    pub(in crate::app::client) timestamp: String,
    pub(in crate::app::client) outgoing: bool,
    pub(in crate::app::client) body: String,
}

pub(in crate::app::client) struct SocialPaneTransition {
    pub(in crate::app::client) forward: bool,
    pub(in crate::app::client) started: Instant,
}
