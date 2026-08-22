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

    /// the member list spends one 9px dot per person instead of an icon and a
    /// word, so the dot carries the whole state.
    #[must_use]
    pub const fn dot_color(self) -> u32 {
        match self {
            Self::Available => 0x0047_d184,
            Self::Away => 0x00d6_b95a,
            Self::Busy => 0x00e3_3d45,
            Self::InGame => 0x004a_9de0,
            Self::Offline | Self::Unknown => 0x005b_6b80,
        }
    }

    /// present states glow; absent ones sit flat so a wall of away players
    /// never lights up the panel.
    #[must_use]
    pub const fn dot_glows(self) -> bool {
        matches!(self, Self::Available | Self::Busy | Self::InGame)
    }

    /// whether a row earns a second line. "Available" is already said by the
    /// dot, and an away player has nothing to add; being in a game does.
    #[must_use]
    pub const fn has_detail(self) -> bool {
        matches!(self, Self::Busy | Self::InGame)
    }

    #[must_use]
    pub const fn absent(self) -> bool {
        matches!(self, Self::Away | Self::Offline | Self::Unknown)
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
    /// the name as it reads everywhere else in the app — clan tag included.
    pub name: String,
    pub presence_id: Option<u32>,
    pub presence: PresenceKind,
    pub presence_label: String,
    pub presence_icon: ImageSource,
    pub portrait: Option<Portrait>,
    pub clan_tag: Option<String>,
    /// your own clan reads brighter than everybody else's.
    pub clan_is_local: bool,
    pub tone: RosterUserTone,
    pub dimmed: bool,
}

impl RosterUser {
    /// the name with the clan tag lifted off, because the member list draws
    /// the tag itself in gold rather than letting it ride along in the name.
    #[must_use]
    pub fn bare_name(&self) -> &str {
        self.clan_tag
            .as_ref()
            .and_then(|tag| self.name.strip_prefix(&format!("<{tag}> ")))
            .unwrap_or(&self.name)
    }

    /// the second line, or nothing when the dot has already said it all.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.presence
            .has_detail()
            .then_some(self.presence_label.as_str())
    }

    #[must_use]
    pub fn clan_tag_color(&self) -> u32 {
        if self.clan_is_local {
            0x00f0_aa64
        } else {
            0x00c9_a86a
        }
    }
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

/// the bands a member list is cut into. order is the reading order: the
/// people you are already playing with, then the ones you have signed up to,
/// then the rest of the room.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum RosterSegment {
    Clan,
    Party,
    Friends,
    #[default]
    Everyone,
}

impl RosterSegment {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clan => "CLAN",
            Self::Party => "PARTY",
            Self::Friends => "FRIENDS",
            Self::Everyone => "EVERYONE",
        }
    }

    /// the header wears the same accent as the rows under it; "everyone" wears
    /// none, which is what makes the others read as an exception.
    #[must_use]
    pub const fn accent(self) -> u32 {
        match self {
            Self::Clan => 0x00f0_aa64,
            Self::Party => 0x00f0_92c4,
            Self::Friends => 0x006b_c2f2,
            Self::Everyone => 0x007d_8fa8,
        }
    }

    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Clan => 0,
            Self::Party => 1,
            Self::Friends => 2,
            Self::Everyone => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
// five yes/no facts about one person; folding them into flags would make every
// call site less readable, not more.
#[allow(clippy::struct_excessive_bools)]
pub struct RosterRelationship {
    /// somebody *else* wearing your clan tag — this is what tints a name.
    pub shared_clan: bool,
    /// wearing your clan tag, you included. you stand with your own clan in
    /// the member list even though your own name is never tinted.
    pub own_clan: bool,
    pub shared_party: bool,
    pub friend: bool,
    pub away: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RosterPresentation {
    pub tone: RosterUserTone,
    pub segment: RosterSegment,
    /// sort key inside the panel: the segment, then presence. away players
    /// sink to the bottom of their own band rather than into a band of their
    /// own — the dimming already says where they are.
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
        let segment = if relationship.own_clan {
            RosterSegment::Clan
        } else if relationship.shared_party {
            RosterSegment::Party
        } else if relationship.friend {
            RosterSegment::Friends
        } else {
            RosterSegment::Everyone
        };
        let rank = segment.rank() * 2 + relationship.away as u8;
        Self {
            tone,
            segment,
            rank,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MembershipKind {
    Joined,
    Left,
}

impl MembershipKind {
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Joined => "joined",
            Self::Left => "left the channel",
        }
    }
}

/// one join or leave. several people arriving in quick succession collapse into
/// a single event holding all of them, which is why `members` is a list.
#[derive(Clone)]
pub struct MembershipEvent {
    pub time: String,
    pub kind: MembershipKind,
    pub members: Vec<RosterUser>,
    pub expanded: bool,
}

impl MembershipEvent {
    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.members.len() > 1 && !self.expanded
    }

    /// one arrival reads as the name alone; several read as the first name and
    /// a count of everyone behind it.
    #[must_use]
    pub fn summary(&self) -> String {
        let verb = self.kind.verb();
        match self.members.split_first() {
            None => format!("Someone {verb}"),
            Some((first, [])) => format!("{} {verb}", first.name),
            Some((first, rest)) => {
                format!("{} and {} others {verb}", first.name, rest.len())
            }
        }
    }
}

/// a minute of a busy channel rolled into one line. both directions live here:
/// giving leaves their own rows would let them interleave with joins and break
/// the collapsing entirely.
#[derive(Clone)]
pub struct DigestEvent {
    pub time: String,
    pub joined: Vec<RosterUser>,
    pub left: Vec<RosterUser>,
}

impl DigestEvent {
    /// beyond this many events in the minute the avatar stack stops meaning
    /// anything, so it is dropped rather than shown as a token three faces.
    const BUSY_MINUTE: usize = 20;
    const FACE_CAP: usize = 3;

    #[must_use]
    pub fn events(&self) -> usize {
        self.joined.len() + self.left.len()
    }

    /// the first few joiners, or nobody when the minute was too busy for a
    /// handful of faces to be representative.
    #[must_use]
    pub fn faces(&self) -> &[RosterUser] {
        if self.events() > Self::BUSY_MINUTE {
            return &[];
        }
        let cap = self.joined.len().min(Self::FACE_CAP);
        &self.joined[..cap]
    }

    /// rows a column shows before the rest go behind "+ N more".
    pub const VISIBLE_ROWS: u8 = 5;

    /// how many names a column hides. the panel never grows to fit them; the
    /// column scrolls instead.
    #[must_use]
    pub const fn overflow(count: usize) -> usize {
        count.saturating_sub(Self::VISIBLE_ROWS as usize)
    }

    #[must_use]
    pub fn summary(&self) -> String {
        format!("{} joined · {} left", self.joined.len(), self.left.len())
    }
}

#[derive(Clone)]
pub enum TranscriptLine {
    Notice {
        time: String,
        text: String,
    },
    /// the local user's own arrival — the line that marks where this session
    /// starts in the channel.
    SessionStart {
        time: String,
        channel: String,
        online: usize,
    },
    Membership(MembershipEvent),
    Digest(DigestEvent),
    Message {
        time: String,
        /// the speaker, carried whole so the line can show their portrait —
        /// the same face the roster and the join events show.
        sender: RosterUser,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
}
