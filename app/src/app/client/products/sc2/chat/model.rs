use super::*;

/// joins that land inside this window of each other fold into one event rather
/// than stacking up as a wall of one-line arrivals.
pub(in crate::app::client) const MEMBERSHIP_COLLAPSE_WINDOW: Duration = Duration::from_secs(90);
/// above this many people, per-event lines stop carrying information and start
/// crowding out the conversation, so arrivals roll into a per-minute digest.
pub(in crate::app::client) const HIGH_TRAFFIC_ONLINE: usize = 25;
/// one digest line covers this much time before a new one starts.
pub(in crate::app::client) const DIGEST_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(in crate::app::client) enum ChatLine {
    Notice {
        time: String,
        text: String,
    },
    /// the local user's own arrival, which marks where this session starts.
    SessionStart {
        time: String,
        channel: String,
    },
    Membership {
        time: String,
        at: Instant,
        kind: MembershipKind,
        members: Vec<UiUser>,
        expanded: bool,
    },
    /// a minute of a busy channel, both directions in one line. whether it is
    /// showing its names is view state, not transcript data — see
    /// `ChatComponent::expanded_digest`.
    Digest {
        time: String,
        at: Instant,
        opened: Instant,
        joined: Vec<UiUser>,
        left: Vec<UiUser>,
    },
    Message {
        time: String,
        sender: UiUser,
        text: String,
    },
    Error {
        time: String,
        text: String,
    },
    /// a party invitation, which arrives in the transcript rather than as a
    /// card over it: it is something somebody said to you, and it waits in the
    /// room the way a message does.
    Invitation {
        time: String,
        id: u64,
        inviter: String,
        /// what the invitation is to, in the words the row shows.
        detail: String,
        /// cleared once you have answered, so the line settles into a record of
        /// what happened instead of asking twice.
        answered: Option<bool>,
    },
}

pub(in crate::app::client) struct ChatEntryReveal {
    pub(in crate::app::client) tab_id: u64,
    pub(in crate::app::client) line_index: usize,
    pub(in crate::app::client) started: Instant,
}

impl From<&FixtureLine> for ChatLine {
    fn from(line: &FixtureLine) -> Self {
        match line {
            FixtureLine::Notice { time, text } => Self::Notice {
                time: (*time).to_owned(),
                text: (*text).to_owned(),
            },
            FixtureLine::Membership { time, user } => Self::Membership {
                time: (*time).to_owned(),
                at: Instant::now(),
                kind: MembershipKind::Joined,
                members: vec![UiUser::fixture(*user)],
                expanded: false,
            },
            FixtureLine::Message { time, user, text } => Self::Message {
                time: (*time).to_owned(),
                sender: UiUser::fixture(*user),
                text: (*text).to_owned(),
            },
        }
    }
}

/// a join lands before its avatar does — the portrait arrives with a later
/// roster update. resolve against the live roster every render so a chip fills
/// in place instead of keeping the placeholder it was born with.
pub(in crate::app::client) fn freshest_member<'a>(
    member: &'a UiUser,
    roster: &'a [UiUser],
) -> &'a UiUser {
    if member.portrait.is_some() {
        return member;
    }
    roster
        .iter()
        .find(|user| user.handle == member.handle && user.portrait.is_some())
        .unwrap_or(member)
}

fn shared_members(
    members: &[UiUser],
    roster: &[UiUser],
    affinity: Option<&RosterAffinity>,
    assets: &Sc2Assets,
) -> Vec<RosterUser> {
    members
        .iter()
        .map(|member| {
            let member = freshest_member(member, roster);
            let mut shared = shared_roster_user(member, assets);
            if let Some(affinity) = affinity {
                shared.tone = affinity.tone(member);
            }
            shared
        })
        .collect()
}

pub(in crate::app::client) fn shared_transcript_line(
    line: &ChatLine,
    online: usize,
    roster: &[UiUser],
    affinity: Option<&RosterAffinity>,
    assets: &Sc2Assets,
) -> TranscriptLine {
    match line {
        ChatLine::Notice { time, text } => TranscriptLine::Notice {
            time: time.clone(),
            text: text.clone(),
        },
        ChatLine::SessionStart { time, channel } => TranscriptLine::SessionStart {
            time: time.clone(),
            channel: channel.clone(),
            online,
        },
        ChatLine::Membership {
            time,
            kind,
            members,
            expanded,
            ..
        } => TranscriptLine::Membership(MembershipEvent {
            time: time.clone(),
            kind: *kind,
            members: shared_members(members, roster, affinity, assets),
            expanded: *expanded,
        }),
        ChatLine::Digest {
            time, joined, left, ..
        } => TranscriptLine::Digest(DigestEvent {
            time: time.clone(),
            joined: shared_members(joined, roster, affinity, assets),
            left: shared_members(left, roster, affinity, assets),
        }),
        ChatLine::Message { time, sender, text } => TranscriptLine::Message {
            time: time.clone(),
            // resolved against the live roster the same way membership events
            // are: a portrait or clan tag can arrive after the message did
            sender: shared_members(std::slice::from_ref(sender), roster, affinity, assets)
                .pop()
                .unwrap_or_else(|| shared_roster_user(sender, assets)),
            text: text.clone(),
        },
        // an invitation is drawn by the app, which owns the buttons; it only
        // reaches the shared line for the transcript's own bookkeeping
        ChatLine::Invitation {
            time,
            inviter,
            detail,
            answered,
            ..
        } => TranscriptLine::Notice {
            time: time.clone(),
            text: match answered {
                Some(true) => format!("You joined {inviter}'s party."),
                Some(false) => format!("You declined {inviter}'s invitation."),
                None => format!("{inviter} {detail}"),
            },
        },
        ChatLine::Error { time, text } => TranscriptLine::Error {
            time: time.clone(),
            text: text.clone(),
        },
    }
}
