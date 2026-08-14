use gpui::ImageSource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceKind {
    Available,
    Away,
    Busy,
    InGame,
    Offline,
    Unknown,
}

impl PresenceKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Away => "Away",
            Self::Busy => "Busy",
            Self::InGame => "In Game",
            Self::Offline => "Offline",
            Self::Unknown => "Presence unknown",
        }
    }

    #[must_use]
    pub fn text_color(self) -> gpui::Rgba {
        gpui::rgb(match self {
            Self::Available => 0x0047_d184,
            Self::Away => 0x00f0_a32e,
            Self::Busy => 0x00e3_3d45,
            Self::InGame => 0x00b6_68ef,
            Self::Offline | Self::Unknown => 0x007d_8fa8,
        })
    }
}

#[derive(Clone)]
pub enum Portrait {
    Image(ImageSource),
    Atlas {
        image: ImageSource,
        cell: u8,
        columns: u8,
        cell_size: f32,
    },
}

#[derive(Clone)]
pub struct RosterUser {
    pub handle: u32,
    pub name: String,
    pub presence_id: Option<u32>,
    pub presence_label: String,
    pub presence_icon: ImageSource,
    pub portrait: Option<Portrait>,
    pub tone: RosterUserTone,
    pub dimmed: bool,
    pub segment_start: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RosterUserTone {
    Clan,
    Party,
    #[default]
    Normal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RosterChannelKind {
    Standard,
    Group,
    Party,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RosterRelationship {
    pub shared_clan: bool,
    pub shared_party: bool,
    pub away: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RosterPresentation {
    pub tone: RosterUserTone,
    pub rank: u8,
}

impl RosterPresentation {
    #[must_use]
    pub const fn resolve(relationship: RosterRelationship) -> Self {
        let tone = if relationship.shared_clan {
            RosterUserTone::Clan
        } else if relationship.shared_party {
            RosterUserTone::Party
        } else {
            RosterUserTone::Normal
        };
        let rank = if relationship.shared_clan {
            0
        } else if relationship.shared_party {
            1
        } else if relationship.away {
            3
        } else {
            2
        };
        Self { tone, rank }
    }
}

#[derive(Clone)]
pub enum TranscriptLine {
    Notice {
        time: String,
        text: String,
    },
    Membership {
        time: String,
        text: String,
    },
    Message {
        time: String,
        sender: String,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
}
