#[derive(Clone, Copy, Debug)]
pub enum FixturePresence {
    Available,
    Away,
    Busy,
    InGame,
}

#[derive(Clone, Copy, Debug)]
pub struct FixtureUser {
    pub name: &'static str,
    pub clan_tag: Option<&'static str>,
    pub presence: FixturePresence,
}

impl FixtureUser {
    #[must_use]
    pub fn display_name(self) -> String {
        self.clan_tag.map_or_else(
            || self.name.to_owned(),
            |tag| format!("<{tag}> {}", self.name),
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FixtureLine {
    Notice {
        time: &'static str,
        text: &'static str,
    },
    Membership {
        time: &'static str,
        user: usize,
    },
    Message {
        time: &'static str,
        user: usize,
        text: &'static str,
    },
}

pub const CHANNEL: &str = "General";
pub const USERS: &[FixtureUser] = &[
    FixtureUser {
        name: "Commander",
        clan_tag: Some("SC2"),
        presence: FixturePresence::Available,
    },
    FixtureUser {
        name: "Nova",
        clan_tag: None,
        presence: FixturePresence::InGame,
    },
    FixtureUser {
        name: "Raynor",
        clan_tag: Some("RAY"),
        presence: FixturePresence::Available,
    },
    FixtureUser {
        name: "Artanis",
        clan_tag: None,
        presence: FixturePresence::Away,
    },
    FixtureUser {
        name: "Kerrigan",
        clan_tag: Some("SWM"),
        presence: FixturePresence::Busy,
    },
];

pub const TRANSCRIPT: &[FixtureLine] = &[
    FixtureLine::Notice {
        time: "7:31 PM",
        text: "Welcome to Superiority.",
    },
    FixtureLine::Membership {
        time: "7:32 PM",
        user: 1,
    },
    FixtureLine::Message {
        time: "7:32 PM",
        user: 1,
        text: "Anyone up for a few games?",
    },
    FixtureLine::Message {
        time: "7:33 PM",
        user: 2,
        text: "Sure — finishing this build, then I’m ready.",
    },
    FixtureLine::Message {
        time: "7:34 PM",
        user: 0,
        text: "Perfect. I’ll make the party.",
    },
];

/// catalogue the `/join` popup is previewed against: a channel name, the
/// conference serving it, and how many people are in there. without a live
/// session there is no directory, so the fast path has nothing to autocomplete.
pub const CATALOGUE: &[(u16, &str, u16)] = &[
    (1028, "General", 370),
    (1030, "Protoss Strategy", 231),
    (1031, "Probes & Pylons", 88),
    (1033, "Practice League", 64),
    (1035, "Looking for Quick Match Team", 96),
    (1036, "Looking for Cooperative Team", 3),
];

/// a group the preview has already searched up, so the popup shows a row that
/// carries an icon and a kind next to the bare channels.
pub const CATALOGUE_GROUP: (u32, &str, u32, u32) = (5322, "Protectorate", 42, 6);
