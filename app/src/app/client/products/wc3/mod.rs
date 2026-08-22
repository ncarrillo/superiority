//! Warcraft III: Reforged desktop state and its warm stone realm surface.

use super::super::*;
use superiority_core::games::wc3::{
    ChatChannel as WarcraftChatChannel, ChatEvent as WarcraftChatEvent,
    ChatFriend as WarcraftChatFriend, ChatMember as WarcraftChatMember,
    ChatPresence as WarcraftChatPresence, ClanMembership as WarcraftClanMembership,
    ClanSnapshot as WarcraftClanSnapshot, FriendPresence as WarcraftFriendPresence,
};

mod avatar;
mod clan;
mod model;
mod tabs;
mod view;

pub(in crate::app::client) use model::{Wc3Member, Wc3Presence, Wc3SessionUi, Wc3TabClose};

impl SuperiorityView {
    pub(in crate::app::client) fn apply_warcraft_channel(&mut self, channel: &WarcraftChatChannel) {
        let time = Self::current_timestamp();
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.apply_channel(channel, time);
        }
    }

    pub(in crate::app::client) fn append_warcraft_event(&mut self, event: &WarcraftChatEvent) {
        let time = Self::current_timestamp();
        if let WarcraftChatEvent::Whisper {
            account_id,
            peer,
            body,
            outgoing,
        } = event
        {
            let display = strip_character_code(peer).to_owned();
            self.session
                .social
                .whisper_targets
                .insert(display, WhisperTarget::WarcraftAccount(*account_id));
            self.session
                .social
                .record_whisper(peer.clone(), body.clone(), *outgoing, time);
            return;
        }
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.append_event(event, time);
        }
    }

    pub(in crate::app::client) fn apply_warcraft_friends(
        &mut self,
        friends: Vec<WarcraftChatFriend>,
    ) {
        self.session.social.friends = friends
            .into_iter()
            .map(|friend| UiFriend {
                name: strip_character_code(&friend.name).to_owned(),
                presence: match friend.presence {
                    WarcraftFriendPresence::Online => PresenceState::Available,
                    WarcraftFriendPresence::Offline => PresenceState::Offline,
                },
                portrait: Some(Portrait::Image(avatar::source(None).into())),
                target: WhisperTarget::WarcraftAccount(friend.account_id),
            })
            .collect();
    }

    pub(in crate::app::client) fn apply_warcraft_channels(&mut self, channels: Vec<String>) {
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.set_public_channels(channels);
        }
    }

    pub(in crate::app::client) fn apply_warcraft_clan(&mut self, clan: WarcraftClanSnapshot) {
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.clan = clan;
        }
    }
}
