//! the wire shape the host sees.
//!
//! deliberately not `superiority_core`'s own types. core's `ChatEvent` moves as the
//! protocol work moves; this is the seam where we decide what a binding is
//! promised, so adding a variant is a decision rather than a side effect.

use std::collections::HashMap;

use serde::Serialize;
use superiority_core::{
    chat::{ChatChannel, ChatEvent, ChatFriend, ChatUser, RosterSnapshot, channel_title},
    connection::{AccountSummary, ClientEvent, ConnectionStage},
    native::PresenceState,
};

/// names arrive separately from the channels that use them: the public catalog
/// and group summaries turn up as their own events, so a channel is only
/// nameable once one of those has been seen. holding them here means a binding
/// gets a resolved name instead of assembling this map itself.
#[derive(Default)]
pub struct Names {
    public: HashMap<u16, String>,
    groups: HashMap<u32, String>,
}

impl Names {
    /// returns whether the event carried names worth keeping.
    pub fn learn(&mut self, event: &ClientEvent) -> bool {
        let ClientEvent::Chat(chat) = event else {
            return false;
        };
        match chat {
            ChatEvent::PublicChannelCatalog(channels) => {
                for channel in channels {
                    self.public.insert(channel.identifier, channel.name.clone());
                }
                true
            }
            ChatEvent::GroupSummary {
                club_id,
                name: Some(name),
                ..
            } => {
                self.groups.insert(*club_id, name.clone());
                true
            }
            _ => false,
        }
    }

    /// the same fallback the app uses: a learned name, else whatever core can
    /// derive from the identifier alone.
    fn resolve(&self, channel: &ChatChannel) -> String {
        match channel {
            ChatChannel::Public(identifier) => self.public.get(identifier).cloned(),
            ChatChannel::Club(club_id) => self.groups.get(club_id).cloned(),
            _ => None,
        }
        .unwrap_or_else(|| channel_title(channel))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Available,
    Away,
    Busy,
    InGame,
    Offline,
    Unknown,
}

impl From<PresenceState> for Presence {
    fn from(state: PresenceState) -> Self {
        match state {
            PresenceState::Available => Self::Available,
            PresenceState::Away => Self::Away,
            PresenceState::Busy => Self::Busy,
            PresenceState::InGame => Self::InGame,
            PresenceState::Offline => Self::Offline,
            PresenceState::Unknown => Self::Unknown,
        }
    }
}

#[derive(Serialize)]
pub struct User {
    /// identifies this membership within its channel.
    pub handle: u32,
    /// identifies the account across channels, when the server has told us.
    /// the join key for anything that wants one person, not one membership.
    pub presence_id: Option<u32>,
    /// as the client shows it, clan tag and all.
    pub name: String,
    pub clan_tag: Option<String>,
    /// often `unknown` on arrival: a join names someone before their profile
    /// exists. a later roster carries the real value.
    pub presence: Presence,
}

impl From<&ChatUser> for User {
    fn from(user: &ChatUser) -> Self {
        Self {
            handle: user.handle,
            presence_id: user.presence_id,
            name: user.visible_name(),
            clan_tag: user.clan_tag.clone(),
            presence: user.presence.into(),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Channel {
    Public { id: u16, name: String },
    Private { name: String },
    Group { club_id: u32, name: String },
    Party { name: String },
}

impl Channel {
    fn resolved(channel: &ChatChannel, names: &Names) -> Self {
        let name = names.resolve(channel);
        match channel {
            ChatChannel::Public(id) => Self::Public { id: *id, name },
            ChatChannel::Private(_) => Self::Private { name },
            ChatChannel::Club(club_id) => Self::Group {
                club_id: *club_id,
                name,
            },
            ChatChannel::Party => Self::Party { name },
        }
    }
}

#[derive(Serialize)]
pub struct Friend {
    pub name: String,
    pub presence: Presence,
}

#[derive(Serialize)]
pub struct SessionAccount {
    /// stable Battle.net identity. Unlike a `BattleTag`, this cannot be renamed.
    pub account_id: Option<u64>,
    pub battle_tag: Option<String>,
    pub region: Option<u32>,
    /// retail product `FourCCs` reported by Battle.net, when the account service
    /// supplied a catalogue.
    pub games: Option<Vec<String>>,
}

impl From<&AccountSummary> for SessionAccount {
    fn from(account: &AccountSummary) -> Self {
        Self {
            account_id: account.account_id,
            battle_tag: account.battle_tag.clone(),
            region: account.region,
            games: account.games.clone(),
        }
    }
}

impl From<&ChatFriend> for Friend {
    fn from(friend: &ChatFriend) -> Self {
        Self {
            name: friend.name.clone(),
            presence: friend.presence.into(),
        }
    }
}

/// the noisier protocol-level events core emits fold into `Other`, so a binding
/// does not grow a case every time the decoder learns something new.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Stage {
        stage: &'static str,
    },
    /// only fires when there is no cached credential, so a bot with a warm
    /// cache never sees it.
    AuthenticationRequired {
        auth_id: u64,
        url: String,
    },
    /// who the session authenticated as. arrives once per connection.
    Account {
        account: SessionAccount,
    },
    Joined {
        channel_index: u8,
        channel: Channel,
        local_handle: u32,
    },
    /// the channels this account may join, by name. arrives once per session.
    PublicChannels {
        channels: Vec<Channel>,
    },
    JoinRejected {
        channel: Option<Channel>,
        reason: Option<u16>,
    },
    Left {
        channel_index: u8,
        reason: Option<u16>,
    },
    /// replaces whatever came before for that channel.
    Roster {
        channel_index: u8,
        complete: bool,
        users: Vec<User>,
    },
    MemberJoined {
        channel_index: u8,
        user: User,
    },
    MemberLeft {
        channel_index: u8,
        user: User,
    },
    Message {
        channel_index: u8,
        sender: User,
        body: String,
    },
    Whisper {
        peer: String,
        body: String,
        /// true for whispers this client sent, echoed back.
        outgoing: bool,
    },
    WhisperFailed {
        peer: String,
        reason: String,
    },
    Friends {
        friends: Vec<Friend>,
    },
    GroupInvitation {
        club_id: u32,
    },
    PartyInvitation {
        inviter: Option<String>,
        channel_index: u8,
    },
    GroupSummary {
        club_id: u32,
        name: Option<String>,
        kind: u8,
        category: u8,
        private: bool,
        member: bool,
        member_count: Option<u32>,
        online: Option<u32>,
    },
    GroupSearch {
        club_ids: Vec<u32>,
    },
    /// the session is still up.
    CommandError {
        message: String,
    },
    /// expect `stage: "disconnected"` to follow.
    Error {
        message: String,
    },
    /// decoded, but with no binding-level meaning yet. only the variant is
    /// named: these include block lists, and debug-formatting
    /// them would put personal data — and core's internal shape — into a
    /// public contract.
    Other {
        kind: &'static str,
    },
    /// the session thread has finished; this client will report nothing more.
    SessionEnded,
}

/// the variant name alone, with none of its payload.
fn name_of(event: &ChatEvent) -> &'static str {
    match event {
        ChatEvent::PublicChannelCatalog(_) => "public_channel_catalog",
        ChatEvent::ConferenceDescriptions { .. } => "conference_descriptions",
        ChatEvent::ConferenceMemberCounts { .. } => "conference_member_counts",
        ChatEvent::Joined { .. } => "joined",
        ChatEvent::JoinRejected { .. } => "join_rejected",
        ChatEvent::Roster(_) => "roster",
        ChatEvent::MemberJoined { .. } => "member_joined",
        ChatEvent::MemberLeft { .. } => "member_left",
        ChatEvent::Removed { .. } => "removed",
        ChatEvent::Message { .. } => "message",
        ChatEvent::Whisper { .. } => "whisper",
        ChatEvent::Friends(_) => "friends",
        ChatEvent::BlockedAccounts(_) => "blocked_accounts",
        ChatEvent::Activity { .. } => "activity",
        ChatEvent::WhisperFailed { .. } => "whisper_failed",
        ChatEvent::GroupInvitation { .. } => "group_invitation",
        ChatEvent::PartyInvitation { .. } => "party_invitation",
        ChatEvent::GroupSummary { .. } => "group_summary",
        ChatEvent::GroupSearch { .. } => "group_search",
    }
}

fn stage_name(stage: ConnectionStage) -> &'static str {
    match stage {
        ConnectionStage::Disconnected => "disconnected",
        ConnectionStage::WebAuthentication => "web_authentication",
        ConnectionStage::GameUtilities => "game_utilities",
        ConnectionStage::NativeAuthentication => "native_authentication",
        ConnectionStage::ChatBootstrap => "chat_bootstrap",
        ConnectionStage::Connected => "connected",
    }
}

fn roster(snapshot: &RosterSnapshot) -> Event {
    Event::Roster {
        channel_index: snapshot.channel_index,
        complete: snapshot.initial_complete,
        users: snapshot.users.iter().map(User::from).collect(),
    }
}

/// `auth_id` is only consulted for the interactive sign-in variant, whose reply
/// channel the caller has already parked.
pub fn translate(event: &ClientEvent, auth_id: u64, names: &Names) -> Event {
    match event {
        ClientEvent::Stage(stage) => Event::Stage {
            stage: stage_name(*stage),
        },
        // stimpak's event vocabulary is StarCraft II's; Remastered has no
        // translation here yet, so it is reported rather than silently dropped
        ClientEvent::ClassicChannel(channel) => Event::Error {
            message: format!("unhandled classic channel: {}", channel.label()),
        },
        ClientEvent::Classic(event) => Event::Error {
            message: format!("unhandled classic chat event: {:?}", event.kind),
        },
        ClientEvent::ClassicFriends(friends) => Event::Error {
            message: format!(
                "unhandled classic friends snapshot: {} entries",
                friends.len()
            ),
        },
        ClientEvent::ClassicWhisperSent { peer, .. } => Event::Error {
            message: format!("unhandled classic whisper confirmation for {peer}"),
        },
        ClientEvent::WarcraftChannel(channel) => Event::Error {
            message: format!("unhandled Reforged channel: {}", channel.name),
        },
        ClientEvent::Warcraft(event) => Event::Error {
            message: format!("unhandled Reforged chat event: {event:?}"),
        },
        ClientEvent::WarcraftChannels(channels) => Event::Error {
            message: format!(
                "unhandled Reforged channel directory: {} entries",
                channels.len()
            ),
        },
        ClientEvent::WarcraftFriends(friends) => Event::Error {
            message: format!(
                "unhandled Reforged friends snapshot: {} entries",
                friends.len()
            ),
        },
        ClientEvent::WarcraftClan(clan) => Event::Error {
            message: format!("unhandled Reforged clan snapshot: {:?}", clan.membership),
        },
        ClientEvent::Authentication { url, .. } => Event::AuthenticationRequired {
            auth_id,
            url: url.to_string(),
        },
        ClientEvent::CommandError(message) => Event::CommandError {
            message: message.clone(),
        },
        ClientEvent::Error(message) => Event::Error {
            message: message.clone(),
        },
        ClientEvent::Chat(chat) => translate_chat(chat, names),
        ClientEvent::Account(account) => Event::Account {
            account: account.into(),
        },
        ClientEvent::ProductCredential { .. } => Event::Other {
            kind: "product-credential",
        },
    }
}

#[allow(clippy::match_same_arms, clippy::too_many_lines)]
fn translate_chat(event: &ChatEvent, names: &Names) -> Event {
    match event {
        ChatEvent::Joined {
            channel_index,
            channel,
            local_member_handle,
            ..
        } => Event::Joined {
            channel_index: *channel_index,
            channel: Channel::resolved(channel, names),
            local_handle: *local_member_handle,
        },
        ChatEvent::JoinRejected { channel, reason } => Event::JoinRejected {
            channel: channel
                .as_ref()
                .map(|channel| Channel::resolved(channel, names)),
            reason: *reason,
        },
        ChatEvent::Removed {
            channel_index,
            reason,
        } => Event::Left {
            channel_index: *channel_index,
            reason: *reason,
        },
        ChatEvent::PublicChannelCatalog(channels) => Event::PublicChannels {
            channels: channels
                .iter()
                .map(|channel| Channel::Public {
                    id: channel.identifier,
                    name: channel.name.clone(),
                })
                .collect(),
        },
        ChatEvent::Roster(snapshot) => roster(snapshot),
        ChatEvent::MemberJoined {
            channel_index,
            user,
        } => Event::MemberJoined {
            channel_index: *channel_index,
            user: user.into(),
        },
        ChatEvent::MemberLeft {
            channel_index,
            user,
            ..
        } => Event::MemberLeft {
            channel_index: *channel_index,
            user: user.into(),
        },
        ChatEvent::Message {
            channel_index,
            sender,
            body,
        } => Event::Message {
            channel_index: *channel_index,
            sender: sender.into(),
            body: body.clone(),
        },
        ChatEvent::Whisper {
            peer,
            body,
            outgoing,
        } => Event::Whisper {
            peer: peer.clone(),
            body: body.clone(),
            outgoing: *outgoing,
        },
        ChatEvent::WhisperFailed { peer, reason } => Event::WhisperFailed {
            peer: peer.clone(),
            reason: reason.clone(),
        },
        ChatEvent::Friends(friends) => Event::Friends {
            friends: friends.iter().map(Friend::from).collect(),
        },
        ChatEvent::GroupInvitation { club_id } => Event::GroupInvitation { club_id: *club_id },
        ChatEvent::PartyInvitation {
            inviter,
            channel_index,
        } => Event::PartyInvitation {
            inviter: inviter.clone(),
            channel_index: *channel_index,
        },
        ChatEvent::GroupSummary {
            club_id,
            name,
            kind,
            category,
            private,
            member,
            member_count,
            online,
        } => Event::GroupSummary {
            club_id: *club_id,
            name: name.clone(),
            kind: *kind,
            category: *category,
            private: *private,
            member: *member,
            member_count: *member_count,
            online: *online,
        },
        ChatEvent::GroupSearch { club_ids } => Event::GroupSearch {
            club_ids: club_ids.clone(),
        },
        other => Event::Other {
            kind: name_of(other),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use superiority_core::chat::PublicChannel;
    use superiority_core::connection::AccountSummary;

    fn catalog(entries: &[(u16, &str)]) -> ClientEvent {
        ClientEvent::Chat(ChatEvent::PublicChannelCatalog(
            entries
                .iter()
                .map(|(identifier, name)| PublicChannel {
                    identifier: *identifier,
                    name: (*name).to_owned(),
                })
                .collect(),
        ))
    }

    #[test]
    fn a_public_channel_keeps_its_catalogued_name() {
        let mut names = Names::default();
        assert!(names.learn(&catalog(&[(1028, "General"), (2, "Arcade")])));
        assert_eq!(names.resolve(&ChatChannel::Public(2)), "Arcade");
    }

    #[test]
    fn an_uncatalogued_channel_falls_back_to_what_core_can_derive() {
        let names = Names::default();
        assert_eq!(names.resolve(&ChatChannel::Public(4242)), "Public 4242");
    }

    #[test]
    fn a_group_takes_the_name_its_summary_carried() {
        let mut names = Names::default();
        assert!(names.learn(&ClientEvent::Chat(ChatEvent::GroupSummary {
            club_id: 7,
            name: Some("Blood Nation".to_owned()),
            kind: 0,
            category: 0,
            member: false,
            private: false,
            member_count: None,
            online: None,
        })));
        assert_eq!(names.resolve(&ChatChannel::Club(7)), "Blood Nation");
        assert_eq!(names.resolve(&ChatChannel::Club(8)), "Group 8");
    }

    #[test]
    fn joining_reports_the_name_not_just_the_identifier() {
        let mut names = Names::default();
        names.learn(&catalog(&[(1028, "General")]));
        let joined = ClientEvent::Chat(ChatEvent::Joined {
            channel_index: 0,
            channel: ChatChannel::Public(1028),
            local_member_handle: 1,
            shard_index: None,
        });
        let json = serde_json::to_string(&translate(&joined, 0, &names)).unwrap();
        assert!(json.contains(r#""name":"General""#), "{json}");
    }

    #[test]
    fn account_identity_is_part_of_the_binding_contract() {
        let account = ClientEvent::Account(AccountSummary {
            account_id: Some(42),
            battle_tag: Some("Medic#1234".to_owned()),
            region: Some(1),
            games: Some(vec!["S2".to_owned()]),
        });
        let json = serde_json::to_string(&translate(&account, 0, &Names::default())).unwrap();
        assert!(json.contains(r#""type":"account""#), "{json}");
        assert!(json.contains(r#""account_id":42"#), "{json}");
    }

    #[test]
    fn group_search_payloads_are_not_reduced_to_other() {
        let search = ClientEvent::Chat(ChatEvent::GroupSearch {
            club_ids: vec![7, 9],
        });
        let json = serde_json::to_string(&translate(&search, 0, &Names::default())).unwrap();
        assert!(json.contains(r#""type":"group_search""#), "{json}");
        assert!(json.contains(r#""club_ids":[7,9]"#), "{json}");
    }
}
