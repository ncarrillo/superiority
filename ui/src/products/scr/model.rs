//! product-owned presentation values for a Remastered channel member.

use gpui::ImageSource;

#[derive(Clone)]
pub struct AccountIdentity {
    pub name: String,
    pub detail: String,
    pub detail_color: u32,
    pub portrait: ImageSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RosterPresence {
    Online,
    Away,
    Busy,
    InGame,
    InLobby,
}

impl RosterPresence {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Away => "Away",
            Self::Busy => "Busy",
            Self::InGame => "In game",
            Self::InLobby => "In lobby",
        }
    }

    #[must_use]
    pub const fn detail(self) -> Option<&'static str> {
        match self {
            Self::Online => None,
            _ => Some(self.label()),
        }
    }

    #[must_use]
    pub const fn color(self) -> u32 {
        // the console's temperature ramp: presence is brightness, not hue —
        // only Away steps out, into the caution amber
        match self {
            Self::Online => 0x00ff_8a78,
            Self::Away => 0x00d0_a94f,
            Self::Busy => 0x00b0_4a3e,
            Self::InGame => 0x00ff_d0c8,
            Self::InLobby => 0x00d8_8070,
        }
    }

    #[must_use]
    pub const fn dimmed(self) -> bool {
        matches!(self, Self::Away | Self::Busy)
    }
}

#[derive(Clone)]
pub struct RosterUser {
    pub handle: u32,
    pub name: String,
    pub presence: RosterPresence,
    pub portrait: ImageSource,
    pub is_operator: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptMessageKind {
    Talk,
    Whisper,
    Emote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptNoticeKind {
    Broadcast,
    Information,
}

#[derive(Clone)]
pub struct TranscriptSpeaker {
    pub name: String,
    pub portrait: ImageSource,
}

#[derive(Clone)]
pub enum TranscriptLine {
    SessionStart {
        time: String,
        channel: String,
        online: usize,
    },
    Message {
        time: String,
        kind: TranscriptMessageKind,
        sender: TranscriptSpeaker,
        text: String,
    },
    Notice {
        time: String,
        kind: TranscriptNoticeKind,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
}
