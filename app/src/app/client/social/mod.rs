use super::*;

mod conversation;
mod model;
mod rows;
mod view;

pub(in crate::app::client) use model::{
    ConversationLine, SocialPaneTransition, UiFriend, friend_order, online_summary,
};

pub(in crate::app::client) const SOCIAL_PANE_SLIDE_DURATION: Duration = Duration::from_millis(260);
const SOCIAL_CONTENT_GUTTER: f32 = 24.0;
const SOCIAL_CONVERSATION_GUTTER: f32 = 30.0;
const SOCIAL_FRAME_CLIP_GUTTER: f32 = 18.0;
const SOCIAL_BODY_TOP: f32 = 80.0;
const SOCIAL_BODY_HEIGHT: f32 = 342.0;
const SOCIAL_GROUP_WHISPERS: usize = 0;
const SOCIAL_GROUP_FRIENDS: usize = 1;
/// the list is inset the same distance as a member row, so the two panels line
/// their portraits up with each other.
const SOCIAL_ROW_INSET: f32 = 14.0;
const SOCIAL_SECTION_HEIGHT: f32 = 26.0;
/// a whisper row carries a second line the member rows do not, so it is taller
/// than the 40px people rows around it.
const SOCIAL_WHISPER_ROW_HEIGHT: f32 = 44.0;
const SOCIAL_DIMMED_OPACITY: f32 = 0.45;
/// whispers wear the same purple as `/w` in the composer popup.
const WHISPER_ACCENT: u32 = 0x00c0_84e8;

pub(super) struct SocialComponent {
    pub(super) social_collapsed: [bool; 2],
    pub(super) friends_snapshot: Vec<ChatFriend>,
    pub(super) friends: Vec<UiFriend>,
    pub(super) blocked_accounts: Vec<BlockedAccount>,
    pub(super) social_scroll: ScrollHandle,
    pub(super) social_detail_open: bool,
    pub(super) social_pane_transition: Option<SocialPaneTransition>,
    pub(super) conversation_peer: Option<String>,
    pub(super) conversation_input: ui_text_input::TextInput,
    pub(super) conversation_focused: bool,
    pub(super) conversation_scroll: ScrollHandle,
    pub(super) conversations: BTreeMap<String, Vec<ConversationLine>>,
    pub(super) whisper_unread: BTreeMap<String, usize>,
    /// how to reach each peer, remembered from the row that opened the thread.
    /// a `/w` result carries the handle the service answers to; a name alone
    /// makes it resolve the person all over again.
    pub(super) whisper_targets: BTreeMap<String, WhisperTarget>,
}

impl SocialComponent {
    pub(super) fn open_conversation(&mut self, peer: String, window: &mut Window, cx: &mut App) {
        self.whisper_unread.remove(&peer);
        self.conversation_input
            .set_placeholder(format!("Whisper {peer}"));
        self.conversation_peer = Some(peer);
        self.social_detail_open = true;
        self.social_pane_transition = Some(SocialPaneTransition {
            forward: true,
            started: Instant::now(),
        });
        self.conversation_focused = true;
        self.conversation_input.focus(window, cx);
        self.conversation_scroll.scroll_to_bottom();
    }

    /// opens the panel straight onto a conversation, the way `/w` arrives at
    /// one. there is nothing to slide away from when the panel was not already
    /// showing the list, so in that case it simply starts on the thread.
    pub(super) fn present_conversation(
        &mut self,
        peer: WhisperPeer,
        sliding: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.whisper_targets
            .insert(peer.display.clone(), peer.target);
        self.open_conversation(peer.display, window, cx);
        if !sliding {
            self.social_pane_transition = None;
        }
    }

    pub(super) fn close_conversation(
        &mut self,
        root_focus: &FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if !self.social_detail_open {
            return false;
        }
        self.social_detail_open = false;
        self.conversation_focused = false;
        root_focus.focus(window, cx);
        self.social_pane_transition = Some(SocialPaneTransition {
            forward: false,
            started: Instant::now(),
        });
        true
    }

    pub(super) fn pane_offset(&self, now: Instant) -> f32 {
        let Some(transition) = &self.social_pane_transition else {
            return if self.social_detail_open { -400.0 } else { 0.0 };
        };
        let progress = ease_in_out(
            (now.saturating_duration_since(transition.started)
                .as_secs_f32()
                / SOCIAL_PANE_SLIDE_DURATION.as_secs_f32())
            .clamp(0.0, 1.0),
        );
        if transition.forward {
            -400.0 * progress
        } else {
            -400.0 * (1.0 - progress)
        }
    }

    pub(super) fn record_whisper(
        &mut self,
        peer: String,
        body: String,
        outgoing: bool,
        timestamp: String,
    ) {
        let peer = strip_character_code(&peer).to_owned();
        let history = self.conversations.entry(peer.clone()).or_default();
        history.push(ConversationLine {
            timestamp,
            outgoing,
            body,
        });
        if history.len() > 500 {
            history.remove(0);
        }
        if self.conversation_peer.as_deref() == Some(peer.as_str()) {
            self.conversation_scroll.scroll_to_bottom();
        } else if !outgoing {
            *self.whisper_unread.entry(peer).or_default() += 1;
        }
    }

    pub(super) fn send_message(
        &mut self,
        connected: bool,
        commands: Option<&Sender<ClientCommand>>,
    ) -> bool {
        let body = self.conversation_input.content().trim().to_owned();
        let Some(peer) = self.conversation_peer.clone() else {
            return false;
        };
        if body.is_empty() || !connected {
            return false;
        }
        let target = self
            .whisper_targets
            .get(&peer)
            .or_else(|| {
                self.friends
                    .iter()
                    .find(|friend| friend.name == peer)
                    .map(|friend| &friend.target)
            })
            .cloned()
            .unwrap_or_else(|| WhisperTarget::Name(peer.clone()));
        let Some(commands) = commands else {
            return false;
        };
        if commands
            .send(ClientCommand::SendWhisper {
                target,
                display_name: peer,
                body,
            })
            .is_err()
        {
            return false;
        }
        self.conversation_input.clear();
        true
    }
}
