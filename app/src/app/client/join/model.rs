use super::*;

#[derive(Clone)]
pub(in crate::app::client) struct UiGroupSummary {
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) private: bool,
    pub(in crate::app::client) kind: u8,
    pub(in crate::app::client) category: u8,
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
    Group,
    Public,
    Community,
    Typed,
}

impl JoinSource {
    pub(in crate::app::client) fn heading(self) -> &'static str {
        match self {
            Self::Group => "Groups",
            Self::Public => "Battle.net channels",
            Self::Community => "Communities",
            Self::Typed => "",
        }
    }
}

#[derive(Clone)]
pub(in crate::app::client) struct JoinRow {
    pub(in crate::app::client) name: String,
    pub(in crate::app::client) note: Option<String>,
    pub(in crate::app::client) source: JoinSource,
    pub(in crate::app::client) target: ChatChannel,
    pub(in crate::app::client) icon: &'static str,
}
