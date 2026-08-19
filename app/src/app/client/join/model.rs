use super::*;

#[derive(Clone)]
pub(in crate::app::client) struct UiGroupSummary {
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) private: bool,
    pub(in crate::app::client) kind: u8,
    pub(in crate::app::client) category: u8,
    pub(in crate::app::client) member_count: Option<u32>,
    pub(in crate::app::client) online: Option<u32>,
}

impl UiGroupSummary {
    pub(in crate::app::client) fn label(&self) -> Option<&'static str> {
        if self.kind == 2 {
            return Some("Clan");
        }
        match self.category {
            1 => Some("Community"),
            2 => Some("Barcraft"),
            3 => Some("Esports team"),
            4 => Some("Coaching"),
            5 => Some("Company"),
            6 => Some("Region"),
            7 => Some("School"),
            8 => Some("Broadcaster"),
            9 => Some("Other"),
            10 => Some("Esports league"),
            11 => Some("Arcade"),
            12 => Some("Internet cafe"),
            _ => None,
        }
    }

    pub(in crate::app::client) fn icon(&self) -> &'static str {
        if self.kind == 2 {
            return "images/groups/clan.png";
        }
        match self.category {
            1 => "images/groups/community.png",
            2 => "images/groups/barcraft.png",
            3 => "images/groups/esports-teams.png",
            4 => "images/groups/coaching.png",
            5 => "images/groups/company.png",
            6 => "images/groups/region.png",
            7 => "images/groups/school.png",
            8 => "images/groups/shoutcast.png",
            9 => "images/groups/other.png",
            10 => "images/groups/esports-leagues.png",
            11 => "images/groups/arcade.png",
            12 => "images/groups/igr.png",
            _ => "images/groups/community.png",
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::app::client) enum InvitationKind {
    Group { club_id: u32 },
    Party { channel_index: u8 },
}

#[derive(Clone)]
pub(in crate::app::client) struct UiInvitation {
    pub(in crate::app::client) id: u64,
    pub(in crate::app::client) kind: InvitationKind,
    pub(in crate::app::client) inviter: Option<String>,
    pub(in crate::app::client) destination: Option<String>,
    pub(in crate::app::client) closing: bool,
}

impl UiInvitation {
    pub(in crate::app::client) fn destination_label(&self) -> String {
        self.destination.clone().unwrap_or_else(|| match self.kind {
            InvitationKind::Group { club_id } => format!("Group {club_id}"),
            InvitationKind::Party { .. } => "a party".to_owned(),
        })
    }

    pub(in crate::app::client) fn inviter_label(&self) -> &str {
        self.inviter.as_deref().unwrap_or("A player")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::app::client) enum JoinSource {
    /// a clan or group you already belong to.
    Group,
    Public,
    /// a group the directory answered with, which you are not in.
    Community,
}

impl JoinSource {
    /// the list ranks the groups you are in above the ones you merely found,
    /// and both above channels; channels then rank by population.
    pub(in crate::app::client) const fn rank(self) -> u8 {
        match self {
            Self::Group => 0,
            Self::Community => 1,
            Self::Public => 2,
        }
    }
}

/// a room the dialog can offer. channels you are already in never appear —
/// this list is only things you can act on.
#[derive(Clone)]
pub(in crate::app::client) struct JoinRow {
    pub(in crate::app::client) name: String,
    /// the kind tag a row wears, e.g. `COMMUNITY`. channels wear none.
    pub(in crate::app::client) note: Option<String>,
    pub(in crate::app::client) source: JoinSource,
    pub(in crate::app::client) target: ChatChannel,
    pub(in crate::app::client) icon: &'static str,
    /// how many people are in there now, when the service has told us.
    pub(in crate::app::client) count: Option<usize>,
}

/// under this many people a room is effectively empty, and the count stops
/// being an invitation and starts being a warning.
const QUIET_ROOM: usize = 10;

impl JoinRow {
    /// nobody is in there. the room is still offered — joining is how it stops
    /// being empty — but it reads and sorts below the ones with people in them.
    /// a room whose population we have not been told is not dead, only unknown.
    pub(in crate::app::client) fn dead(&self) -> bool {
        self.count == Some(0)
    }
}

#[must_use]
pub(in crate::app::client) const fn count_color(count: usize) -> u32 {
    if count >= QUIET_ROOM {
        0x0047_d184
    } else {
        0x007d_8fa8
    }
}
