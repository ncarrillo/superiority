use gpui::ImageSource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RosterPresence {
    Offline,
    Online,
    Away,
    Busy,
}

impl RosterPresence {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "Offline",
            Self::Online => "Online",
            Self::Away => "Away",
            Self::Busy => "Busy",
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
        match self {
            Self::Offline => super::theme::QUIET,
            Self::Online => super::theme::MOSS,
            Self::Away => super::theme::GOLD_DIM,
            Self::Busy => super::theme::EMBER_BRIGHT,
        }
    }

    #[must_use]
    pub const fn dimmed(self) -> bool {
        matches!(self, Self::Offline | Self::Away | Self::Busy)
    }
}

#[derive(Clone)]
pub struct RosterUser {
    pub handle: u64,
    pub name: String,
    pub presence: RosterPresence,
    pub portrait: ImageSource,
    pub clan_abbreviation: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranscriptLine {
    SessionStart {
        time: String,
        channel: String,
        online: usize,
    },
    Message {
        time: String,
        sender: Option<String>,
        text: String,
    },
    Notice {
        time: String,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
}
