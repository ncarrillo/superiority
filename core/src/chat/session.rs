use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rand::random;

use super::{portrait_catalog, public_channels};

const CLUB_LOOKUP_BATCH: usize = 8;
const NAMED_CHANNEL_TYPE: u8 = 1;
const PARTY_CHANNEL_TYPE: u8 = 3;
const PUBLIC_CHANNEL_TYPE: u8 = 4;
const CLUB_CHANNEL_TYPE: u8 = 5;
const PARTY_UI_CHANNEL_INDEX: u8 = u8::MAX;

pub const GENERAL_PUBLIC_CHANNEL: u16 = 1028;

const JOIN_ANSWER_WINDOW: Duration = Duration::from_secs(20);

use crate::{
    Error, Result,
    bgs::fourcc,
    native::{
        AccountBlockEntry, FriendEntry, FriendIdentity, FriendsPage, ImageTableEntry, Payload,
        PresenceState, ProfileAddress, ProfileReadResponse, ProfileReadResult, Protocol, Record,
        Session, SocialOperation, ToonFullName, WhisperTarget,
        model::{
            ChatJoin, ConferenceDescriptions, MembershipKind, PartyMemberStatus, ToonHandle,
            ToonList,
        },
        presence::PresenceDirectory,
    },
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChatChannel {
    Public(u16),
    Private(String),
    Club(u32),
    Party,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatUser {
    pub handle: u32,
    pub presence_id: Option<u32>,
    pub name: Option<String>,
    pub clan_tag: Option<String>,
    pub avatar: Option<ImageTableEntry>,
    pub presence: PresenceState,
}

impl ChatUser {
    #[must_use]
    pub fn visible_name(&self) -> String {
        let name = self.name.as_deref().map_or_else(
            || format!("User {}", self.handle),
            |name| strip_character_code(name).to_owned(),
        );
        self.clan_tag
            .as_ref()
            .map_or_else(|| name.clone(), |tag| format!("<{tag}> {name}"))
    }
}

#[must_use]
pub fn strip_character_code(name: &str) -> &str {
    let Some((base, code)) = name.rsplit_once('#') else {
        return name;
    };
    if !base.is_empty() && !code.is_empty() && code.bytes().all(|byte| byte.is_ascii_digit()) {
        base
    } else {
        name
    }
}

#[must_use]
pub fn channel_title(channel: &ChatChannel) -> String {
    match channel {
        ChatChannel::Public(GENERAL_PUBLIC_CHANNEL) => "General".into(),
        ChatChannel::Public(identifier) => format!("Public {identifier}"),
        ChatChannel::Private(name) => name.clone(),
        ChatChannel::Club(club_id) => format!("Group {club_id}"),
        ChatChannel::Party => "Party".into(),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RosterMember {
    joined_order: u64,
    presence_id: Option<u32>,
    name: Option<String>,
    toon_name: Option<ToonFullName>,
    party_status: Option<PartyMemberStatus>,
    clan_tag: Option<String>,
    avatar: Option<ImageTableEntry>,
    presence: PresenceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterSnapshot {
    pub channel_index: u8,
    pub initial_complete: bool,
    pub users: Vec<ChatUser>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Observed {
    identity: bool,
    presence: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatFriend {
    pub name: String,
    pub full_name: Option<String>,
    pub note: Option<String>,
    pub avatar: Option<ImageTableEntry>,
    pub presence: PresenceState,
    pub target: WhisperTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockedAccount {
    pub account_id: u32,
    pub name: String,
    pub full_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatEvent {
    PublicChannelCatalog(Vec<public_channels::PublicChannel>),
    ConferenceDirectory {
        identifiers: Vec<u32>,
        complete: bool,
    },
    Joined {
        channel_index: u8,
        channel: ChatChannel,
        local_member_handle: u32,
        shard_index: Option<u16>,
    },
    JoinRejected {
        channel: Option<ChatChannel>,
        reason: Option<u16>,
    },
    Roster(RosterSnapshot),
    MemberJoined {
        channel_index: u8,
        user: ChatUser,
    },
    MemberLeft {
        channel_index: u8,
        user: ChatUser,
        reason: Option<u16>,
    },
    Removed {
        channel_index: u8,
        reason: Option<u16>,
    },
    Message {
        channel_index: u8,
        sender: ChatUser,
        body: String,
    },
    Whisper {
        peer: String,
        body: String,
        outgoing: bool,
    },
    Friends(Vec<ChatFriend>),
    BlockedAccounts(Vec<BlockedAccount>),
    Activity {
        route: (Option<u8>, u8),
    },
    WhisperFailed {
        peer: String,
        reason: String,
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
    },
    GroupSearch {
        club_ids: Vec<u32>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ChannelState {
    channel: Option<ChatChannel>,
    local_member_handle: Option<u32>,
    roster: BTreeMap<u32, RosterMember>,
    next_roster_order: u64,
    initial_roster_complete: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PartyChannelState {
    wire_channel_index: u8,
    state: ChannelState,
    announced: bool,
}

struct PendingLocalEcho {
    channel_index: u8,
    member_handle: u32,
    body: String,
    sent_at: Instant,
}

struct PendingWhisperEcho {
    peer: String,
    body: String,
    sent_at: Instant,
}

#[derive(Clone, Debug)]
struct PendingWhisper {
    display_name: String,
    body: String,
    lookup_name: Option<String>,
    account_id: Option<u32>,
    profile: Option<ProfileAddress>,
    toon_name: Option<ToonFullName>,
    toon_handle: Option<ToonHandle>,
    queued_at: Instant,
}

pub struct LiveChat {
    session: Session,
    protocol: Protocol,
    channels: BTreeMap<u8, ChannelState>,
    party: Option<PartyChannelState>,
    pending_joins: BTreeMap<u32, (ChatChannel, Instant)>,
    default_channel_index: u8,
    pending_events: VecDeque<ChatEvent>,
    pending_local_echoes: VecDeque<PendingLocalEcho>,
    pending_whisper_echoes: VecDeque<PendingWhisperEcho>,
    pending_whispers: VecDeque<PendingWhisper>,
    pending_toon_resolutions: BTreeSet<ToonFullName>,
    pending_club_info: Option<u32>,
    pending_invitations: BTreeMap<u32, u32>,
    party_invitations: BTreeSet<u8>,
    pending_party_accepts: BTreeSet<u8>,
    party_online_channels: BTreeSet<u8>,
    pending_club_subscription: Option<ToonHandle>,
    pending_club_lookups: Vec<Vec<u32>>,
    announced_friends: Vec<ChatFriend>,
    local_toon: Option<ToonHandle>,
    requested_whisper_friend_accounts: BTreeSet<u32>,
    requested_temporary_presences: BTreeSet<ToonHandle>,
    profiles: ProfileResolver,
    friends: BTreeMap<FriendIdentity, FriendEntry>,
    blocked_accounts: BTreeMap<u32, AccountBlockEntry>,
    next_transport_correlation_id: u32,
}

#[derive(Debug, Default)]
struct ProfileResolver {
    presence: PresenceDirectory,
    avatars: BTreeMap<ProfileAddress, Option<ImageTableEntry>>,
    queued: BTreeSet<ProfileAddress>,
    queue: VecDeque<ProfileAddress>,
    pending: BTreeMap<u32, ProfileAddress>,
    next_request_id: u32,
    queued_friend_accounts: BTreeSet<u32>,
    friend_account_queue: VecDeque<u32>,
    requested_friend_accounts: BTreeSet<u32>,
    avatar_tokens: f64,
    avatar_last_refill: Option<Instant>,
}

impl std::fmt::Debug for LiveChat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveChat")
            .field("channels", &self.channels.len())
            .field("default_channel_index", &self.default_channel_index)
            .finish_non_exhaustive()
    }
}

impl LiveChat {
    #[allow(clippy::too_many_lines)]
    pub fn establish(
        mut session: Session,
        mut initial_channel: ChatChannel,
    ) -> Result<(Self, Vec<ChatEvent>)> {
        let protocol = session.records().protocol().clone();
        request_conference_cache(&mut session, &protocol)?;

        let mut toon_selection_sent = false;
        let mut toon_selected = false;
        let mut local_toon_realm = None;
        let mut local_toon_handle = None;
        let mut directory_complete = false;
        let mut public_catalog_loaded = false;
        let mut general_channel_id = None;
        let mut channel_list_received = false;
        let mut presence_statistics_subscribed = false;
        let mut queries_sent = false;
        let mut join_sent = false;
        let mut default_channel_index = None;
        let mut channels: BTreeMap<u8, ChannelState> = BTreeMap::new();
        let mut profiles = ProfileResolver::default();
        let mut friends = BTreeMap::new();
        let mut blocked_accounts = BTreeMap::new();
        let mut events = Vec::new();

        loop {
            let record = session.records_mut().receive()?;
            let profile_changed = profiles.observe(&record.value);
            match &record.value {
                Payload::CacheStreamItems(response) => {
                    let catalog = public_channels::load(response)?;
                    let resolved_general = catalog
                        .iter()
                        .find(|channel| channel.name.eq_ignore_ascii_case("General"))
                        .map(|channel| channel.identifier)
                        .ok_or_else(|| {
                            protocol_error("Battle.net public-channel catalog omits General")
                        })?;
                    general_channel_id = Some(resolved_general);
                    if let ChatChannel::Public(identifier) = &mut initial_channel
                        && (*identifier == GENERAL_PUBLIC_CHANNEL
                            || !catalog
                                .iter()
                                .any(|channel| channel.identifier == *identifier))
                    {
                        *identifier = resolved_general;
                    }
                    if std::env::var_os("SUPERIORITY_TRACE").is_some() {
                        eprintln!(
                            "superiority: Battle.net public-channel catalog entries={} General={resolved_general}",
                            catalog.len()
                        );
                    }
                    events.push(ChatEvent::PublicChannelCatalog(catalog));
                    public_catalog_loaded = true;
                }
                Payload::ToonList(list) if !toon_selection_sent => {
                    local_toon_realm = list.displays.first().map(|toon| toon.realm);
                    select_first_toon(&mut session, &protocol, list)?;
                    toon_selection_sent = true;
                }
                Payload::ToonSelected(selected) => {
                    local_toon_handle = Some(selected.handle);
                    toon_selected = true;
                }
                Payload::ConferenceDescriptions(directory) => {
                    directory_complete |= directory.is_last;
                    events.push(conference_event(directory));
                }
                Payload::ChannelList(_) => {
                    channel_list_received = true;
                    events.push(ChatEvent::Activity {
                        route: record_route(&record),
                    });
                }
                Payload::ChatJoin(result) => {
                    if std::env::var_os("SUPERIORITY_TRACE").is_some() {
                        eprintln!(
                            "superiority: join response requested={initial_channel:?} success={} returned_name_id={:?} shard={:?} channel_index={:?}",
                            result.success,
                            result.channel_name_id,
                            result.channel_shard_index,
                            result.channel_index
                        );
                    }
                    if !result.success
                        && Some(initial_channel.clone())
                            != general_channel_id.map(ChatChannel::Public)
                    {
                        events.push(ChatEvent::JoinRejected {
                            channel: Some(initial_channel.clone()),
                            reason: result.reason,
                        });
                        let general_channel_id = general_channel_id.ok_or_else(|| {
                            protocol_error("General was not resolved from Battle.net")
                        })?;
                        initial_channel = ChatChannel::Public(general_channel_id);
                        session.records_mut().send(&protocol.chat_join_public(
                            general_channel_id,
                            random(),
                            "enUS",
                        )?)?;
                        continue;
                    }
                    validate_public_join(&initial_channel, result)?;
                    let (channel_index, member_handle) = accepted_join(result)?;
                    default_channel_index = Some(channel_index);
                    let channel = channels.entry(channel_index).or_default();
                    let joined = initial_channel.clone();
                    channel.channel = Some(joined.clone());
                    channel.local_member_handle = Some(member_handle);
                    events.push(ChatEvent::Joined {
                        channel_index,
                        channel: joined,
                        local_member_handle: member_handle,
                        shard_index: result.channel_shard_index,
                    });
                }
                Payload::ChatMembership(membership) => {
                    let channel = channels.entry(membership.channel_index).or_default();
                    let local = channel.local_member_handle;
                    apply_membership(
                        &mut channel.roster,
                        &mut channel.next_roster_order,
                        membership,
                        false,
                        local,
                        &mut events,
                    );
                    channel.initial_roster_complete |= membership.end_of_initial;
                    events.push(ChatEvent::Roster(roster_snapshot(
                        &channel.roster,
                        membership.channel_index,
                        channel.initial_roster_complete,
                    )));
                }
                Payload::ChatMessage(message) => {
                    events.push(message_event(&channels, None, message));
                }
                Payload::Friends(page) => {
                    apply_friend_page(&mut friends, page);
                    events.push(ChatEvent::Friends(friend_snapshot(&friends, &profiles)));
                }
                Payload::FriendToons(page) => {
                    apply_friend_toons(&mut friends, page);
                    events.push(ChatEvent::Friends(friend_snapshot(&friends, &profiles)));
                }
                Payload::AccountBlocks(page) => {
                    for entry in &page.entries {
                        blocked_accounts.insert(entry.account_id, entry.clone());
                    }
                    events.push(ChatEvent::BlockedAccounts(block_snapshot(
                        &blocked_accounts,
                    )));
                }
                _ => events.push(ChatEvent::Activity {
                    route: record_route(&record),
                }),
            }

            append_profile_rosters(&profiles, &mut channels, &mut events);
            if (profile_changed.identity || profile_changed.presence) && !friends.is_empty() {
                events.push(ChatEvent::Friends(friend_snapshot(&friends, &profiles)));
            }
            profiles.enqueue_missing(channels.values(), &friends);
            profiles.pump(&mut session, &protocol)?;

            if toon_selection_sent && toon_selected && !presence_statistics_subscribed {
                session
                    .records_mut()
                    .send(&protocol.presence_statistics_subscribe(true)?)?;
                presence_statistics_subscribed = true;
            }
            if toon_selection_sent && toon_selected && !queries_sent {
                session
                    .records_mut()
                    .send(&protocol.chat_enum_conference_descriptions()?)?;
                session.records_mut().send(&protocol.chat_channel_list()?)?;
                queries_sent = true;
            }
            let catalog_ready =
                !matches!(initial_channel, ChatChannel::Public(_)) || public_catalog_loaded;
            if directory_complete && channel_list_received && catalog_ready && !join_sent {
                let token = random();
                let request = match &initial_channel {
                    ChatChannel::Public(channel_name_id) => {
                        if std::env::var_os("SUPERIORITY_TRACE").is_some() {
                            eprintln!(
                                "superiority: joining public channel id={channel_name_id} locale=enUS"
                            );
                        }
                        protocol.chat_join_public(*channel_name_id, token, "enUS")?
                    }
                    ChatChannel::Private(name) => protocol.chat_join_private(name, token)?,
                    ChatChannel::Club(club_id) => protocol.chat_join_club(*club_id, token)?,
                    ChatChannel::Party => {
                        return Err(protocol_error("party channels cannot be restored directly"));
                    }
                };
                session.records_mut().send(&request)?;
                join_sent = true;
            }

            if default_channel_index.is_some_and(|index| {
                channels
                    .get(&index)
                    .is_some_and(|channel| channel.initial_roster_complete)
            }) {
                break;
            }
        }

        let default_channel_index = default_channel_index
            .ok_or_else(|| protocol_error("chat roster arrived before the join reply"))?;
        local_toon_realm.ok_or_else(|| protocol_error("selected SC2 toon omitted its realm"))?;
        Ok((
            Self {
                session,
                protocol,
                channels,
                party: None,
                pending_joins: BTreeMap::new(),
                default_channel_index,
                pending_events: VecDeque::new(),
                pending_local_echoes: VecDeque::new(),
                pending_whisper_echoes: VecDeque::new(),
                pending_whispers: VecDeque::new(),
                pending_toon_resolutions: BTreeSet::new(),
                pending_club_info: None,
                pending_invitations: BTreeMap::new(),
                party_invitations: BTreeSet::new(),
                pending_party_accepts: BTreeSet::new(),
                party_online_channels: BTreeSet::new(),
                pending_club_lookups: Vec::new(),
                pending_club_subscription: local_toon_handle,
                announced_friends: Vec::new(),
                local_toon: local_toon_handle,
                requested_whisper_friend_accounts: BTreeSet::new(),
                requested_temporary_presences: BTreeSet::new(),
                profiles,
                friends,
                blocked_accounts,
                next_transport_correlation_id: random(),
            },
            events,
        ))
    }

    #[must_use]
    pub fn roster(&self, channel_index: u8) -> Option<RosterSnapshot> {
        if channel_index == PARTY_UI_CHANNEL_INDEX {
            return self.party.as_ref().map(|party| {
                roster_snapshot(
                    &party.state.roster,
                    PARTY_UI_CHANNEL_INDEX,
                    party.state.initial_roster_complete,
                )
            });
        }
        self.channels.get(&channel_index).map(|channel| {
            roster_snapshot(
                &channel.roster,
                channel_index,
                channel.initial_roster_complete,
            )
        })
    }

    #[must_use]
    pub fn rosters(&self) -> Vec<RosterSnapshot> {
        let mut rosters = self
            .channels
            .keys()
            .filter_map(|index| self.roster(*index))
            .collect::<Vec<_>>();
        if let Some(party) = self.roster(PARTY_UI_CHANNEL_INDEX) {
            rosters.push(party);
        }
        rosters
    }

    #[must_use]
    pub const fn channel_index(&self) -> u8 {
        self.default_channel_index
    }

    fn resolve_presence_name(&self, presence_id: u32) -> Option<String> {
        let roster_name = self
            .channels
            .values()
            .chain(self.party.iter().map(|party| &party.state))
            .find_map(|channel| {
                channel.roster.values().find_map(|member| {
                    if member.presence_id != Some(presence_id) {
                        return None;
                    }
                    let name = member.name.as_deref()?;
                    let base = strip_character_code(name).to_owned();
                    Some(
                        member
                            .clan_tag
                            .as_ref()
                            .map_or_else(|| base.clone(), |tag| format!("<{tag}> {base}")),
                    )
                })
            });
        if roster_name.is_some() {
            return roster_name;
        }

        let canonical = self.profiles.presence.canonical_presence_id(presence_id);
        self.friends.values().find_map(|friend| {
            let friend_presence = friend_presence_id(friend, &self.profiles)?;
            (self
                .profiles
                .presence
                .canonical_presence_id(friend_presence)
                == canonical)
                .then(|| {
                    friend
                        .display_name
                        .as_deref()
                        .or(friend.full_name.as_deref())
                        .or_else(|| friend.toon_name.as_ref().map(|toon| toon.name.as_str()))
                        .map(strip_character_code)
                        .map(str::to_owned)
                })
                .flatten()
        })
    }

    fn party_inviter_name(&self, presence_id: u32, channel_index: u8) -> Option<String> {
        self.resolve_presence_name(presence_id).or_else(|| {
            let party = self
                .party
                .as_ref()
                .filter(|party| party.wire_channel_index == channel_index)?;
            let local = party.state.local_member_handle;
            let candidate = party.state.roster.iter().find_map(|(handle, member)| {
                (local != Some(*handle) && member.party_status != Some(PartyMemberStatus::Invited))
                    .then_some(member)
            })?;
            candidate.name.as_deref().map(|name| {
                let base = strip_character_code(name).to_owned();
                candidate
                    .clan_tag
                    .as_ref()
                    .map_or_else(|| base.clone(), |tag| format!("<{tag}> {base}"))
            })
        })
    }

    #[must_use]
    pub fn local_member_handle(&self, channel_index: u8) -> Option<u32> {
        if channel_index == PARTY_UI_CHANNEL_INDEX {
            return self
                .party
                .as_ref()
                .and_then(|party| party.state.local_member_handle);
        }
        self.channels
            .get(&channel_index)
            .and_then(|channel| channel.local_member_handle)
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.session.set_read_timeout(timeout)
    }

    pub fn join_channel(&mut self, channel: ChatChannel) -> Result<()> {
        let token = self.next_join_token();
        let request = match &channel {
            ChatChannel::Public(channel_name_id) => {
                self.protocol
                    .chat_join_public(*channel_name_id, token, "enUS")?
            }
            ChatChannel::Private(name) => self.protocol.chat_join_private(name, token)?,
            ChatChannel::Club(club_id) => self.protocol.chat_join_club(*club_id, token)?,
            ChatChannel::Party => {
                return Err(protocol_error("party channels are joined by invitation"));
            }
        };
        self.session.records_mut().send(&request)?;
        self.expire_pending_joins();
        self.pending_joins.insert(token, (channel, Instant::now()));
        Ok(())
    }

    pub fn send_message(&mut self, channel_index: u8, body: &str) -> Result<()> {
        let (wire_channel_index, channel) = if channel_index == PARTY_UI_CHANNEL_INDEX {
            let party = self
                .party
                .as_ref()
                .ok_or_else(|| protocol_error("party channel is not joined"))?;
            (party.wire_channel_index, &party.state)
        } else {
            let channel = self
                .channels
                .get(&channel_index)
                .ok_or_else(|| protocol_error("chat channel is not joined"))?;
            (channel_index, channel)
        };
        if !channel.initial_roster_complete {
            return Err(protocol_error("chat is not ready to send messages"));
        }
        let local_member_handle = channel
            .local_member_handle
            .ok_or_else(|| protocol_error("chat channel omitted the local member"))?;
        let local_member = channel.roster.get(&local_member_handle);
        let sender = ChatUser {
            handle: local_member_handle,
            presence_id: local_member.and_then(|member| member.presence_id),
            name: local_member.and_then(|member| member.name.clone()),
            clan_tag: local_member.and_then(|member| member.clan_tag.clone()),
            avatar: local_member.and_then(|member| member.avatar),
            presence: local_member.map_or(PresenceState::Unknown, |member| member.presence),
        };
        self.session
            .records_mut()
            .send(&self.protocol.chat_message(wire_channel_index, body)?)?;
        self.pending_events.push_back(ChatEvent::Message {
            channel_index,
            sender,
            body: body.to_owned(),
        });
        self.pending_local_echoes.push_back(PendingLocalEcho {
            channel_index: wire_channel_index,
            member_handle: local_member_handle,
            body: body.to_owned(),
            sent_at: Instant::now(),
        });
        while self.pending_local_echoes.len() > 32 {
            self.pending_local_echoes.pop_front();
        }
        Ok(())
    }

    pub fn send_whisper(
        &mut self,
        target: &WhisperTarget,
        display_name: &str,
        body: &str,
    ) -> Result<()> {
        if body.trim().is_empty() {
            return Err(protocol_error("whisper body cannot be empty"));
        }
        let body = body.trim();
        match target {
            WhisperTarget::Presence(presence_id) => {
                self.send_resolved_whisper(display_name, *presence_id, body)
            }
            WhisperTarget::ToonHandle(handle) => {
                self.queue_whisper(display_name, body, None, None, None, None, Some(*handle))
            }
            WhisperTarget::ToonName(name) => self.queue_whisper(
                display_name,
                body,
                None,
                None,
                None,
                Some(name.clone()),
                None,
            ),
            WhisperTarget::Account(account_id) => {
                if let Some(presence_id) = self.profiles.presence.presence_for_account(*account_id)
                {
                    return self.send_resolved_whisper(display_name, presence_id, body);
                }
                let friend = self.friends.get(&FriendIdentity::Account(*account_id));
                let profile = friend.and_then(|friend| friend.profile);
                if let Some(profile) = profile
                    && let Some(presence_id) = self.profiles.presence.presence_for_profile(profile)
                {
                    return self.send_resolved_whisper(display_name, presence_id, body);
                }
                let toon_name = friend.and_then(|friend| friend.toon_name.clone());
                self.queue_whisper(
                    display_name,
                    body,
                    None,
                    Some(*account_id),
                    profile,
                    toon_name,
                    None,
                )
            }
            WhisperTarget::Name(name) => {
                if let Some(friend) = self
                    .friends
                    .values()
                    .find(|friend| friend_matches_name(friend, name))
                    .cloned()
                {
                    let resolved = friend_whisper_target(&friend);
                    return self.send_whisper(&resolved, display_name, body);
                }
                self.queue_whisper(
                    display_name,
                    body,
                    Some(name.clone()),
                    None,
                    None,
                    None,
                    None,
                )
            }
        }
    }

    pub fn leave_channel(&mut self, channel_index: u8) -> Result<()> {
        if channel_index == PARTY_UI_CHANNEL_INDEX {
            let party = self
                .party
                .take()
                .ok_or_else(|| protocol_error("party channel is not joined"))?;
            self.session
                .records_mut()
                .send(&self.protocol.chat_leave(party.wire_channel_index)?)?;
            self.party_invitations.remove(&party.wire_channel_index);
            self.pending_party_accepts.remove(&party.wire_channel_index);
            self.party_online_channels.remove(&party.wire_channel_index);
            return Ok(());
        }
        if !self.channels.contains_key(&channel_index) {
            return Err(protocol_error("chat channel is not joined"));
        }
        self.session
            .records_mut()
            .send(&self.protocol.chat_leave(channel_index)?)?;
        self.channels.remove(&channel_index);
        self.pending_local_echoes
            .retain(|echo| echo.channel_index != channel_index);
        if self.default_channel_index == channel_index {
            self.default_channel_index = self.channels.keys().next().copied().unwrap_or(0);
        }
        Ok(())
    }

    pub fn answer_party_invitation(&mut self, channel_index: u8, accept: bool) -> Result<()> {
        let packet = self.protocol.chat_invite_answer(channel_index, accept)?;
        let members = self
            .party
            .as_ref()
            .filter(|party| party.wire_channel_index == channel_index)
            .map(|party| &party.state)
            .map(|channel| {
                channel
                    .roster
                    .iter()
                    .map(|(handle, member)| (*handle, member.party_status))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        trace_party(format_args!(
            "{} party invitation for channel {channel_index}; members={members:?}; packet={}",
            if accept { "accepting" } else { "declining" },
            hex::encode(&packet),
        ));
        self.session.records_mut().send(&packet)?;
        if accept {
            self.pending_party_accepts.insert(channel_index);
            self.publish_party_online_if_ready(channel_index)?;
        } else {
            self.party_invitations.remove(&channel_index);
            self.pending_party_accepts.remove(&channel_index);
            self.party_online_channels.remove(&channel_index);
            if self
                .party
                .as_ref()
                .is_some_and(|party| party.wire_channel_index == channel_index)
            {
                self.party = None;
            }
        }
        Ok(())
    }

    fn identify_invited_party_member(&mut self, channel_index: u8) {
        if !self.party_invitations.contains(&channel_index)
            && !self.pending_party_accepts.contains(&channel_index)
        {
            return;
        }
        let Some(channel) = self
            .party
            .as_mut()
            .filter(|party| party.wire_channel_index == channel_index)
            .map(|party| &mut party.state)
        else {
            return;
        };
        if channel.local_member_handle.is_some() {
            return;
        }
        let invited = channel
            .roster
            .iter()
            .filter(|(_, member)| member.party_status == Some(PartyMemberStatus::Invited))
            .map(|(handle, _)| *handle)
            .collect::<Vec<_>>();
        if let [member_handle] = invited.as_slice() {
            channel.local_member_handle = Some(*member_handle);
            trace_party(format_args!(
                "identified invited local party member {member_handle} on channel {channel_index}"
            ));
        }
    }

    fn publish_party_online_if_ready(&mut self, channel_index: u8) -> Result<bool> {
        if !self.pending_party_accepts.contains(&channel_index)
            || self.party_online_channels.contains(&channel_index)
        {
            return Ok(false);
        }
        self.identify_invited_party_member(channel_index);
        let Some(member_handle) = self
            .party
            .as_ref()
            .filter(|party| party.wire_channel_index == channel_index)
            .and_then(|party| party.state.local_member_handle)
        else {
            trace_party(format_args!(
                "accepted party channel {channel_index}; waiting for the local member handle"
            ));
            return Ok(false);
        };
        let packet = self
            .protocol
            .chat_party_online(channel_index, member_handle)?;
        self.session.records_mut().send(&packet)?;
        self.party_online_channels.insert(channel_index);
        self.pending_party_accepts.remove(&channel_index);
        self.party_invitations.remove(&channel_index);
        trace_party(format_args!(
            "published party online status for member {member_handle} on channel {channel_index}"
        ));
        Ok(true)
    }

    pub fn keep_alive(&mut self) -> Result<()> {
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| protocol_error("system clock is before the Unix epoch"))?
            .as_micros();
        let micros = i64::try_from(micros)
            .map_err(|_| protocol_error("system clock exceeds native ping range"))?;
        self.session
            .records_mut()
            .send(&self.protocol.ping(micros)?)
    }

    pub fn maintain_transport(&mut self) -> Result<()> {
        let records = self
            .protocol
            .transport_control_maintenance(self.next_transport_correlation_id)?;
        self.next_transport_correlation_id = self.next_transport_correlation_id.wrapping_sub(2);
        for record in records {
            self.session.records_mut().send(&record)?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match over the inbound record types"
    )]
    pub fn receive(&mut self) -> Result<ChatEvent> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(event);
        }
        self.request_clubs_when_idle()?;

        let record = self.session.records_mut().receive()?;
        let observed = self.profiles.observe(&record.value);
        let event = match &record.value {
            Payload::ChatMembership(membership) => {
                let is_party = self.party_invitations.contains(&membership.channel_index)
                    || self
                        .pending_party_accepts
                        .contains(&membership.channel_index)
                    || self
                        .party
                        .as_ref()
                        .is_some_and(|party| party.wire_channel_index == membership.channel_index)
                    || membership
                        .changes
                        .iter()
                        .any(|change| change.party_status.is_some());
                if is_party {
                    trace_party(format_args!(
                        "party membership channel {} end={} changes={:?}",
                        membership.channel_index,
                        membership.end_of_initial,
                        membership
                            .changes
                            .iter()
                            .map(|change| (
                                change.kind,
                                change.member_handle,
                                change.presence_id,
                                change.party_status
                            ))
                            .collect::<Vec<_>>()
                    ));
                }
                let mut events = Vec::new();
                if is_party {
                    let party = self.party.get_or_insert_with(|| PartyChannelState {
                        wire_channel_index: membership.channel_index,
                        ..PartyChannelState::default()
                    });
                    let was_complete = party.state.initial_roster_complete;
                    let local = party.state.local_member_handle;
                    let mut visible = membership.clone();
                    visible.channel_index = PARTY_UI_CHANNEL_INDEX;
                    apply_membership(
                        &mut party.state.roster,
                        &mut party.state.next_roster_order,
                        &visible,
                        was_complete,
                        local,
                        &mut events,
                    );
                    party.state.initial_roster_complete |= membership.end_of_initial;
                } else {
                    let channel = self.channels.entry(membership.channel_index).or_default();
                    let was_complete = channel.initial_roster_complete;
                    let local = channel.local_member_handle;
                    apply_membership(
                        &mut channel.roster,
                        &mut channel.next_roster_order,
                        membership,
                        was_complete,
                        local,
                        &mut events,
                    );
                    channel.initial_roster_complete |= membership.end_of_initial;
                }
                self.identify_invited_party_member(membership.channel_index);
                self.publish_party_online_if_ready(membership.channel_index)?;
                let (visible_index, channel) = if is_party {
                    let party = self
                        .party
                        .as_mut()
                        .expect("party channel was just inserted");
                    if self
                        .party_online_channels
                        .contains(&membership.channel_index)
                        && !party.announced
                    {
                        let local_member_handle =
                            party.state.local_member_handle.ok_or_else(|| {
                                protocol_error("party channel omitted the local member")
                            })?;
                        events.insert(
                            0,
                            ChatEvent::Joined {
                                channel_index: PARTY_UI_CHANNEL_INDEX,
                                channel: ChatChannel::Party,
                                local_member_handle,
                                shard_index: None,
                            },
                        );
                        party.announced = true;
                    }
                    (PARTY_UI_CHANNEL_INDEX, &party.state)
                } else {
                    (
                        membership.channel_index,
                        self.channels
                            .get(&membership.channel_index)
                            .expect("membership channel was just inserted"),
                    )
                };
                events.push(ChatEvent::Roster(roster_snapshot(
                    &channel.roster,
                    visible_index,
                    channel.initial_roster_complete,
                )));
                let mut events = events.into_iter();
                let event = events
                    .next()
                    .ok_or_else(|| protocol_error("membership produced no chat event"))?;
                self.pending_events.extend(events);
                Ok::<ChatEvent, Error>(event)
            }
            Payload::ChatMessage(message) => {
                if self.consume_local_echo(message) {
                    Ok(ChatEvent::Activity {
                        route: record_route(&record),
                    })
                } else {
                    Ok(message_event(&self.channels, self.party.as_ref(), message))
                }
            }
            Payload::ChatWhisper(whisper) => Ok(ChatEvent::Whisper {
                peer: strip_character_code(&whisper.peer.name).to_owned(),
                body: whisper.body.clone(),
                outgoing: false,
            }),
            Payload::ChatWhisperEcho(whisper) => {
                if self.consume_whisper_echo(whisper) {
                    Ok(ChatEvent::Activity {
                        route: record_route(&record),
                    })
                } else {
                    Ok(ChatEvent::Whisper {
                        peer: strip_character_code(&whisper.peer.name).to_owned(),
                        body: whisper.body.clone(),
                        outgoing: true,
                    })
                }
            }
            Payload::ChatJoin(result) => {
                self.expire_pending_joins();
                trace_party(format_args!(
                    "chat join success={} index={:?} member={:?} type={:?} locator={:?} shard={:?} token={:?}",
                    result.success,
                    result.channel_index,
                    result.member_handle,
                    result.channel_type,
                    result.channel_name_id,
                    result.channel_shard_index,
                    result.token,
                ));
                let rejoins_party = result.success
                    && result.channel_name_id.is_none()
                    && result.token.is_none()
                    && self.party.as_ref().is_some_and(|party| {
                        Some(party.wire_channel_index) == result.channel_index
                    });
                if result.success
                    && (result.channel_type == Some(PARTY_CHANNEL_TYPE) || rejoins_party)
                {
                    let was_announced = self.party.as_ref().is_some_and(|party| party.announced);
                    let (channel_index, local_member_handle) =
                        accept_party_join(&mut self.party, result)?;
                    self.party_invitations.remove(&channel_index);
                    self.pending_party_accepts.insert(channel_index);
                    self.publish_party_online_if_ready(channel_index)?;
                    self.party
                        .as_mut()
                        .expect("accepted party channel exists")
                        .announced = true;
                    if was_announced {
                        Ok(ChatEvent::Activity {
                            route: record_route(&record),
                        })
                    } else {
                        Ok(ChatEvent::Joined {
                            channel_index: PARTY_UI_CHANNEL_INDEX,
                            channel: ChatChannel::Party,
                            local_member_handle,
                            shard_index: None,
                        })
                    }
                } else {
                    let requested = take_requested_join(&mut self.pending_joins, result)?;
                    if !result.success {
                        return Ok(ChatEvent::JoinRejected {
                            channel: requested,
                            reason: result.reason,
                        });
                    }
                    if let Some(requested) = &requested {
                        validate_public_join(requested, result)?;
                    }
                    let (channel_index, local_member_handle) = accepted_join(result)?;
                    let joined = requested
                        .or_else(|| result.channel_name_id.map(ChatChannel::Public))
                        .or_else(|| {
                            self.channels
                                .get(&channel_index)
                                .and_then(|channel| channel.channel.clone())
                        });
                    let channel = self.channels.entry(channel_index).or_default();
                    channel.local_member_handle = Some(local_member_handle);
                    if let Some(joined) = joined {
                        channel.channel = Some(joined.clone());
                        Ok(ChatEvent::Joined {
                            channel_index,
                            channel: joined,
                            local_member_handle,
                            shard_index: result.channel_shard_index,
                        })
                    } else {
                        trace_party(format_args!(
                            "accepted auxiliary chat channel {channel_index} without a visible locator"
                        ));
                        Ok(ChatEvent::Activity {
                            route: record_route(&record),
                        })
                    }
                }
            }
            Payload::ChatInvite(invite) => {
                self.party.get_or_insert_with(|| PartyChannelState {
                    wire_channel_index: invite.channel_index,
                    ..PartyChannelState::default()
                });
                self.party_invitations.insert(invite.channel_index);
                self.party_online_channels.remove(&invite.channel_index);
                self.identify_invited_party_member(invite.channel_index);
                trace_party(format_args!(
                    "received party invitation type {} for channel {}; local member {:?}",
                    invite.channel_type,
                    invite.channel_index,
                    self.channels
                        .get(&invite.channel_index)
                        .and_then(|channel| channel.local_member_handle)
                ));
                Ok(ChatEvent::PartyInvitation {
                    inviter: self.party_inviter_name(invite.inviter_presence, invite.channel_index),
                    channel_index: invite.channel_index,
                })
            }
            Payload::ClubInviteAction(action) => {
                let token = action.club_id;
                self.pending_club_info = Some(action.club_id);
                self.pending_invitations
                    .insert(action.club_id, action.program);
                if let Ok(packet) = self
                    .session
                    .records()
                    .protocol()
                    .club_info_request(token, &[action.club_id])
                {
                    let _ = self.session.records_mut().send(&packet);
                }
                Ok(ChatEvent::GroupInvitation {
                    club_id: action.club_id,
                })
            }
            Payload::ClubSummaries(clubs) => {
                let mut summaries = clubs.iter().map(|club| ChatEvent::GroupSummary {
                    club_id: club.club_id,
                    name: club.name.clone(),
                    kind: club.kind,
                    category: club.category,
                    private: club.private,
                    member: true,
                });
                let first = summaries.next().unwrap_or(ChatEvent::Activity {
                    route: record_route(&record),
                });
                self.pending_events.extend(summaries);
                Ok(first)
            }
            Payload::ClubSearch(club_ids) => {
                self.pending_club_lookups = club_ids
                    .chunks(CLUB_LOOKUP_BATCH)
                    .map(<[u32]>::to_vec)
                    .rev()
                    .collect();
                self.request_next_club_lookup();
                Ok(ChatEvent::GroupSearch {
                    club_ids: club_ids.clone(),
                })
            }
            Payload::ClubInfo(clubs) => {
                self.pending_club_info.take();
                self.request_next_club_lookup();
                let mut summaries = clubs.iter().map(|club| ChatEvent::GroupSummary {
                    club_id: club.club_id,
                    name: club.name.clone(),
                    kind: club.kind,
                    category: club.category,
                    private: club.private,
                    member: false,
                });
                let first = summaries.next().unwrap_or(ChatEvent::Activity {
                    route: record_route(&record),
                });
                self.pending_events.extend(summaries);
                Ok(first)
            }
            Payload::ConferenceDescriptions(directory) => Ok(conference_event(directory)),
            Payload::Friends(page) => {
                apply_friend_page(&mut self.friends, page);
                Ok(self.friends_event())
            }
            Payload::FriendToons(page) => {
                apply_friend_toons(&mut self.friends, page);
                Ok(self.friends_event())
            }
            Payload::AccountBlocks(page) => {
                for entry in &page.entries {
                    self.blocked_accounts
                        .insert(entry.account_id, entry.clone());
                }
                Ok(ChatEvent::BlockedAccounts(block_snapshot(
                    &self.blocked_accounts,
                )))
            }
            _ => Ok(ChatEvent::Activity {
                route: record_route(&record),
            }),
        }?;
        self.advance_pending_whispers(&record.value)?;

        for channel_index in self.profiles.synchronize(&mut self.channels) {
            let channel = self
                .channels
                .get(&channel_index)
                .expect("synchronized channel exists");
            self.pending_events
                .push_back(ChatEvent::Roster(roster_snapshot(
                    &channel.roster,
                    channel_index,
                    channel.initial_roster_complete,
                )));
        }
        if let Some(party) = self.party.as_mut()
            && self.profiles.synchronize_channel(&mut party.state)
        {
            self.pending_events
                .push_back(ChatEvent::Roster(roster_snapshot(
                    &party.state.roster,
                    PARTY_UI_CHANNEL_INDEX,
                    party.state.initial_roster_complete,
                )));
        }
        self.profiles.enqueue_missing(
            self.channels
                .values()
                .chain(self.party.iter().map(|party| &party.state)),
            &self.friends,
        );
        self.profiles.pump(&mut self.session, &self.protocol)?;
        if (observed.identity || observed.presence) && !self.friends.is_empty() {
            let snapshot = friend_snapshot(&self.friends, &self.profiles);
            if snapshot != self.announced_friends {
                self.announced_friends.clone_from(&snapshot);
                let friends = ChatEvent::Friends(snapshot);
                if matches!(event, ChatEvent::Activity { .. }) {
                    return Ok(friends);
                }
                self.pending_events.push_back(friends);
            }
        }
        Ok(event)
    }

    fn friends_event(&mut self) -> ChatEvent {
        let snapshot = friend_snapshot(&self.friends, &self.profiles);
        self.announced_friends.clone_from(&snapshot);
        ChatEvent::Friends(snapshot)
    }

    pub fn close(self) -> Result<()> {
        self.session.close()
    }

    fn expire_pending_joins(&mut self) {
        self.pending_joins
            .retain(|_, (_, asked)| asked.elapsed() < JOIN_ANSWER_WINDOW);
    }

    fn next_join_token(&self) -> u32 {
        loop {
            let token = random();
            if !self.pending_joins.contains_key(&token) {
                return token;
            }
        }
    }

    fn send_resolved_whisper(
        &mut self,
        display_name: &str,
        presence_id: u32,
        body: &str,
    ) -> Result<()> {
        let record = self
            .protocol
            .chat_whisper(&WhisperTarget::Presence(presence_id), body)?;
        self.session.records_mut().send(&record)?;
        self.pending_events.push_back(ChatEvent::Whisper {
            peer: display_name.to_owned(),
            body: body.to_owned(),
            outgoing: true,
        });
        self.pending_whisper_echoes.push_back(PendingWhisperEcho {
            peer: display_name.to_owned(),
            body: body.to_owned(),
            sent_at: Instant::now(),
        });
        while self.pending_whisper_echoes.len() > 32 {
            self.pending_whisper_echoes.pop_front();
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the pending identity can be represented at several resolution stages"
    )]
    fn queue_whisper(
        &mut self,
        display_name: &str,
        body: &str,
        lookup_name: Option<String>,
        account_id: Option<u32>,
        profile: Option<ProfileAddress>,
        toon_name: Option<ToonFullName>,
        toon_handle: Option<ToonHandle>,
    ) -> Result<()> {
        if let Some(handle) = toon_handle
            && let Some(presence_id) = self.profiles.presence.presence_for_toon_handle(handle)
        {
            return self.send_resolved_whisper(display_name, presence_id, body);
        }
        self.pending_whispers.push_back(PendingWhisper {
            display_name: display_name.to_owned(),
            body: body.to_owned(),
            lookup_name,
            account_id,
            profile,
            toon_name: toon_name.clone(),
            toon_handle,
            queued_at: Instant::now(),
        });
        if let Some(handle) = toon_handle {
            self.request_temporary_presence(handle)?;
        } else if let Some(name) = toon_name
            && self.pending_toon_resolutions.insert(name.clone())
        {
            self.session
                .records_mut()
                .send(&self.protocol.resolve_toon_name(&name)?)?;
        }
        if let Some(account_id) = account_id
            && self.requested_whisper_friend_accounts.insert(account_id)
        {
            self.session
                .records_mut()
                .send(&self.protocol.friend_toons(account_id)?)?;
        }
        Ok(())
    }

    fn request_next_club_lookup(&mut self) {
        let Some(batch) = self.pending_club_lookups.pop() else {
            return;
        };
        if let Ok(packet) = self
            .session
            .records()
            .protocol()
            .club_info_request(0, &batch)
        {
            let _ = self.session.records_mut().send(&packet);
        }
    }

    pub fn search_groups(&mut self, query: &str) -> Result<()> {
        let packet = self.protocol.club_search_request(0, query)?;
        self.session.records_mut().send(&packet)
    }

    pub fn answer_group_invitation(&mut self, club_id: u32, accept: bool) -> Result<()> {
        const ACCEPTED: u8 = 1;
        const DECLINED: u8 = 2;
        let code = if accept { ACCEPTED } else { DECLINED };
        let program = self
            .pending_invitations
            .remove(&club_id)
            .ok_or_else(|| protocol_error("that group invitation is no longer pending"))?;
        let packet = Protocol::club_invite_answer(program, club_id, code)?;
        self.session.records_mut().send(&packet)?;
        if accept
            && let Some(handle) = self.local_toon
            && self.request_clubs(handle).is_err()
        {
            self.pending_club_subscription = Some(handle);
        }
        Ok(())
    }

    fn request_clubs_when_idle(&mut self) -> Result<()> {
        if self.session.records_mut().buffered_bytes() != 0 {
            return Ok(());
        }
        let Some(handle) = self.pending_club_subscription.take() else {
            return Ok(());
        };
        self.request_clubs(handle)
    }

    fn request_clubs(&mut self, handle: ToonHandle) -> Result<()> {
        let packet = self.protocol.club_subscribe(0, handle)?;
        self.session.records_mut().send(&packet)
    }

    fn request_temporary_presence(&mut self, handle: ToonHandle) -> Result<()> {
        if self.requested_temporary_presences.insert(handle) {
            let record = self.protocol.temporary_presence(&[handle])?;
            self.session.records_mut().send(&record)?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "identity resolution advances every pending representation in one pass"
    )]
    fn advance_pending_whispers(&mut self, payload: &Payload) -> Result<()> {
        for pending in &mut self.pending_whispers {
            if pending.account_id.is_none()
                && pending.toon_name.is_none()
                && pending.toon_handle.is_none()
                && let Some(query) = pending.lookup_name.as_deref()
                && let Some(friend) = self
                    .friends
                    .values()
                    .find(|friend| friend_matches_name(friend, query))
            {
                match friend.identity {
                    FriendIdentity::Account(account_id) => {
                        pending.account_id = Some(account_id);
                        pending.profile = friend.profile;
                        pending.toon_name.clone_from(&friend.toon_name);
                    }
                    FriendIdentity::Character {
                        program_id,
                        region,
                        realm,
                        id,
                    } => {
                        pending.toon_handle = Some(ToonHandle {
                            region: u8::try_from(region).unwrap_or(1),
                            program_id,
                            realm,
                            id,
                        });
                    }
                }
            }

            if pending.toon_name.is_none()
                && let Some(account_id) = pending.account_id
                && let Some(friend) = self.friends.get(&FriendIdentity::Account(account_id))
            {
                pending.profile = friend.profile;
                pending.toon_name.clone_from(&friend.toon_name);
            }
        }

        if let Payload::ToonNameResolved(resolved) = payload {
            self.pending_toon_resolutions.remove(&resolved.name);
            if resolved.result == 0
                && let Some(handle) = resolved.handle
            {
                for pending in &mut self.pending_whispers {
                    if pending.toon_name.as_ref() == Some(&resolved.name) {
                        pending.toon_handle = Some(handle);
                    }
                }
                self.request_temporary_presence(handle)?;
            }
        }

        let unresolved_accounts = self
            .pending_whispers
            .iter()
            .filter(|pending| pending.toon_name.is_none() && pending.toon_handle.is_none())
            .filter_map(|pending| pending.account_id)
            .collect::<BTreeSet<_>>();
        for account_id in unresolved_accounts {
            if self.requested_whisper_friend_accounts.insert(account_id) {
                self.session
                    .records_mut()
                    .send(&self.protocol.friend_toons(account_id)?)?;
            }
        }

        let unresolved_toon_names = self
            .pending_whispers
            .iter()
            .filter(|pending| pending.toon_handle.is_none())
            .filter_map(|pending| pending.toon_name.clone())
            .collect::<BTreeSet<_>>();
        for name in unresolved_toon_names {
            if self.pending_toon_resolutions.insert(name.clone()) {
                self.session
                    .records_mut()
                    .send(&self.protocol.resolve_toon_name(&name)?)?;
            }
        }

        let unresolved_handles = self
            .pending_whispers
            .iter()
            .filter_map(|pending| pending.toon_handle)
            .collect::<BTreeSet<_>>();
        for handle in unresolved_handles {
            self.request_temporary_presence(handle)?;
        }

        let mut index = 0;
        while index < self.pending_whispers.len() {
            const RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);
            if self.pending_whispers[index].queued_at.elapsed() >= RESOLUTION_TIMEOUT {
                let pending = self
                    .pending_whispers
                    .remove(index)
                    .expect("pending whisper index exists");
                self.pending_events.push_back(ChatEvent::WhisperFailed {
                    peer: pending.display_name,
                    reason: "Battle.net identity could not be resolved".to_owned(),
                });
                continue;
            }
            if let Some(account_id) = self.pending_whispers[index].account_id
                && let Some(presence_id) = self.profiles.presence.presence_for_account(account_id)
            {
                let pending = self
                    .pending_whispers
                    .remove(index)
                    .expect("pending whisper index exists");
                self.send_resolved_whisper(&pending.display_name, presence_id, &pending.body)?;
                continue;
            }
            if let Some(profile) = self.pending_whispers[index].profile
                && let Some(presence_id) = self.profiles.presence.presence_for_profile(profile)
            {
                let pending = self
                    .pending_whispers
                    .remove(index)
                    .expect("pending whisper index exists");
                self.send_resolved_whisper(&pending.display_name, presence_id, &pending.body)?;
                continue;
            }
            let Some(handle) = self.pending_whispers[index].toon_handle else {
                index += 1;
                continue;
            };
            let Some(presence_id) = self.profiles.presence.presence_for_toon_handle(handle) else {
                index += 1;
                continue;
            };
            let pending = self
                .pending_whispers
                .remove(index)
                .expect("pending whisper index exists");
            self.send_resolved_whisper(&pending.display_name, presence_id, &pending.body)?;
            self.requested_temporary_presences.remove(&handle);
        }
        Ok(())
    }

    fn consume_local_echo(&mut self, message: &crate::native::model::ChatMessage) -> bool {
        const ECHO_WINDOW: Duration = Duration::from_secs(10);
        while self
            .pending_local_echoes
            .front()
            .is_some_and(|echo| echo.sent_at.elapsed() > ECHO_WINDOW)
        {
            self.pending_local_echoes.pop_front();
        }
        let Some(index) = self.pending_local_echoes.iter().position(|echo| {
            echo.channel_index == message.channel_index
                && echo.member_handle == message.member_handle
                && echo.body == message.body
        }) else {
            return false;
        };
        self.pending_local_echoes.remove(index);
        true
    }

    fn consume_whisper_echo(&mut self, whisper: &crate::native::model::ChatWhisper) -> bool {
        const ECHO_WINDOW: Duration = Duration::from_secs(10);
        while self
            .pending_whisper_echoes
            .front()
            .is_some_and(|echo| echo.sent_at.elapsed() > ECHO_WINDOW)
        {
            self.pending_whisper_echoes.pop_front();
        }
        let peer = strip_character_code(&whisper.peer.name);
        let Some(index) = self
            .pending_whisper_echoes
            .iter()
            .position(|echo| echo.peer.eq_ignore_ascii_case(peer) && echo.body == whisper.body)
        else {
            return false;
        };
        self.pending_whisper_echoes.remove(index);
        true
    }
}

fn apply_friend_page(friends: &mut BTreeMap<FriendIdentity, FriendEntry>, page: &FriendsPage) {
    for update in &page.updates {
        match update.operation {
            SocialOperation::Remove => {
                friends.remove(&update.entry.identity);
            }
            SocialOperation::Add | SocialOperation::Modify => {
                if let Some(existing) = friends.get_mut(&update.entry.identity) {
                    if update.entry.display_name.is_some() {
                        existing.display_name.clone_from(&update.entry.display_name);
                    }
                    if update.entry.full_name.is_some() {
                        existing.full_name.clone_from(&update.entry.full_name);
                    }
                    if update.entry.note.is_some() {
                        existing.note.clone_from(&update.entry.note);
                    }
                    if update.entry.profile.is_some() {
                        existing.profile = update.entry.profile;
                    }
                    if update.entry.toon_name.is_some() {
                        existing.toon_name.clone_from(&update.entry.toon_name);
                    }
                } else {
                    friends.insert(update.entry.identity.clone(), update.entry.clone());
                }
            }
        }
    }
}

fn friend_matches_name(friend: &FriendEntry, query: &str) -> bool {
    [friend.display_name.as_deref(), friend.full_name.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .any(|name| {
            name.eq_ignore_ascii_case(query)
                || strip_character_code(name).eq_ignore_ascii_case(query)
                || name
                    .split_whitespace()
                    .next()
                    .is_some_and(|first| first.eq_ignore_ascii_case(query))
        })
}

fn friend_whisper_target(friend: &FriendEntry) -> WhisperTarget {
    match friend.identity {
        FriendIdentity::Account(account_id) => WhisperTarget::Account(account_id),
        FriendIdentity::Character {
            program_id,
            region,
            realm,
            id,
        } => WhisperTarget::ToonHandle(ToonHandle {
            region: u8::try_from(region).unwrap_or(1),
            program_id,
            realm,
            id,
        }),
    }
}

fn apply_friend_toons(
    friends: &mut BTreeMap<FriendIdentity, FriendEntry>,
    page: &crate::native::FriendToonPage,
) {
    for toon in &page.entries {
        if toon.program_id != fourcc("S2") {
            continue;
        }
        if let Some(friend) = friends.get_mut(&FriendIdentity::Account(toon.account_id)) {
            if toon.profile.is_some() {
                friend.profile = toon.profile;
            }
            friend.toon_name = Some(toon.toon_name.clone());
        }
    }
}

fn friend_snapshot(
    friends: &BTreeMap<FriendIdentity, FriendEntry>,
    profiles: &ProfileResolver,
) -> Vec<ChatFriend> {
    let mut unique = BTreeMap::new();
    for friend in friends.values() {
        let name = friend
            .display_name
            .as_deref()
            .or(friend.full_name.as_deref())
            .map(str::trim)
            .filter(|name| !name.is_empty());
        let Some(name) = name else {
            continue;
        };
        let avatar = friend
            .profile
            .and_then(|address| profiles.avatars.get(&address).copied().flatten());
        let presence_id = friend_presence_id(friend, profiles);
        let presence = presence_id.map_or(PresenceState::Offline, |presence_id| {
            profiles.presence.identity(presence_id).state
        });
        let target = presence_id.map_or_else(
            || match (&friend.toon_name, &friend.identity) {
                (Some(name), _) => WhisperTarget::ToonName(name.clone()),
                (None, FriendIdentity::Account(account_id)) => WhisperTarget::Account(*account_id),
                (
                    None,
                    FriendIdentity::Character {
                        program_id,
                        region,
                        realm,
                        id,
                    },
                ) => WhisperTarget::ToonHandle(ToonHandle {
                    region: u8::try_from(*region).unwrap_or(1),
                    program_id: *program_id,
                    realm: *realm,
                    id: *id,
                }),
            },
            WhisperTarget::Presence,
        );
        unique
            .entry(name.to_lowercase())
            .and_modify(|existing: &mut ChatFriend| {
                if existing.avatar.is_none() {
                    existing.avatar = avatar;
                }
                if matches!(
                    existing.presence,
                    PresenceState::Offline | PresenceState::Unknown
                ) && !matches!(presence, PresenceState::Offline | PresenceState::Unknown)
                {
                    existing.presence = presence;
                    existing.target = target.clone();
                }
            })
            .or_insert_with(|| ChatFriend {
                name: name.to_owned(),
                full_name: friend.full_name.clone(),
                note: friend.note.clone(),
                avatar,
                presence,
                target,
            });
    }
    unique.into_values().collect()
}

fn friend_presence_id(friend: &FriendEntry, profiles: &ProfileResolver) -> Option<u32> {
    let identity_presence = match friend.identity {
        FriendIdentity::Account(account_id) => profiles.presence.presence_for_account(account_id),
        FriendIdentity::Character {
            program_id,
            region,
            realm,
            id,
        } => profiles.presence.presence_for_toon_handle(ToonHandle {
            region: u8::try_from(region).unwrap_or(1),
            program_id,
            realm,
            id,
        }),
    };
    identity_presence.or_else(|| {
        friend
            .profile
            .and_then(|profile| profiles.presence.presence_for_profile(profile))
    })
}

fn block_snapshot(blocks: &BTreeMap<u32, AccountBlockEntry>) -> Vec<BlockedAccount> {
    let mut blocks = blocks
        .values()
        .map(|entry| BlockedAccount {
            account_id: entry.account_id,
            name: entry
                .nickname
                .clone()
                .or_else(|| entry.full_name.clone())
                .unwrap_or_else(|| format!("Account #{}", entry.account_id)),
            full_name: entry.full_name.clone(),
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|entry| entry.name.to_lowercase());
    blocks
}

impl ProfileResolver {
    const AVATAR_PATH: [u8; 1] = [0x14];

    fn observe(&mut self, payload: &Payload) -> Observed {
        let mut identity_changed = false;
        let mut presence_changed = false;
        match payload {
            Payload::PresenceFields(fields) => {
                self.presence.announce(fields);
            }
            Payload::PresenceUpdate(update) => {
                let before = (
                    self.presence.identity(update.local_presence_id).state,
                    self.presence.identity(update.master_presence_id).state,
                );
                if let Ok(identity) = self.presence.apply(update) {
                    identity_changed = identity.account_id.is_some();
                }
                presence_changed = before
                    != (
                        self.presence.identity(update.local_presence_id).state,
                        self.presence.identity(update.master_presence_id).state,
                    );
            }
            Payload::ProfileRead(response) => identity_changed = self.receive(response),
            _ => {}
        }
        Observed {
            identity: identity_changed,
            presence: presence_changed,
        }
    }

    fn synchronize(&self, channels: &mut BTreeMap<u8, ChannelState>) -> Vec<u8> {
        let mut changed = Vec::new();
        for (channel_index, channel) in channels {
            if self.synchronize_channel(channel) {
                changed.push(*channel_index);
            }
        }
        changed
    }

    fn synchronize_channel(&self, channel: &mut ChannelState) -> bool {
        let mut changed = false;
        for member in channel.roster.values_mut() {
            let Some(presence_id) = member.presence_id else {
                continue;
            };
            let identity = self.presence.identity(presence_id);
            let local_presence_id = self.presence.local_presence_id(presence_id);
            if local_presence_id != presence_id {
                member.presence_id = Some(local_presence_id);
                changed = true;
            }
            let avatar = identity.avatar.or_else(|| {
                identity
                    .profile
                    .and_then(|address| self.avatars.get(&address).copied().flatten())
            });
            if member.avatar != avatar {
                member.avatar = avatar;
                changed = true;
            }
            if member.presence != identity.state {
                member.presence = identity.state;
                changed = true;
            }
            if member.clan_tag != identity.clan_tag {
                member.clan_tag = identity.clan_tag;
                changed = true;
            }
        }
        changed
    }

    fn enqueue_missing<'a>(
        &mut self,
        channels: impl IntoIterator<Item = &'a ChannelState>,
        friends: &BTreeMap<FriendIdentity, FriendEntry>,
    ) {
        for member in channels
            .into_iter()
            .flat_map(|channel| channel.roster.values())
        {
            let Some(presence_id) = member.presence_id else {
                continue;
            };
            let identity = self.presence.identity(presence_id);
            let Some(address) = identity.profile else {
                continue;
            };
            if identity.avatar.is_some() {
                continue;
            }
            self.enqueue(address);
        }
        for friend in friends.values() {
            if let Some(address) = friend.profile {
                self.enqueue(address);
            }
            let FriendIdentity::Account(account_id) = friend.identity else {
                continue;
            };
            if friend.toon_name.is_some() {
                continue;
            }
            if self.requested_friend_accounts.contains(&account_id)
                || !self.queued_friend_accounts.insert(account_id)
            {
                continue;
            }
            self.friend_account_queue.push_back(account_id);
        }
    }

    fn enqueue(&mut self, address: ProfileAddress) {
        if self.avatars.contains_key(&address)
            || self.queued.contains(&address)
            || self.pending.values().any(|pending| *pending == address)
        {
            return;
        }
        self.queued.insert(address);
        self.queue.push_back(address);
    }

    fn pump(&mut self, session: &mut Session, protocol: &Protocol) -> Result<()> {
        const MAX_IN_FLIGHT: usize = 8;
        const AVATAR_BURST: f64 = 10.0;
        const AVATAR_RATE_PER_SEC: f64 = 20.0;

        while let Some(account_id) = self.friend_account_queue.pop_front() {
            self.queued_friend_accounts.remove(&account_id);
            if !self.requested_friend_accounts.insert(account_id) {
                continue;
            }
            session
                .records_mut()
                .send(&protocol.friend_toons(account_id)?)?;
        }

        let now = Instant::now();
        match self.avatar_last_refill {
            None => {
                self.avatar_tokens = AVATAR_BURST;
            }
            Some(last) => {
                let elapsed = now.duration_since(last).as_secs_f64();
                self.avatar_tokens =
                    (self.avatar_tokens + elapsed * AVATAR_RATE_PER_SEC).min(AVATAR_BURST);
            }
        }
        self.avatar_last_refill = Some(now);
        while self.pending.len() < MAX_IN_FLIGHT && self.avatar_tokens >= 1.0 {
            let Some(address) = self.queue.pop_front() else {
                break;
            };
            self.avatar_tokens -= 1.0;
            self.queued.remove(&address);
            let request_id = self.next_request_id;
            self.next_request_id = self.next_request_id.wrapping_add(1);
            session.records_mut().send(&protocol.profile_read(
                request_id,
                address,
                &Self::AVATAR_PATH,
            )?)?;
            self.pending.insert(request_id, address);
        }
        Ok(())
    }

    fn receive(&mut self, response: &ProfileReadResponse) -> bool {
        let Some(address) = self.pending.get(&response.request_id).copied() else {
            return false;
        };
        match &response.result {
            ProfileReadResult::Start {
                packet_count: 0, ..
            }
            | ProfileReadResult::Failure(_)
            | ProfileReadResult::Cache => {
                self.pending.remove(&response.request_id);
                self.avatars.insert(address, None);
                true
            }
            ProfileReadResult::Start { .. } => false,
            ProfileReadResult::Block(block) => {
                self.pending.remove(&response.request_id);
                let avatar = decode_avatar_block(block);
                self.avatars.insert(address, avatar);
                true
            }
        }
    }
}

fn decode_avatar_block(block: &[u8]) -> Option<ImageTableEntry> {
    let offset = block
        .windows(8)
        .position(|entry| entry == [6, 0x14, 0xf0, b'P', b'O', b'R', b'T', 1])?;
    let unlockable_id = decode_profile_uint(block.get(offset + 8..)?)?;
    portrait_catalog::image_for_unlockable(unlockable_id)
}

fn decode_profile_uint(value: &[u8]) -> Option<u32> {
    let marker = value.first().copied()?;
    if marker == 0 {
        return Some(0);
    }
    let byte_count = usize::from(marker >> 4).checked_sub(11)?;
    if byte_count > 4 {
        return None;
    }
    let mut encoded = u64::from(marker & 0x0f);
    for byte in value.get(1..=byte_count)? {
        encoded = (encoded << 8) | u64::from(*byte);
    }
    u32::try_from(encoded >> 1).ok()
}

fn append_profile_rosters(
    profiles: &ProfileResolver,
    channels: &mut BTreeMap<u8, ChannelState>,
    events: &mut Vec<ChatEvent>,
) {
    for channel_index in profiles.synchronize(channels) {
        let channel = channels
            .get(&channel_index)
            .expect("synchronized channel exists");
        events.push(ChatEvent::Roster(roster_snapshot(
            &channel.roster,
            channel_index,
            channel.initial_roster_complete,
        )));
    }
}

fn take_requested_join(
    pending: &mut BTreeMap<u32, (ChatChannel, Instant)>,
    result: &ChatJoin,
) -> Result<Option<ChatChannel>> {
    if let Some(token) = result.token {
        return Ok(pending.remove(&token).map(|(channel, _)| channel));
    }

    let candidates = pending
        .iter()
        .filter_map(|(token, (channel, _))| {
            join_result_matches_channel(result, channel).then_some(*token)
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => Ok(None),
        [token] => Ok(pending.remove(token).map(|(channel, _)| channel)),
        _ => Err(protocol_error(
            "Battle.net omitted the join token for multiple matching requests",
        )),
    }
}

fn join_result_matches_channel(result: &ChatJoin, channel: &ChatChannel) -> bool {
    match channel {
        ChatChannel::Public(identifier) => {
            result.channel_type == Some(PUBLIC_CHANNEL_TYPE)
                && (!result.success || result.channel_name_id == Some(*identifier))
        }
        ChatChannel::Private(_) => result.channel_type == Some(NAMED_CHANNEL_TYPE),
        ChatChannel::Club(_) => result.channel_type == Some(CLUB_CHANNEL_TYPE),
        ChatChannel::Party => result.channel_type == Some(PARTY_CHANNEL_TYPE),
    }
}

fn validate_public_join(requested: &ChatChannel, result: &ChatJoin) -> Result<()> {
    let ChatChannel::Public(expected) = requested else {
        return Ok(());
    };
    let Some(actual) = result.channel_name_id else {
        return Err(protocol_error(format!(
            "Battle.net omitted the public channel ID after {expected} was requested"
        )));
    };
    if actual != *expected {
        return Err(protocol_error(format!(
            "Battle.net joined public channel {actual} after {expected} was requested"
        )));
    }
    Ok(())
}

fn accepted_join(result: &ChatJoin) -> Result<(u8, u32)> {
    if !result.success {
        return Err(Error::Server(format!(
            "Battle.net rejected the chat join: reason={:?}",
            result.reason
        )));
    }
    let channel_index = result
        .channel_index
        .ok_or_else(|| protocol_error("chat join omitted its channel index"))?;
    let member_handle = result
        .member_handle
        .ok_or_else(|| protocol_error("chat join omitted the local member"))?;
    Ok((channel_index, member_handle))
}

fn accept_party_join(
    party: &mut Option<PartyChannelState>,
    result: &ChatJoin,
) -> Result<(u8, u32)> {
    let (channel_index, local_member_handle) = accepted_join(result)?;
    if party
        .as_ref()
        .is_none_or(|party| party.wire_channel_index != channel_index)
    {
        *party = Some(PartyChannelState {
            wire_channel_index: channel_index,
            ..PartyChannelState::default()
        });
    }
    let party = party.as_mut().expect("party channel was initialized");
    party.state.channel = Some(ChatChannel::Party);
    party.state.local_member_handle = Some(local_member_handle);
    Ok((channel_index, local_member_handle))
}

fn trace_party(message: impl std::fmt::Display) {
    if std::env::var_os("SUPERIORITY_TRACE").is_some()
        || std::env::var_os("SUPERIORITY_PARTY_TRACE").is_some()
    {
        eprintln!("superiority: {message}");
    }
}

fn conference_event(directory: &ConferenceDescriptions) -> ChatEvent {
    ChatEvent::ConferenceDirectory {
        identifiers: directory
            .entries
            .iter()
            .map(|entry| entry.identifier)
            .collect(),
        complete: directory.is_last,
    }
}

fn select_first_toon(session: &mut Session, protocol: &Protocol, list: &ToonList) -> Result<()> {
    let toon = list
        .displays
        .first()
        .ok_or_else(|| protocol_error("the account has no selectable SC2 toon"))?;
    session
        .records_mut()
        .send(&protocol.toon_select(&toon.name, toon.realm)?)
}

fn request_conference_cache(session: &mut Session, protocol: &Protocol) -> Result<()> {
    session
        .records_mut()
        .send(&protocol.cache_get_stream_items(1, "BNET", "CONF", "enUS")?)
}

fn apply_membership(
    roster: &mut BTreeMap<u32, RosterMember>,
    next_roster_order: &mut u64,
    membership: &crate::native::model::ChatMembership,
    emit_incremental: bool,
    local_member_handle: Option<u32>,
    events: &mut Vec<ChatEvent>,
) {
    for change in &membership.changes {
        match change.kind {
            MembershipKind::Leave => {
                let member = roster.remove(&change.member_handle);
                if local_member_handle == Some(change.member_handle) {
                    if emit_incremental {
                        events.push(ChatEvent::Removed {
                            channel_index: membership.channel_index,
                            reason: change.reason,
                        });
                    }
                    continue;
                }
                let user = ChatUser {
                    handle: change.member_handle,
                    presence_id: member.as_ref().and_then(|member| member.presence_id),
                    name: member.as_ref().and_then(|member| member.name.clone()),
                    clan_tag: member.as_ref().and_then(|member| member.clan_tag.clone()),
                    avatar: member.as_ref().and_then(|member| member.avatar),
                    presence: member.map_or(PresenceState::Unknown, |member| member.presence),
                };
                if emit_incremental {
                    events.push(ChatEvent::MemberLeft {
                        channel_index: membership.channel_index,
                        user,
                        reason: change.reason,
                    });
                }
            }
            MembershipKind::Join => {
                let joined_order = roster.get(&change.member_handle).map_or_else(
                    || {
                        let order = *next_roster_order;
                        *next_roster_order = (*next_roster_order).saturating_add(1);
                        order
                    },
                    |member| member.joined_order,
                );
                roster.insert(
                    change.member_handle,
                    RosterMember {
                        joined_order,
                        presence_id: change.presence_id,
                        name: change.display_name.clone(),
                        toon_name: change.toon_name.clone(),
                        party_status: change.party_status,
                        clan_tag: None,
                        avatar: None,
                        presence: PresenceState::Unknown,
                    },
                );
                if emit_incremental {
                    events.push(ChatEvent::MemberJoined {
                        channel_index: membership.channel_index,
                        user: ChatUser {
                            handle: change.member_handle,
                            presence_id: change.presence_id,
                            name: change.display_name.clone(),
                            clan_tag: None,
                            avatar: None,
                            presence: PresenceState::Unknown,
                        },
                    });
                }
            }
            MembershipKind::Status => {
                if !roster.contains_key(&change.member_handle) {
                    let joined_order = *next_roster_order;
                    *next_roster_order = (*next_roster_order).saturating_add(1);
                    roster.insert(
                        change.member_handle,
                        RosterMember {
                            joined_order,
                            ..RosterMember::default()
                        },
                    );
                }
                if let Some(name) = &change.display_name {
                    roster
                        .get_mut(&change.member_handle)
                        .expect("status member was just inserted")
                        .name = Some(name.clone());
                }
                if let Some(toon_name) = &change.toon_name {
                    roster
                        .get_mut(&change.member_handle)
                        .expect("status member was just inserted")
                        .toon_name = Some(toon_name.clone());
                }
                if let Some(party_status) = change.party_status {
                    roster
                        .get_mut(&change.member_handle)
                        .expect("status member was just inserted")
                        .party_status = Some(party_status);
                }
            }
        }
    }
}

fn message_event(
    channels: &BTreeMap<u8, ChannelState>,
    party: Option<&PartyChannelState>,
    message: &crate::native::model::ChatMessage,
) -> ChatEvent {
    let is_party = party.is_some_and(|party| party.wire_channel_index == message.channel_index);
    let roster = if is_party {
        party.map(|party| &party.state.roster)
    } else {
        channels
            .get(&message.channel_index)
            .map(|channel| &channel.roster)
    };
    ChatEvent::Message {
        channel_index: if is_party {
            PARTY_UI_CHANNEL_INDEX
        } else {
            message.channel_index
        },
        sender: ChatUser {
            handle: message.member_handle,
            presence_id: roster
                .and_then(|roster| roster.get(&message.member_handle))
                .and_then(|member| member.presence_id),
            name: roster
                .and_then(|roster| roster.get(&message.member_handle))
                .and_then(|member| member.name.clone()),
            clan_tag: roster
                .and_then(|roster| roster.get(&message.member_handle))
                .and_then(|member| member.clan_tag.clone()),
            avatar: roster
                .and_then(|roster| roster.get(&message.member_handle))
                .and_then(|member| member.avatar),
            presence: roster
                .and_then(|roster| roster.get(&message.member_handle))
                .map_or(PresenceState::Unknown, |member| member.presence),
        },
        body: message.body.clone(),
    }
}

fn roster_snapshot(
    roster: &BTreeMap<u32, RosterMember>,
    channel_index: u8,
    initial_complete: bool,
) -> RosterSnapshot {
    let mut users = roster
        .iter()
        .map(|(handle, member)| {
            (
                member.joined_order,
                ChatUser {
                    handle: *handle,
                    presence_id: member.presence_id,
                    name: member.name.clone(),
                    clan_tag: member.clan_tag.clone(),
                    avatar: member.avatar,
                    presence: member.presence,
                },
            )
        })
        .collect::<Vec<_>>();
    users.sort_by_key(|(joined_order, _)| *joined_order);
    RosterSnapshot {
        channel_index,
        initial_complete,
        users: users.into_iter().map(|(_, user)| user).collect(),
    }
}

fn record_route(record: &Record) -> (Option<u8>, u8) {
    (record.header.service_slot, record.header.command_id)
}

fn protocol_error(message: impl Into<String>) -> Error {
    Error::Native(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::model::{
        ChatMembership, FriendToon, FriendToonPage, FriendUpdate, MembershipChange, PresenceField,
        PresenceFieldFlags, PresenceFields, PresenceUpdate,
    };

    #[test]
    fn visible_names_match_retail_character_and_clan_formatting() {
        let user = ChatUser {
            handle: 17,
            presence_id: None,
            name: Some("Tagban#542".into()),
            clan_tag: Some("BNU".into()),
            avatar: None,
            presence: PresenceState::Available,
        };

        assert_eq!(user.visible_name(), "<BNU> Tagban");
    }

    #[test]
    fn character_code_stripping_only_accepts_a_numeric_suffix() {
        assert_eq!(strip_character_code("Tagban#542"), "Tagban");
        assert_eq!(strip_character_code("Tagban#abc"), "Tagban#abc");
        assert_eq!(strip_character_code("Tagban#"), "Tagban#");
        assert_eq!(strip_character_code("#542"), "#542");
        assert_eq!(strip_character_code("Tagban"), "Tagban");
    }

    #[test]
    fn friend_updates_retain_names_across_presence_only_modifications() {
        let identity = FriendIdentity::Account(42);
        let mut friends = BTreeMap::new();
        apply_friend_page(
            &mut friends,
            &FriendsPage {
                complete: Some(false),
                updates: vec![FriendUpdate {
                    operation: SocialOperation::Add,
                    entry: FriendEntry {
                        identity: identity.clone(),
                        display_name: Some("Raynor#123".into()),
                        full_name: Some("Jim Raynor".into()),
                        note: Some("Marshal".into()),
                        profile: Some(ProfileAddress { label: 7, id: 42 }),
                        toon_name: None,
                    },
                }],
            },
        );
        apply_friend_page(
            &mut friends,
            &FriendsPage {
                complete: Some(true),
                updates: vec![FriendUpdate {
                    operation: SocialOperation::Modify,
                    entry: FriendEntry {
                        identity,
                        display_name: None,
                        full_name: None,
                        note: None,
                        profile: None,
                        toon_name: None,
                    },
                }],
            },
        );
        let profiles = ProfileResolver::default();
        assert_eq!(friend_snapshot(&friends, &profiles)[0].name, "Raynor#123");
        assert_eq!(
            friend_snapshot(&friends, &profiles)[0].note.as_deref(),
            Some("Marshal")
        );
        assert_eq!(
            friends.get(&FriendIdentity::Account(42)).unwrap().profile,
            Some(ProfileAddress { label: 7, id: 42 })
        );
    }

    #[test]
    fn friend_snapshot_uses_resolved_profile_avatar() {
        let address = ProfileAddress { label: 7, id: 42 };
        let avatar = ImageTableEntry {
            table_id: 3,
            offset: 11,
        };
        let friends = BTreeMap::from([(
            FriendIdentity::Account(42),
            FriendEntry {
                identity: FriendIdentity::Account(42),
                display_name: Some("Raynor#123".into()),
                full_name: None,
                note: None,
                profile: Some(address),
                toon_name: None,
            },
        )]);
        let mut profiles = ProfileResolver::default();
        profiles.avatars.insert(address, Some(avatar));

        assert_eq!(friend_snapshot(&friends, &profiles)[0].avatar, Some(avatar));
    }

    #[test]
    fn friend_snapshot_whispers_to_the_resolved_sc2_toon() {
        let toon_name = ToonFullName {
            region: 1,
            program_id: fourcc("S2"),
            realm: 1,
            name: "Raynor#123".into(),
        };
        let friends = BTreeMap::from([(
            FriendIdentity::Account(42),
            FriendEntry {
                identity: FriendIdentity::Account(42),
                display_name: Some("Raynor#123".into()),
                full_name: None,
                note: None,
                profile: None,
                toon_name: Some(toon_name.clone()),
            },
        )]);

        assert_eq!(
            friend_snapshot(&friends, &ProfileResolver::default())[0].target,
            WhisperTarget::ToonName(toon_name)
        );
    }

    #[test]
    fn account_friend_without_a_toon_is_queued_for_sc2_identity_resolution() {
        let friends = BTreeMap::from([(
            FriendIdentity::Account(308_451_427),
            FriendEntry {
                identity: FriendIdentity::Account(308_451_427),
                display_name: None,
                full_name: Some("Carlos Perez".into()),
                note: None,
                profile: None,
                toon_name: None,
            },
        )]);

        assert_eq!(
            friend_snapshot(&friends, &ProfileResolver::default())[0].target,
            WhisperTarget::Account(308_451_427)
        );
    }

    #[test]
    fn friend_name_matching_accepts_first_name_shorthand() {
        let friend = FriendEntry {
            identity: FriendIdentity::Account(308_451_427),
            display_name: None,
            full_name: Some("Carlos Perez".into()),
            note: None,
            profile: None,
            toon_name: None,
        };

        assert!(friend_matches_name(&friend, "Carlos"));
        assert!(friend_matches_name(&friend, "carlos perez"));
        assert!(!friend_matches_name(&friend, "Carl"));
    }

    #[test]
    fn account_friend_uses_the_linked_local_presence_id() {
        const ACCOUNT_INFO: u32 = 0x0001_0015;
        let friends = BTreeMap::from([(
            FriendIdentity::Account(308_451_427),
            FriendEntry {
                identity: FriendIdentity::Account(308_451_427),
                display_name: None,
                full_name: Some("Carlos Perez".into()),
                note: None,
                profile: None,
                toon_name: None,
            },
        )]);
        let mut profiles = ProfileResolver::default();
        profiles.presence.announce(&PresenceFields {
            entries: vec![PresenceField {
                handle: ACCOUNT_INFO,
                identifier: 21,
                flags: crate::native::model::PresenceFieldFlags::default(),
                fixed_size: Some(7),
            }],
        });
        profiles
            .presence
            .apply(&crate::native::model::PresenceUpdate {
                local_presence_id: 34_222_470,
                master_presence_id: 91,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();
        profiles
            .presence
            .apply(&crate::native::model::PresenceUpdate {
                local_presence_id: 0,
                master_presence_id: 91,
                field_data: hex::decode("12629863010000").unwrap(),
                cleared_handles: Vec::new(),
                handles: vec![ACCOUNT_INFO],
                variable_sizes: Vec::new(),
                online: true,
            })
            .unwrap();

        let snapshot = friend_snapshot(&friends, &profiles);
        assert_eq!(snapshot[0].target, WhisperTarget::Presence(34_222_470));
        assert_eq!(snapshot[0].presence, PresenceState::Available);
    }

    #[test]
    fn friend_toon_identity_does_not_require_an_avatar_profile() {
        let toon_name = ToonFullName {
            region: 1,
            program_id: fourcc("S2"),
            realm: 1,
            name: "Raynor#123".into(),
        };
        let identity = FriendIdentity::Account(42);
        let mut friends = BTreeMap::from([(
            identity.clone(),
            FriendEntry {
                identity: identity.clone(),
                display_name: Some("Jim Raynor".into()),
                full_name: Some("Jim Raynor".into()),
                note: None,
                profile: None,
                toon_name: None,
            },
        )]);

        apply_friend_toons(
            &mut friends,
            &FriendToonPage {
                entries: vec![FriendToon {
                    account_id: 42,
                    program_id: fourcc("S2"),
                    profile: None,
                    toon_name: toon_name.clone(),
                }],
                complete: true,
            },
        );

        assert_eq!(friends[&identity].toon_name, Some(toon_name.clone()));
        assert_eq!(
            friend_snapshot(&friends, &ProfileResolver::default())[0].target,
            WhisperTarget::ToonName(toon_name)
        );
    }

    #[test]
    fn friend_toon_identity_is_requested_even_when_an_avatar_profile_exists() {
        let identity = FriendIdentity::Account(42);
        let friends = BTreeMap::from([(
            identity.clone(),
            FriendEntry {
                identity,
                display_name: Some("Jim Raynor".into()),
                full_name: Some("Jim Raynor".into()),
                note: None,
                profile: Some(ProfileAddress { label: 7, id: 42 }),
                toon_name: None,
            },
        )]);
        let mut profiles = ProfileResolver::default();

        profiles.enqueue_missing(std::iter::empty(), &friends);

        assert_eq!(profiles.friend_account_queue.pop_front(), Some(42));
        assert_eq!(
            profiles.queue.pop_front(),
            Some(ProfileAddress { label: 7, id: 42 })
        );
    }

    #[test]
    fn roster_preserves_presence_identity_across_status_updates() {
        let mut roster = BTreeMap::new();
        let mut next_roster_order = 0;
        let mut events = Vec::new();
        apply_membership(
            &mut roster,
            &mut next_roster_order,
            &ChatMembership {
                channel_index: 0,
                end_of_initial: false,
                changes: vec![MembershipChange {
                    kind: MembershipKind::Join,
                    member_handle: 17,
                    presence_id: Some(41),
                    display_name: Some("Nova".into()),
                    toon_name: None,
                    party_status: None,
                    reason: None,
                }],
            },
            false,
            None,
            &mut events,
        );
        apply_membership(
            &mut roster,
            &mut next_roster_order,
            &ChatMembership {
                channel_index: 0,
                end_of_initial: true,
                changes: vec![MembershipChange {
                    kind: MembershipKind::Status,
                    member_handle: 17,
                    presence_id: None,
                    display_name: Some("November".into()),
                    toon_name: None,
                    party_status: None,
                    reason: None,
                }],
            },
            false,
            None,
            &mut events,
        );

        assert_eq!(
            roster_snapshot(&roster, 0, true).users,
            vec![ChatUser {
                handle: 17,
                presence_id: Some(41),
                name: Some("November".into()),
                clan_tag: None,
                avatar: None,
                presence: PresenceState::Unknown,
            }]
        );
    }

    #[test]
    fn roster_snapshots_preserve_join_order() {
        let mut roster = BTreeMap::new();
        let mut next_roster_order = 0;
        let mut events = Vec::new();
        for (handle, name) in [(20, "Zulu"), (10, "Alpha"), (30, "Mike")] {
            apply_membership(
                &mut roster,
                &mut next_roster_order,
                &ChatMembership {
                    channel_index: 0,
                    end_of_initial: false,
                    changes: vec![MembershipChange {
                        kind: MembershipKind::Join,
                        member_handle: handle,
                        presence_id: None,
                        display_name: Some(name.into()),
                        toon_name: None,
                        party_status: None,
                        reason: None,
                    }],
                },
                false,
                None,
                &mut events,
            );
        }

        assert_eq!(
            roster_snapshot(&roster, 0, true)
                .users
                .iter()
                .map(ChatUser::visible_name)
                .collect::<Vec<_>>(),
            vec!["Zulu", "Alpha", "Mike"]
        );
    }

    #[test]
    fn a_leave_carries_its_reason_and_self_removal_is_distinct() {
        let mut roster =
            BTreeMap::from([(17, RosterMember::default()), (99, RosterMember::default())]);
        let leave = |handle: u32, reason: u16| crate::native::model::ChatMembership {
            channel_index: 2,
            end_of_initial: false,
            changes: vec![MembershipChange {
                kind: MembershipKind::Leave,
                member_handle: handle,
                presence_id: None,
                display_name: None,
                toon_name: None,
                party_status: None,
                reason: Some(reason),
            }],
        };

        let mut events = Vec::new();
        let mut next_roster_order = 2;
        apply_membership(
            &mut roster,
            &mut next_roster_order,
            &leave(99, 70),
            true,
            Some(17),
            &mut events,
        );
        assert_eq!(
            events,
            vec![ChatEvent::MemberLeft {
                channel_index: 2,
                user: ChatUser {
                    handle: 99,
                    presence_id: None,
                    name: None,
                    clan_tag: None,
                    avatar: None,
                    presence: PresenceState::Unknown,
                },
                reason: Some(70),
            }]
        );

        let mut events = Vec::new();
        apply_membership(
            &mut roster,
            &mut next_roster_order,
            &leave(17, 70),
            true,
            Some(17),
            &mut events,
        );
        assert_eq!(
            events,
            vec![ChatEvent::Removed {
                channel_index: 2,
                reason: Some(70),
            }]
        );
    }

    #[test]
    fn roster_tracks_server_presence_updates() {
        let mut resolver = ProfileResolver::default();
        let mut channels = BTreeMap::from([(
            0,
            ChannelState {
                roster: BTreeMap::from([(
                    17,
                    RosterMember {
                        joined_order: 0,
                        presence_id: Some(91),
                        name: Some("Nova".into()),
                        toon_name: None,
                        party_status: None,
                        clan_tag: None,
                        avatar: None,
                        presence: PresenceState::Unknown,
                    },
                )]),
                ..ChannelState::default()
            },
        )]);

        for (online, expected) in [
            (true, PresenceState::Available),
            (false, PresenceState::Offline),
        ] {
            resolver.observe(&Payload::PresenceUpdate(PresenceUpdate {
                local_presence_id: 41,
                master_presence_id: 91,
                field_data: Vec::new(),
                cleared_handles: Vec::new(),
                handles: Vec::new(),
                variable_sizes: Vec::new(),
                online,
            }));
            assert_eq!(resolver.synchronize(&mut channels), vec![0]);
            assert_eq!(
                channels[&0].roster[&17].presence, expected,
                "the roster should expose the server's presence state"
            );
            assert_eq!(
                channels[&0].roster[&17].presence_id,
                Some(41),
                "chat targets must normalize the membership id to presence m_idLocal"
            );
        }
    }

    #[test]
    fn roster_uses_the_retail_presence_clan_tag() {
        let mut resolver = ProfileResolver::default();
        let mut channels = BTreeMap::from([(
            0,
            ChannelState {
                roster: BTreeMap::from([(
                    17,
                    RosterMember {
                        joined_order: 0,
                        presence_id: Some(41),
                        name: Some("Tagban#542".into()),
                        toon_name: None,
                        party_status: None,
                        clan_tag: None,
                        avatar: None,
                        presence: PresenceState::Unknown,
                    },
                )]),
                ..ChannelState::default()
            },
        )]);
        resolver.observe(&Payload::PresenceFields(PresenceFields {
            entries: vec![PresenceField {
                handle: crate::native::presence::FIELD_CLAN_TAG,
                identifier: 13,
                flags: PresenceFieldFlags::default(),
                fixed_size: None,
            }],
        }));
        resolver.observe(&Payload::PresenceUpdate(PresenceUpdate {
            local_presence_id: 41,
            master_presence_id: 91,
            field_data: hex::decode("0101424e55").unwrap(),
            cleared_handles: Vec::new(),
            handles: vec![crate::native::presence::FIELD_CLAN_TAG],
            variable_sizes: vec![5],
            online: true,
        }));

        assert_eq!(resolver.synchronize(&mut channels), vec![0]);
        let roster = roster_snapshot(&channels[&0].roster, 0, true);
        assert_eq!(roster.users[0].visible_name(), "<BNU> Tagban");
    }

    #[test]
    fn channel_rosters_and_messages_remain_isolated() {
        let mut channels = BTreeMap::new();
        for (channel_index, name) in [(2, "Raynor"), (7, "Kerrigan")] {
            let channel = channels
                .entry(channel_index)
                .or_insert_with(ChannelState::default);
            channel.roster.insert(
                17,
                RosterMember {
                    joined_order: 0,
                    presence_id: Some(u32::from(channel_index)),
                    name: Some(name.into()),
                    toon_name: None,
                    party_status: None,
                    clan_tag: None,
                    avatar: None,
                    presence: PresenceState::Unknown,
                },
            );
        }

        let event = message_event(
            &channels,
            None,
            &crate::native::model::ChatMessage {
                channel_index: 7,
                member_handle: 17,
                body: "For the Swarm".into(),
            },
        );
        assert_eq!(
            event,
            ChatEvent::Message {
                channel_index: 7,
                sender: ChatUser {
                    handle: 17,
                    presence_id: Some(7),
                    name: Some("Kerrigan".into()),
                    clan_tag: None,
                    avatar: None,
                    presence: PresenceState::Unknown,
                },
                body: "For the Swarm".into(),
            }
        );
        assert_eq!(channels.get(&2).unwrap().roster.len(), 1);
        assert_eq!(channels.get(&7).unwrap().roster.len(), 1);
    }

    #[test]
    fn party_join_has_the_state_needed_for_membership_tracking() {
        let result = ChatJoin {
            success: true,
            channel_index: Some(6),
            member_handle: Some(91),
            channel_type: Some(PARTY_CHANNEL_TYPE),
            channel_name_id: None,
            channel_shard_index: None,
            reason: None,
            token: None,
        };

        let mut party = None;
        assert_eq!(accept_party_join(&mut party, &result).unwrap(), (6, 91));

        let party = party.unwrap();
        assert_eq!(party.wire_channel_index, 6);
        assert_eq!(party.state.local_member_handle, Some(91));
        assert_eq!(party.state.channel, Some(ChatChannel::Party));
        assert!(!party.state.initial_roster_complete);
        assert!(party.state.roster.is_empty());
    }

    #[test]
    fn public_join_without_a_token_matches_the_returned_channel_id() {
        let now = Instant::now();
        let mut pending = BTreeMap::from([
            (11, (ChatChannel::Public(1028), now)),
            (12, (ChatChannel::Public(1030), now)),
        ]);
        let result = ChatJoin {
            success: true,
            channel_index: Some(2),
            member_handle: Some(91),
            channel_type: Some(PUBLIC_CHANNEL_TYPE),
            channel_name_id: Some(1030),
            channel_shard_index: Some(1),
            reason: None,
            token: None,
        };

        assert_eq!(
            take_requested_join(&mut pending, &result).unwrap(),
            Some(ChatChannel::Public(1030))
        );
        assert_eq!(pending.len(), 1);
        assert!(pending.contains_key(&11));
    }

    #[test]
    fn missing_join_token_never_guesses_between_matching_requests() {
        let now = Instant::now();
        let mut pending = BTreeMap::from([
            (11, (ChatChannel::Private("one".into()), now)),
            (12, (ChatChannel::Private("two".into()), now)),
        ]);
        let result = ChatJoin {
            success: true,
            channel_index: Some(2),
            member_handle: Some(91),
            channel_type: Some(NAMED_CHANNEL_TYPE),
            channel_name_id: None,
            channel_shard_index: Some(1),
            reason: None,
            token: None,
        };

        assert!(take_requested_join(&mut pending, &result).is_err());
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn public_join_rejects_a_different_returned_channel_id() {
        let result = ChatJoin {
            success: true,
            channel_index: Some(2),
            member_handle: Some(91),
            channel_type: Some(PUBLIC_CHANNEL_TYPE),
            channel_name_id: Some(1033),
            channel_shard_index: Some(2),
            reason: None,
            token: Some(11),
        };

        assert!(validate_public_join(&ChatChannel::Public(1028), &result).is_err());
    }

    #[test]
    fn decodes_targeted_avatar_profile_value() {
        assert_eq!(
            decode_avatar_block(&hex::decode("0614f0504f525401f15fce1068").unwrap()),
            Some(ImageTableEntry {
                table_id: 0,
                offset: 0,
            })
        );
        assert_eq!(
            decode_avatar_block(&hex::decode("0614f0504f525401ebae8364").unwrap()),
            Some(ImageTableEntry {
                table_id: 0,
                offset: 15,
            })
        );
        assert_eq!(
            decode_avatar_block(&hex::decode("0614f049434f4e0100").unwrap()),
            None
        );
    }
}
