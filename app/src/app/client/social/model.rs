use super::*;

#[derive(Clone)]
pub(in crate::app::client) struct UiFriend {
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) presence: PresenceState,
    pub(in crate::app::client) portrait: Option<Arc<RenderImage>>,
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
            portrait: friend.avatar.and_then(|avatar| portraits.image(avatar)),
            target: friend.target.clone(),
        }
    }

    pub(in crate::app::client) fn is_online(&self) -> bool {
        !matches!(
            self.presence,
            PresenceState::Offline | PresenceState::Unknown
        )
    }
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
