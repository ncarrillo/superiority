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

/// the shared Social interaction dressed in the focused realm's visual
/// language. Keeping this palette beside the shared state prevents a product
/// adapter from accidentally growing a second Social implementation.
#[derive(Clone, Copy)]
struct SocialSkin {
    variant: ui_shared_modal::ModalVariant,
    interface_font: &'static str,
    body_font: &'static str,
    text: u32,
    bright: u32,
    muted: u32,
    accent: u32,
    structural: u32,
    hover: u32,
    whisper: u32,
    whisper_rule: u32,
    whisper_wash: u32,
    outgoing_fill: u32,
    outgoing_border: u32,
    outgoing_text: u32,
    incoming_fill: u32,
    incoming_border: u32,
    incoming_text: u32,
}

impl SocialSkin {
    const fn for_variant(variant: ui_shared_modal::ModalVariant) -> Self {
        match variant {
            ui_shared_modal::ModalVariant::Sc2 => Self {
                variant,
                interface_font: FONT_INTERFACE,
                body_font: FONT_INTERNATIONAL,
                text: TEXT,
                bright: 0x00e6_f9ff,
                muted: MUTED,
                accent: NOTICE,
                structural: BORDER_STRUCTURAL,
                hover: 0x1231_5e59,
                whisper: 0x00c0_84e8,
                whisper_rule: 0x783c_a066,
                whisper_wash: 0x5028_7833,
                outgoing_fill: 0x1231_5e8c,
                outgoing_border: 0x33a8_f059,
                outgoing_text: 0x00e6_f9ff,
                incoming_fill: 0x5028_784d,
                incoming_border: 0xc084_e859,
                incoming_text: 0x00e8_ddf2,
            },
            ui_shared_modal::ModalVariant::Remastered => Self {
                variant,
                interface_font: ui_scr_theme::FONT_INTERFACE,
                body_font: ui_scr_theme::FONT_INTERNATIONAL,
                text: ui_scr_theme::TEXT,
                bright: 0x00ff_e8e2,
                muted: ui_scr_theme::MUTED,
                accent: ui_scr_theme::ACCENT,
                structural: ui_scr_theme::BORDER_STRUCTURAL,
                hover: 0x4012_0e4d,
                whisper: 0x00e8_a838,
                whisper_rule: 0xe8a8_3866,
                whisper_wash: 0x4a2d_1033,
                outgoing_fill: 0x3812_105c,
                outgoing_border: 0xff8a_7866,
                outgoing_text: 0x00ff_e8e2,
                incoming_fill: 0x4a20_183d,
                incoming_border: 0xc93a_2c66,
                incoming_text: 0x00f0_d8d0,
            },
            ui_shared_modal::ModalVariant::Reforged => Self {
                variant,
                interface_font: ui_wc3_theme::FONT_INTERFACE,
                body_font: ui_wc3_theme::FONT_INTERFACE,
                text: ui_wc3_theme::PARCHMENT,
                bright: ui_wc3_theme::GOLD_BRIGHT,
                muted: ui_wc3_theme::MUTED,
                accent: ui_wc3_theme::GOLD,
                structural: 0x5e4a_2680,
                hover: 0x5e4a_264d,
                whisper: ui_wc3_theme::EMBER_BRIGHT,
                whisper_rule: 0xc88a_6a66,
                whisper_wash: 0x5a28_1433,
                outgoing_fill: 0x4a38_1f73,
                outgoing_border: 0xe8c8_7466,
                outgoing_text: ui_wc3_theme::PARCHMENT,
                incoming_fill: 0x4a20_164d,
                incoming_border: 0xc88a_6a66,
                incoming_text: ui_wc3_theme::PARCHMENT,
            },
        }
    }

    fn presence_color(self, presence: PresenceState) -> u32 {
        use ui_shared_modal::ModalVariant::{Reforged, Remastered, Sc2};
        match (self.variant, presence) {
            (Sc2, state) => presence_kind(state).dot_color(),
            (Remastered, PresenceState::Available) => ui_scr_theme::ACCENT,
            (Remastered, PresenceState::Away) => 0x00d0_a94f,
            (Remastered, PresenceState::Busy) => 0x00e3_5f4e,
            (Remastered, PresenceState::InGame) => 0x005f_d8dd,
            (Remastered, PresenceState::Offline | PresenceState::Unknown) => ui_scr_theme::MUTED,
            (Reforged, PresenceState::Available | PresenceState::InGame) => ui_wc3_theme::MOSS,
            (Reforged, PresenceState::Away) => ui_wc3_theme::GOLD_DIM,
            (Reforged, PresenceState::Busy) => ui_wc3_theme::EMBER_BRIGHT,
            (Reforged, PresenceState::Offline | PresenceState::Unknown) => ui_wc3_theme::QUIET,
        }
    }
}

pub(in crate::app::client) struct SocialComponent {
    pub(in crate::app::client) social_collapsed: [bool; 2],
    pub(in crate::app::client) friends_snapshot: Vec<ChatFriend>,
    pub(in crate::app::client) friends: Vec<UiFriend>,
    pub(in crate::app::client) blocked_accounts: Vec<BlockedAccount>,
    pub(in crate::app::client) social_scroll: ScrollHandle,
    pub(in crate::app::client) social_detail_open: bool,
    pub(in crate::app::client) social_pane_transition: Option<SocialPaneTransition>,
    pub(in crate::app::client) conversation_peer: Option<String>,
    pub(in crate::app::client) conversation_input: ui_text_input::TextInput,
    pub(in crate::app::client) conversation_focused: bool,
    pub(in crate::app::client) conversation_scroll: ScrollHandle,
    pub(in crate::app::client) conversations: BTreeMap<String, Vec<ConversationLine>>,
    pub(in crate::app::client) whisper_unread: BTreeMap<String, usize>,
    /// how to reach each peer, remembered from the row that opened the thread.
    /// a `/w` result carries the handle the service answers to; a name alone
    /// makes it resolve the person all over again.
    pub(in crate::app::client) whisper_targets: BTreeMap<String, WhisperTarget>,
}

impl SocialComponent {
    pub(in crate::app::client) fn clear(&mut self) {
        self.friends_snapshot.clear();
        self.friends.clear();
        self.blocked_accounts.clear();
        self.conversations.clear();
        self.whisper_unread.clear();
        self.whisper_targets.clear();
        self.conversation_peer = None;
        self.conversation_input.clear();
        self.conversation_focused = false;
        self.social_detail_open = false;
        self.social_pane_transition = None;
        self.social_collapsed = [false, false];
    }

    pub(in crate::app::client) fn open_conversation(
        &mut self,
        peer: String,
        window: &mut Window,
        cx: &mut App,
    ) {
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
    pub(in crate::app::client) fn present_conversation(
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

    pub(in crate::app::client) fn close_conversation(
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

    pub(in crate::app::client) fn pane_offset(&self, now: Instant) -> f32 {
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

    pub(in crate::app::client) fn record_whisper(
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

    pub(in crate::app::client) fn send_message(
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
