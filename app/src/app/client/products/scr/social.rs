//! Remastered adapters for the shared StarCraft II Social surface.
//!
//! The presentation and interaction are intentionally the SC2 implementation:
//! the same frame, sections, presence rows, unread state, pane transition, and
//! whisper conversation. Only the identities and presence values come from
//! SC:R's classic services.

use super::*;
use superiority_core::games::scr::chat::{
    ChatFriend as ClassicChatFriend, FriendPresence as ClassicFriendPresence,
};

impl SuperiorityView {
    pub(in crate::app::client) fn apply_classic_friends(
        &mut self,
        friends: Vec<ClassicChatFriend>,
    ) {
        let avatars = self.scr_member_avatars();
        self.session.social.friends = friends
            .into_iter()
            .map(|friend| {
                let target_name = friend.name;
                let source = avatars
                    .get(&target_name.to_ascii_lowercase())
                    .cloned()
                    .unwrap_or_else(avatar::default_source);
                UiFriend {
                    name: strip_character_code(&target_name).to_owned(),
                    presence: match friend.presence {
                        ClassicFriendPresence::Online => PresenceState::Available,
                        ClassicFriendPresence::InGame => PresenceState::InGame,
                        ClassicFriendPresence::Offline => PresenceState::Offline,
                    },
                    portrait: Some(Portrait::Image(source.into())),
                    // keep the account identity supplied by AuroraFriends even
                    // though the row displays the friendly name without its
                    // discriminator. The worker must never address a whisper
                    // by presentation text.
                    target: friend
                        .account_id
                        .map_or_else(|| WhisperTarget::Name(target_name), WhisperTarget::Account),
                }
            })
            .collect();
    }

    pub(in crate::app::client) fn refresh_classic_friend_avatars(&mut self) {
        let avatars = self.scr_member_avatars();
        for friend in &mut self.session.social.friends {
            let target_name = match &friend.target {
                WhisperTarget::Name(name) => name.as_str(),
                _ => friend.name.as_str(),
            };
            let source = avatars
                .get(&target_name.to_ascii_lowercase())
                .or_else(|| avatars.get(&friend.name.to_ascii_lowercase()))
                .cloned()
                .unwrap_or_else(avatar::default_source);
            friend.portrait = Some(Portrait::Image(source.into()));
        }
    }

    fn scr_member_avatars(&self) -> BTreeMap<String, String> {
        self.session
            .scr()
            .and_then(|scr| scr.channel.as_ref())
            .into_iter()
            .flat_map(|channel| channel.members.iter())
            .flat_map(|member| {
                member.avatar_url.as_ref().into_iter().flat_map(|avatar| {
                    std::iter::once((member.name.to_ascii_lowercase(), avatar.clone())).chain(
                        member
                            .battle_tag
                            .as_ref()
                            .map(|tag| (tag.to_ascii_lowercase(), avatar.clone())),
                    )
                })
            })
            .collect()
    }
}
