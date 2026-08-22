//! StarCraft: Remastered desktop state and Terran-console presentation.

use super::super::*;
use superiority_core::games::scr::chat::{
    ChatChannel as ClassicChatChannel, ChatEvent as ClassicChatEvent, ChatUser as ClassicChatUser,
    EventKind as ClassicEventKind,
};

mod avatar;
mod model;
mod social;
mod view;

pub(in crate::app::client) use model::{
    ScrLine, ScrMember, ScrMessageKind, ScrNoticeKind, ScrPresence, ScrSessionUi,
};

pub(in crate::app::client) const SCR_CHAT_ENTRY_REVEAL_DURATION: Duration =
    Duration::from_millis(350);

impl SuperiorityView {
    pub(in crate::app::client) fn record_classic_whisper(
        &mut self,
        peer: String,
        target: WhisperTarget,
        body: String,
        outgoing: bool,
        time: String,
    ) {
        let display = strip_character_code(&peer).to_owned();
        self.session.social.whisper_targets.insert(display, target);
        self.session
            .social
            .record_whisper(peer, body, outgoing, time);
    }

    pub(in crate::app::client) fn append_classic_line(&mut self, event: &ClassicChatEvent) {
        let time = Self::current_timestamp();
        if event.aurora_whisper.is_none()
            && let Some(scr) = self.session.scr_mut()
        {
            scr.append_event(event, time.clone());
        }
        if event.kind == ClassicEventKind::Whisper
            && let (Some(peer), Some(body)) = (event.sender.as_ref(), event.text.as_ref())
        {
            if let Some(whisper) = event.aurora_whisper {
                self.record_classic_whisper(
                    peer.clone(),
                    WhisperTarget::Account(whisper.account_id),
                    body.clone(),
                    whisper.outgoing,
                    time,
                );
                return;
            }
            let local = self
                .session
                .scr()
                .and_then(|scr| scr.local_member(self.session.account_battle_tag.as_deref()))
                .map(|member| member.name.as_str());
            if local.is_none_or(|local| !local.eq_ignore_ascii_case(peer)) {
                self.record_classic_whisper(
                    peer.clone(),
                    WhisperTarget::Name(peer.clone()),
                    body.clone(),
                    false,
                    time,
                );
            }
        }
    }

    pub(in crate::app::client) fn apply_classic_channel(&mut self, channel: &ClassicChatChannel) {
        let time = Self::current_timestamp();
        if let Some(scr) = self.session.scr_mut() {
            scr.apply_channel(channel, time);
        }
        self.refresh_classic_friend_avatars();
    }
}
