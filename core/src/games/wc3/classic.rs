use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    time::{Duration, Instant},
};

use aes::{
    Aes256,
    cipher::{BlockDecrypt as _, KeyInit as _, generic_array::GenericArray},
};
use rand::RngCore as _;
use sha2::{Digest as _, Sha512_256};
use url::Url;
use zeroize::{Zeroize as _, Zeroizing};

use crate::{
    Error, Result,
    games::scr::{
        envelope::CheckValueEnvelope,
        rpc::{Frame, Header},
    },
    platform::{
        bgs::SecretBytes,
        wire::{
            raw::{self as protobuf, Message, Value},
            websocket::{RpcSocket, SocketProfile},
        },
    },
    product::Product,
};

use super::{
    account::{AuthSession, ConnectionSession},
    identity::ClientIdentity,
    protocol::{
        CLASSIC_RPC_PATH, CLIENT_CAPABILITIES, GAME_VERSION, LOCALE_FOURCC, PLATFORM_FOURCC,
        SESSION_TYPE, TITLE_ID,
    },
};

const DEFAULT_ROUTING_ID: u32 = 2_525_111_537;
const AUTHENTICATION_SERVICE: u32 = 0x17CD_FF07;
const AUTH_SESSION: u32 = 0x95F5_9163;
const COOKIE_UPDATE: u32 = 0xF2C6_4BA6;
const GAME_ACCOUNT: u32 = 0x3542_52A4;
const GET_TOONS: u32 = 0xBC18_EDE5;
const GAME_VERSION_SERVICE: u32 = 0x3D93_0F0E;
const SET_GAME_VERSION: u32 = 0xD48D_E460;
const AURORA_CHAT: u32 = 0x924C_CFDA;
const AURORA_FRIENDS: u32 = 0xAA4E_1E00;
const SEND_WHISPER: u32 = 0x6251_CCD8;
const GET_PUBLIC_CHANNEL_LIST: u32 = 0x1D5C_7428;
const JOIN_CHANNEL: u32 = 0x661D_7CA1;
const LEAVE_CHANNEL: u32 = 0x8112_AF72;
const SEND_MESSAGE: u32 = 0x2FEE_D495;
const CHANNEL_ADDED: u32 = 0x4ADE_AA27;
const CHANNEL_REMOVED: u32 = 0xAD99_5A17;
const MEMBER_ADDED: u32 = 0x55F5_73C2;
const MEMBER_REMOVED: u32 = 0x8CA0_DB7A;
const UPDATE_MEMBER_PRESENCE: u32 = 0x061D_FC04;
const BATCH_UPDATE_MEMBER_PRESENCE: u32 = 0xF898_F80E;
const MESSAGE_RECEIVED: u32 = 0x1D6B_1DC2;
const WHISPER_RECEIVED: u32 = 0x7255_E575;
const WHISPER_ECHO_RECEIVED: u32 = 0x82B8_44A8;
const CHANNEL_SUBSCRIPTION_UPDATED: u32 = 0xF90D_37BF;
const WHISPER_SUBSCRIPTION_UPDATED: u32 = 0x8ECE_5580;
const BATCH_FRIEND_UPDATED: u32 = 0x412F_3DBB;
const CLAN: u32 = 0x122F_0B4B;
const GET_CLAN_MEMBERS: u32 = 0x18D1_2EF6;
const RECEIVED_MY_CLAN_ON_LOGIN: u32 = 0x904C_D17E;
const CLAN_UPDATED: u32 = 0xF74F_2D2E;
const CLAN_MEMBER_ADDED: u32 = 0x3625_E7F1;
const CLAN_MEMBER_REMOVED: u32 = 0xF766_8051;
const CLAN_MEMBER_RANK_CHANGED: u32 = 0xF23B_93BD;
const CLAN_MEMBER_PRESENCE_UPDATED: u32 = 0x5D4F_0B7F;
const CLAN_BATCHED_MEMBER_PRESENCE_UPDATED: u32 = 0x5DF1_23B6;

const MAX_PUBLIC_CHANNELS: usize = 1_024;
const MAX_CHANNEL_IDENTIFIER_BYTES: usize = 4 * 1_024;
const MAX_MEMBERS: usize = 4_096;
const MAX_NAME_BYTES: usize = 256;
const MAX_CHAT_BYTES: usize = 4 * 1_024;
const MAX_CLAN_MEMBERS: usize = 1_024;

#[derive(Clone)]
pub(super) struct ClassicEndpoint {
    host: String,
    port: u16,
    path: String,
    ticket: SecretBytes,
}

impl ClassicEndpoint {
    pub fn from_url(url: &str, ticket: SecretBytes) -> Result<Self> {
        if ticket.len() != 56 {
            return Err(classic_error("classic handoff ticket is not 56 bytes"));
        }
        let parsed = Url::parse(url)
            .map_err(|_| classic_error("ProcessTask returned an invalid classic URL"))?;
        if parsed.scheme() != "wss" || !parsed.username().is_empty() || parsed.password().is_some()
        {
            return Err(classic_error("classic handoff URL is not a clean wss URL"));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| classic_error("classic handoff URL has no host"))?;
        if host != "battle.net" && !host.ends_with(".battle.net") {
            return Err(classic_error("classic handoff URL is outside battle.net"));
        }
        let root = parsed.path().trim_end_matches('/');
        let mut path = format!("{root}{CLASSIC_RPC_PATH}");
        if let Some(query) = parsed.query() {
            path.push('?');
            path.push_str(query);
        }
        Ok(Self {
            host: host.into(),
            port: parsed.port().unwrap_or(443),
            path,
            ticket,
        })
    }
}

impl fmt::Debug for ClassicEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClassicEndpoint")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("path", &self.path)
            .field("ticket", &self.ticket)
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
struct Request<'a> {
    service_id: u32,
    method_id: u32,
    body: &'a [u8],
    trace: Option<&'a [u8]>,
}

impl<'a> Request<'a> {
    const fn new(service_id: u32, method_id: u32, body: &'a [u8]) -> Self {
        Self {
            service_id,
            method_id,
            body,
            trace: None,
        }
    }

    const fn traced(mut self, trace: &'a [u8]) -> Self {
        self.trace = Some(trace);
        self
    }
}

struct QueuedCallback {
    service_id: u32,
    method_id: u32,
    body: Zeroizing<Vec<u8>>,
}

impl QueuedCallback {
    fn from_frame(frame: &Frame) -> Self {
        Self {
            service_id: frame.header.service_id,
            method_id: frame.header.method_id,
            body: Zeroizing::new(frame.body.clone()),
        }
    }
}

struct ClassicClient {
    socket: RpcSocket,
    next_token: u32,
    default_timeout: Duration,
    authenticated: bool,
    callbacks: VecDeque<QueuedCallback>,
}

impl ClassicClient {
    fn connect(endpoint: &ClassicEndpoint, timeout: Duration) -> Result<Self> {
        let mut socket = RpcSocket::connect(
            &endpoint.host,
            endpoint.port,
            timeout,
            SocketProfile {
                path: &endpoint.path,
                subprotocol: None,
                lenient_upgrade: true,
            },
        )?;
        let envelope = CheckValueEnvelope::from_websocket_key(
            socket.handshake_key(),
            Product::Warcraft3.fourcc(),
        )?;
        socket.set_transform(Box::new(envelope));
        Ok(Self {
            socket,
            next_token: 1,
            default_timeout: timeout,
            authenticated: false,
            callbacks: VecDeque::new(),
        })
    }

    fn call(&mut self, request: &Request<'_>, timeout: Duration) -> Result<Frame> {
        let token = self.next_token;
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or_else(|| classic_error("classic request token exhausted"))?;
        self.transmit(
            &Header {
                service_id: request.service_id,
                method_id: request.method_id,
                token,
                routing_id: Some(DEFAULT_ROUTING_ID),
                object_id: Some(0),
                is_response: Some(false),
                request_trace: request.trace.map(<[u8]>::to_vec),
                ..Header::default()
            },
            request.body,
        )?;
        let deadline = Instant::now() + timeout;
        let mut acknowledgement_trace = request.trace;
        loop {
            let mut frame = self.receive(deadline)?;
            if frame.header.is_response() {
                if frame.header.token == token
                    && frame.header.service_id == request.service_id
                    && frame.header.method_id == request.method_id
                {
                    self.restore_timeout()?;
                    return Ok(frame);
                }
                frame.body.zeroize();
                continue;
            }
            self.callbacks.push_back(QueuedCallback::from_frame(&frame));
            let acknowledgement = self.acknowledge(&frame, acknowledgement_trace);
            frame.body.zeroize();
            acknowledgement?;
            acknowledgement_trace = None;
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<bool> {
        self.socket.set_timeout(Some(timeout))?;
        let mut frame = Frame::decode(&self.socket.receive_raw()?)?;
        let request = !frame.header.is_response();
        if request {
            self.callbacks.push_back(QueuedCallback::from_frame(&frame));
            let acknowledgement = self.acknowledge(&frame, None);
            frame.body.zeroize();
            acknowledgement?;
        } else {
            frame.body.zeroize();
        }
        self.restore_timeout()?;
        Ok(request)
    }

    fn receive(&mut self, deadline: Instant) -> Result<Frame> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(classic_error("classic response timed out"));
        }
        self.socket.set_timeout(Some(remaining))?;
        Frame::decode(&self.socket.receive_raw()?)
    }

    fn acknowledge(&mut self, request: &Frame, trace: Option<&[u8]>) -> Result<()> {
        self.transmit(
            &Header {
                service_id: request.header.service_id,
                method_id: request.header.method_id,
                token: request.header.token,
                routing_id: request.header.routing_id.or(Some(DEFAULT_ROUTING_ID)),
                object_id: request.header.object_id,
                is_response: Some(true),
                request_trace: trace.map(<[u8]>::to_vec),
                ..Header::default()
            },
            &[],
        )
    }

    fn transmit(&mut self, header: &Header, body: &[u8]) -> Result<()> {
        let frame = Zeroizing::new(Frame::encode(header, body)?);
        self.socket.send_raw(&frame)
    }

    fn restore_timeout(&self) -> Result<()> {
        self.socket.set_timeout(Some(self.default_timeout))
    }

    fn close(&mut self) -> Result<()> {
        self.socket.close()
    }
}

#[derive(Clone, Debug)]
pub struct PublicChannel {
    identifier: SecretBytes,
}

impl PublicChannel {
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        let name = std::str::from_utf8(self.identifier.expose()).ok()?;
        (!name.is_empty() && !name.chars().any(char::is_control)).then_some(name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChatPresence {
    Offline,
    #[default]
    Online,
    Away,
    Busy,
}

#[derive(Clone, PartialEq, Eq)]
struct ChatMemberIdentity {
    account_id: u64,
    title_id: u32,
    region: u32,
}

impl fmt::Debug for ChatMemberIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatMemberIdentity")
            .field("account_id", &"<redacted>")
            .field("title_id", &self.title_id)
            .field("region", &self.region)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChatMember {
    pub handle: u64,
    pub name: String,
    pub presence: ChatPresence,
    pub avatar_id: Option<String>,
    pub clan_abbreviation: Option<String>,
    identity: ChatMemberIdentity,
}

impl fmt::Debug for ChatMember {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatMember")
            .field("handle", &self.handle)
            .field("name", &self.name)
            .field("presence", &self.presence)
            .field("avatar_id", &self.avatar_id)
            .field("clan_abbreviation", &self.clan_abbreviation)
            .field("identity", &self.identity)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatChannel {
    /// client-local channel index used by the shared send/leave/tab contract.
    /// Battle.net's structured channel identifier remains opaque inside the
    /// session and never crosses into UI state.
    pub id: u8,
    pub name: String,
    pub members: Vec<ChatMember>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEvent {
    ChannelJoined {
        channel_id: u8,
        name: String,
        member_count: usize,
    },
    ChannelLeft {
        channel_id: u8,
    },
    MemberJoined {
        channel_id: u8,
        name: String,
    },
    MemberLeft {
        channel_id: u8,
        name: Option<String>,
    },
    Message {
        channel_id: u8,
        body: String,
    },
    Whisper {
        account_id: u64,
        peer: String,
        body: String,
        outgoing: bool,
    },
    Notice {
        text: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendPresence {
    Online,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatFriend {
    pub account_id: u64,
    pub name: String,
    pub presence: FriendPresence,
}

/// the account's clan membership as reported by the retail Clan service.
/// `Pending` is deliberately distinct from `None`: Battle.net sends an empty
/// `ReceivedMyClanOnLogin` callback for an account with no clan, so the client
/// does not manufacture a no-clan state while that callback is still in flight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClanMembership {
    #[default]
    Pending,
    None,
    Member(ClanInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanInfo {
    pub id: u64,
    pub tag: String,
    pub name: String,
    pub motd: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanMember {
    pub account_id: u64,
    pub name: String,
    pub rank: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClanSnapshot {
    pub membership: ClanMembership,
    pub members: Vec<ClanMember>,
    /// clan reads are optional to the chat session. A malformed or rejected
    /// roster is surfaced here instead of disconnecting the realm hall.
    pub read_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameListing {
    pub id: u64,
    pub name: String,
}

#[derive(Clone)]
struct JoinedChannel {
    id: u8,
    channel_id: SecretBytes,
    channel: ChatChannel,
    announced: bool,
}

struct ParsedChannel {
    channel_id: SecretBytes,
    name: String,
    members: Vec<ChatMember>,
}

#[derive(Default)]
struct ChatState {
    channels: Vec<JoinedChannel>,
    focused_channel: Option<u8>,
    friends: BTreeMap<u64, ChatFriend>,
    peer_names: BTreeMap<u64, String>,
    friends_revision: u64,
}

#[derive(Default)]
struct ClanState {
    snapshot: ClanSnapshot,
    revision: u64,
    members_refresh_pending: bool,
}

impl ClanState {
    fn dispatch(&mut self, client: &mut ClassicClient) -> Result<()> {
        let mut deferred = VecDeque::new();
        while let Some(callback) = client.callbacks.pop_front() {
            if callback.service_id != CLAN {
                deferred.push_back(callback);
                continue;
            }
            trace_classic(format_args!(
                "WC3 Clan callback method=0x{:08X} bytes={}",
                callback.method_id,
                callback.body.len()
            ));
            match callback.method_id {
                RECEIVED_MY_CLAN_ON_LOGIN => {
                    let membership = match parse_my_clan(&callback.body) {
                        Ok(membership) => membership,
                        Err(error) => {
                            // an empty callback is the only captured no-clan
                            // signal. Preserve Pending for any other unknown
                            // shape instead of inventing account state or
                            // dropping the otherwise healthy chat session.
                            self.fail_read(error.to_string());
                            continue;
                        }
                    };
                    let refresh = matches!(membership, ClanMembership::Member(_));
                    let changed = self.snapshot.membership != membership
                        || !self.snapshot.members.is_empty()
                        || self.snapshot.read_error.is_some();
                    self.snapshot.membership = membership;
                    self.snapshot.members.clear();
                    self.snapshot.read_error = None;
                    self.members_refresh_pending = refresh;
                    if changed {
                        self.bump();
                    }
                }
                CLAN_MEMBER_ADDED
                | CLAN_MEMBER_REMOVED
                | CLAN_MEMBER_RANK_CHANGED
                | CLAN_MEMBER_PRESENCE_UPDATED
                | CLAN_BATCHED_MEMBER_PRESENCE_UPDATED => {
                    if matches!(self.snapshot.membership, ClanMembership::Member(_)) {
                        self.members_refresh_pending = true;
                    }
                }
                // ClanUpdated is retained for diagnostics until a positive
                // retail capture establishes the descriptor's semantic
                // attribute mapping. Existing authoritative data remains
                // visible; no field is guessed from the callback.
                CLAN_UPDATED => {}
                _ => {}
            }
        }
        client.callbacks = deferred;
        Ok(())
    }

    fn refresh_request(&mut self) -> Option<Vec<u8>> {
        if !self.members_refresh_pending {
            return None;
        }
        self.members_refresh_pending = false;
        let ClanMembership::Member(info) = &self.snapshot.membership else {
            return None;
        };
        let clan_id = Message::new().varint(2, info.id).into_vec();
        Some(Message::new().bytes(1, &clan_id).into_vec())
    }

    fn apply_members(&mut self, body: &[u8]) -> Result<()> {
        let expected_id = match &self.snapshot.membership {
            ClanMembership::Member(info) => info.id,
            ClanMembership::Pending | ClanMembership::None => return Ok(()),
        };
        let members = parse_clan_members(body, expected_id)?;
        if self.snapshot.members != members || self.snapshot.read_error.is_some() {
            self.snapshot.members = members;
            self.snapshot.read_error = None;
            self.bump();
        }
        Ok(())
    }

    fn fail_read(&mut self, error: impl Into<String>) {
        let error = error.into();
        if self.snapshot.read_error.as_deref() != Some(&error) {
            self.snapshot.read_error = Some(error);
            self.bump();
        }
    }

    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

impl ChatState {
    fn allocate_channel_id(&self) -> Result<u8> {
        (u8::MIN..=u8::MAX)
            .find(|candidate| self.channels.iter().all(|channel| channel.id != *candidate))
            .ok_or_else(|| classic_error("too many simultaneous WC3 chat channels"))
    }

    fn reserve_channel(&mut self, channel_id: SecretBytes, name: String) -> Result<u8> {
        if let Some(channel) = self
            .channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
        {
            self.focused_channel = Some(channel.id);
            return Ok(channel.id);
        }
        let id = self.allocate_channel_id()?;
        self.channels.push(JoinedChannel {
            id,
            channel_id,
            channel: ChatChannel {
                id,
                name,
                members: Vec::new(),
            },
            announced: false,
        });
        self.focused_channel = Some(id);
        Ok(id)
    }

    fn apply_channel(&mut self, parsed: ParsedChannel) -> Result<(u8, bool)> {
        if let Some(channel) = self
            .channels
            .iter_mut()
            .find(|channel| channel.channel_id == parsed.channel_id)
        {
            let first_announcement = !channel.announced;
            channel.channel.name = parsed.name;
            channel.channel.members = parsed.members;
            channel.announced = true;
            return Ok((channel.id, first_announcement));
        }
        let id = self.allocate_channel_id()?;
        self.channels.push(JoinedChannel {
            id,
            channel_id: parsed.channel_id,
            channel: ChatChannel {
                id,
                name: parsed.name,
                members: parsed.members,
            },
            announced: true,
        });
        self.focused_channel.get_or_insert(id);
        Ok((id, true))
    }

    fn channel(&self, id: u8) -> Option<&JoinedChannel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    fn channel_mut(&mut self, id: u8) -> Option<&mut JoinedChannel> {
        self.channels.iter_mut().find(|channel| channel.id == id)
    }

    fn callback_channel_id(&self, body: &[u8]) -> Option<u8> {
        let wire_id = protobuf::first_bytes(body, 1)?;
        self.channels
            .iter()
            .find(|channel| channel.channel_id.expose() == wire_id)
            .map(|channel| channel.id)
    }

    fn remove_channel(&mut self, id: u8) -> Option<JoinedChannel> {
        let index = self.channels.iter().position(|channel| channel.id == id)?;
        let removed = self.channels.remove(index);
        if self.focused_channel == Some(id) {
            self.focused_channel = self
                .channels
                .iter()
                .rev()
                .find(|channel| channel.announced)
                .map(|channel| channel.id);
        }
        Some(removed)
    }

    fn remove_callback_channel(&mut self, body: &[u8]) -> Option<JoinedChannel> {
        let id = self.callback_channel_id(body)?;
        self.remove_channel(id)
    }

    fn announced_channels(&self) -> impl Iterator<Item = &ChatChannel> {
        self.channels
            .iter()
            .filter(|channel| channel.announced)
            .map(|channel| &channel.channel)
    }

    fn focused(&self) -> Option<&ChatChannel> {
        self.focused_channel
            .and_then(|id| self.channel(id))
            .filter(|channel| channel.announced)
            .map(|channel| &channel.channel)
            .or_else(|| self.announced_channels().next())
    }

    fn dispatch(&mut self, client: &mut ClassicClient) -> Result<Vec<ChatEvent>> {
        let mut events = Vec::new();
        let mut deferred = VecDeque::new();
        while let Some(callback) = client.callbacks.pop_front() {
            if callback.service_id == AURORA_FRIENDS {
                if callback.method_id == BATCH_FRIEND_UPDATED {
                    self.apply_batch_friend_updated(&callback.body)?;
                }
                continue;
            }
            if callback.service_id != AURORA_CHAT {
                deferred.push_back(callback);
                continue;
            }
            trace_classic(format_args!(
                "WC3 Aurora callback method=0x{:08X} bytes={}",
                callback.method_id,
                callback.body.len()
            ));
            match callback.method_id {
                CHANNEL_SUBSCRIPTION_UPDATED => events.push(ChatEvent::Notice {
                    text: "Channel service subscribed.".into(),
                }),
                WHISPER_SUBSCRIPTION_UPDATED => {}
                CHANNEL_ADDED => {
                    let parsed = parse_channel_added(&callback.body)?;
                    let name = parsed.name.clone();
                    let member_count = parsed.members.len();
                    let (channel_id, first_announcement) = self.apply_channel(parsed)?;
                    if first_announcement {
                        events.push(ChatEvent::ChannelJoined {
                            channel_id,
                            name,
                            member_count,
                        });
                    }
                }
                CHANNEL_REMOVED => {
                    if let Some(channel) = self.remove_callback_channel(&callback.body) {
                        events.push(ChatEvent::ChannelLeft {
                            channel_id: channel.id,
                        });
                    }
                }
                MEMBER_ADDED => {
                    if let Some(channel_id) = self.callback_channel_id(&callback.body)
                        && let Some(member) = find_member_info(&callback.body)?
                    {
                        let name = member.name.clone();
                        if let Some(channel) = self.channel_mut(channel_id)
                            && !channel
                                .channel
                                .members
                                .iter()
                                .any(|current| current.identity == member.identity)
                        {
                            channel.channel.members.push(member);
                            sort_members(&mut channel.channel.members);
                        }
                        events.push(ChatEvent::MemberJoined { channel_id, name });
                    }
                }
                MEMBER_REMOVED => {
                    if let Some(channel_id) = self.callback_channel_id(&callback.body) {
                        let name = find_member_name(&callback.body)?;
                        if let (Some(channel), Some(name)) =
                            (self.channel_mut(channel_id), name.as_ref())
                        {
                            channel
                                .channel
                                .members
                                .retain(|member| member.name != *name);
                        }
                        events.push(ChatEvent::MemberLeft { channel_id, name });
                    }
                }
                UPDATE_MEMBER_PRESENCE => {
                    for channel in &mut self.channels {
                        apply_member_presence(&mut channel.channel, &callback.body)?;
                    }
                }
                BATCH_UPDATE_MEMBER_PRESENCE => {
                    for field in protobuf::fields(&callback.body) {
                        let field = field?;
                        if field.number != 1 {
                            continue;
                        }
                        let Value::Bytes(update) = field.value else {
                            return Err(classic_error(
                                "BatchUpdateMemberPresence contains a non-message update",
                            ));
                        };
                        for channel in &mut self.channels {
                            apply_member_presence(&mut channel.channel, update)?;
                        }
                    }
                }
                MESSAGE_RECEIVED => {
                    if let Some(channel_id) = self.callback_channel_id(&callback.body) {
                        events.push(ChatEvent::Message {
                            channel_id,
                            body: parse_chat_message(&callback.body)?,
                        });
                    }
                }
                WHISPER_RECEIVED | WHISPER_ECHO_RECEIVED => {
                    let outgoing = callback.method_id == WHISPER_ECHO_RECEIVED;
                    let whisper = parse_whisper(&callback.body, outgoing)?;
                    let peer = whisper
                        .peer
                        .or_else(|| self.peer_names.get(&whisper.account_id).cloned())
                        .or_else(|| {
                            self.friends
                                .get(&whisper.account_id)
                                .map(|friend| friend.name.clone())
                        })
                        .unwrap_or_else(|| format!("Battle.net account {}", whisper.account_id));
                    self.peer_names.insert(whisper.account_id, peer.clone());
                    events.push(ChatEvent::Whisper {
                        account_id: whisper.account_id,
                        peer,
                        body: whisper.body,
                        outgoing,
                    });
                }
                _ => {}
            }
        }
        client.callbacks = deferred;
        Ok(events)
    }

    fn apply_batch_friend_updated(&mut self, body: &[u8]) -> Result<()> {
        let mut changed = false;
        for field in protobuf::fields(body) {
            let field = field?;
            if field.number != 1 {
                continue;
            }
            let Value::Bytes(update) = field.value else {
                return Err(classic_error(
                    "BatchFriendUpdated contains a non-message update",
                ));
            };
            match parse_friend_update(update)? {
                FriendUpdate::Remove(account_id) => {
                    changed |= self.friends.remove(&account_id).is_some();
                }
                FriendUpdate::Upsert(friend) => {
                    self.peer_names
                        .insert(friend.account_id, friend.name.clone());
                    if self.friends.get(&friend.account_id) != Some(&friend) {
                        self.friends.insert(friend.account_id, friend);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.friends_revision = self.friends_revision.wrapping_add(1);
        }
        Ok(())
    }
}

pub(super) struct ClassicSession {
    client: ClassicClient,
    public_channels: Vec<PublicChannel>,
    chat: ChatState,
    clan: ClanState,
    timeout: Duration,
    latest_cookie: Option<SecretBytes>,
}

impl ClassicSession {
    pub fn establish(
        endpoint: &ClassicEndpoint,
        connection: &ConnectionSession,
        session: &AuthSession,
        identity: &ClientIdentity,
        cached_cookie: Option<&SecretBytes>,
        timeout: Duration,
    ) -> Result<Self> {
        let mut client = ClassicClient::connect(endpoint, timeout)?;
        let body = auth_request(endpoint, connection, session, identity, cached_cookie)?;
        let trace = Zeroizing::new(request_trace());
        let response = client.call(
            &Request::new(AUTHENTICATION_SERVICE, AUTH_SESSION, &body).traced(&trace),
            timeout,
        )?;
        validate_online_stats(&response.body, &session.session_key)?;
        client.authenticated = true;
        let latest_cookie = take_cookie_updates(&mut client)?;

        let toons = client.call(&Request::new(GAME_ACCOUNT, GET_TOONS, &[]), timeout)?;
        validate_toons(&toons.body)?;
        let version = Message::new().bytes(1, GAME_VERSION.as_bytes()).into_vec();
        let response = client.call(
            &Request::new(GAME_VERSION_SERVICE, SET_GAME_VERSION, &version),
            timeout,
        )?;
        require_empty(&response.body, "SetGameVersion")?;
        let response = client.call(
            &Request::new(AURORA_CHAT, GET_PUBLIC_CHANNEL_LIST, &[]),
            timeout,
        )?;
        let public_channels = parse_public_channels(&response.body)?;
        let this = Self {
            client,
            public_channels,
            chat: ChatState::default(),
            clan: ClanState::default(),
            timeout,
            latest_cookie,
        };
        // startup callbacks are meaningful product events; keep them ready for
        // the worker's first dispatch rather than discarding them here.
        Ok(this)
    }

    pub fn public_channels(&self) -> &[PublicChannel] {
        &self.public_channels
    }

    pub fn channel(&self) -> Option<&ChatChannel> {
        self.chat.focused()
    }

    pub fn channels(&self) -> impl Iterator<Item = &ChatChannel> {
        self.chat.announced_channels()
    }

    pub fn friends(&self) -> impl Iterator<Item = &ChatFriend> {
        self.chat.friends.values()
    }

    pub const fn friends_revision(&self) -> u64 {
        self.chat.friends_revision
    }

    pub const fn clan(&self) -> &ClanSnapshot {
        &self.clan.snapshot
    }

    pub const fn clan_revision(&self) -> u64 {
        self.clan.revision
    }

    pub fn take_cookie(&mut self) -> Option<SecretBytes> {
        self.latest_cookie.take()
    }

    pub fn join(&mut self, index: usize) -> Result<Vec<ChatEvent>> {
        let channel = self
            .public_channels
            .get(index)
            .ok_or_else(|| classic_error("public channel index is out of range"))?;
        let display_name = channel.display_name().unwrap_or("WC3 channel").to_owned();
        let body = Message::new()
            .bytes(1, channel.identifier.expose())
            .into_vec();
        let response = self.client.call(
            &Request::new(AURORA_CHAT, JOIN_CHANNEL, &body),
            self.timeout,
        )?;
        let channel_id = protobuf::first_bytes(&response.body, 1)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| classic_error("JoinChannel returned no channel id"))?;
        self.chat
            .reserve_channel(SecretBytes::new(channel_id.to_vec())?, display_name)?;
        self.dispatch_product_events()
    }

    pub fn join_named(&mut self, name: &str) -> Result<Vec<ChatEvent>> {
        let index = self
            .public_channels
            .iter()
            .position(|channel| {
                channel
                    .display_name()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name.trim()))
            })
            .ok_or_else(|| classic_error(format!("unknown WC3 public channel {name:?}")))?;
        self.join(index)
    }

    pub fn send_message(&mut self, text: &str) -> Result<Vec<ChatEvent>> {
        let channel_id = self
            .chat
            .focused_channel
            .ok_or_else(|| classic_error("cannot chat before joining a channel"))?;
        self.send_message_to(channel_id, text)
    }

    pub fn send_message_to(&mut self, channel_id: u8, text: &str) -> Result<Vec<ChatEvent>> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_CHAT_BYTES || text.chars().any(char::is_control) {
            return Err(classic_error(
                "chat message is empty, too long, or contains controls",
            ));
        }
        let wire_channel_id = self
            .chat
            .channel(channel_id)
            .ok_or_else(|| classic_error("WC3 chat channel is no longer joined"))?
            .channel_id
            .clone();
        let message = Message::new().bytes(1, text.as_bytes()).into_vec();
        let body = Message::new()
            .bytes(1, wire_channel_id.expose())
            .bytes(2, &message)
            .into_vec();
        let response = self.client.call(
            &Request::new(AURORA_CHAT, SEND_MESSAGE, &body),
            self.timeout,
        )?;
        for field in protobuf::fields(&response.body) {
            let field = field?;
            if field.number != 1 || !matches!(field.value, Value::Bytes(_)) {
                return Err(classic_error("SendMessage returned an unexpected body"));
            }
        }
        self.dispatch_product_events()
    }

    pub fn leave(&mut self, channel_id: u8) -> Result<Vec<ChatEvent>> {
        let wire_channel_id = self
            .chat
            .channel(channel_id)
            .ok_or_else(|| classic_error("WC3 chat channel is no longer joined"))?
            .channel_id
            .clone();
        let body = Message::new().bytes(1, wire_channel_id.expose()).into_vec();
        let response = self.client.call(
            &Request::new(AURORA_CHAT, LEAVE_CHANNEL, &body),
            self.timeout,
        )?;
        require_empty(&response.body, "LeaveChannel")?;
        let mut events = self.dispatch_product_events()?;
        if self.chat.remove_channel(channel_id).is_some() {
            events.push(ChatEvent::ChannelLeft { channel_id });
        }
        Ok(events)
    }

    pub fn send_whisper(&mut self, account_id: u64, text: &str) -> Result<Vec<ChatEvent>> {
        let body = send_whisper_request(account_id, text)?;
        let response = self.client.call(
            &Request::new(AURORA_CHAT, SEND_WHISPER, &body),
            self.timeout,
        )?;
        for field in protobuf::fields(&response.body) {
            let field = field?;
            if field.number != 1 || !matches!(field.value, Value::Bytes(_)) {
                return Err(classic_error("SendWhisper returned an unexpected body"));
            }
        }
        self.dispatch_product_events()
    }

    pub fn poll(&mut self, timeout: Duration) -> Result<Vec<ChatEvent>> {
        self.client.poll(timeout)?;
        if let Some(cookie) = take_cookie_updates(&mut self.client)? {
            self.latest_cookie = Some(cookie);
        }
        self.dispatch_product_events()
    }

    pub fn dispatch_queued(&mut self) -> Result<Vec<ChatEvent>> {
        if let Some(cookie) = take_cookie_updates(&mut self.client)? {
            self.latest_cookie = Some(cookie);
        }
        self.dispatch_product_events()
    }

    pub fn close(&mut self) -> Result<()> {
        self.client.close()
    }

    fn dispatch_product_events(&mut self) -> Result<Vec<ChatEvent>> {
        let events = self.chat.dispatch(&mut self.client)?;
        self.clan.dispatch(&mut self.client)?;
        if let Some(body) = self.clan.refresh_request() {
            match self
                .client
                .call(&Request::new(CLAN, GET_CLAN_MEMBERS, &body), self.timeout)
            {
                Ok(response) => {
                    if let Err(error) = self.clan.apply_members(&response.body) {
                        self.clan.fail_read(error.to_string());
                    }
                }
                Err(error) => {
                    let _ = self.client.restore_timeout();
                    self.clan.fail_read(error.to_string());
                }
            }
        }
        Ok(events)
    }
}

fn parse_my_clan(body: &[u8]) -> Result<ClanMembership> {
    if body.is_empty() {
        return Ok(ClanMembership::None);
    }
    let info = protobuf::first_bytes(body, 2)
        .ok_or_else(|| classic_error("non-empty ReceivedMyClanOnLogin callback has no ClanInfo"))?;
    let clan_id = protobuf::first_bytes(info, 1)
        .ok_or_else(|| classic_error("GetClanResponse ClanInfo has no ClanId"))?;
    let id = protobuf::first_varint(clan_id, 2)
        .ok_or_else(|| classic_error("GetClanResponse ClanId has no numeric id"))?;
    let tag = optional_validated_text(protobuf::first_bytes(clan_id, 1), "clan tag")?;
    let name = optional_validated_text(protobuf::first_bytes(info, 2), "clan name")?;
    let motd = optional_validated_text(protobuf::first_bytes(info, 3), "clan motd")?;
    let description = optional_validated_text(protobuf::first_bytes(info, 4), "clan description")?;
    Ok(ClanMembership::Member(ClanInfo {
        id,
        tag,
        name,
        motd,
        description,
    }))
}

fn parse_clan_members(body: &[u8], expected_id: u64) -> Result<Vec<ClanMember>> {
    // the response is already correlated to the request by the Classic RPC
    // token. Retail may omit its optional ClanId field; when the field is
    // present, still require and verify its numeric identity.
    if let Some(clan_id) = protobuf::first_bytes(body, 1) {
        let id = protobuf::first_varint(clan_id, 2)
            .ok_or_else(|| classic_error("GetClanMembers ClanId has no numeric id"))?;
        if id != expected_id {
            return Err(classic_error(
                "GetClanMembers returned a roster for a different clan",
            ));
        }
    }
    let mut members = Vec::new();
    for field in protobuf::fields(body) {
        let field = field?;
        if field.number != 2 {
            continue;
        }
        let Value::Bytes(member) = field.value else {
            return Err(classic_error("GetClanMembers member is not a message"));
        };
        if members.len() == MAX_CLAN_MEMBERS {
            return Err(classic_error("clan roster exceeds the safety limit"));
        }
        let member_id = protobuf::first_bytes(member, 1)
            .ok_or_else(|| classic_error("ClanMember has no ClanMemberId"))?;
        let account_id = protobuf::first_varint(member_id, 1)
            .ok_or_else(|| classic_error("ClanMemberId has no account id"))?;
        let name = validated_text(
            protobuf::first_bytes(member_id, 3)
                .ok_or_else(|| classic_error("ClanMemberId has no name"))?,
            "clan member name",
        )?;
        let rank = protobuf::first_varint(member, 2)
            .map(u32::try_from)
            .transpose()
            .map_err(|_| classic_error("ClanMember rank exceeds uint32"))?
            .unwrap_or_default();
        members.push(ClanMember {
            account_id,
            name,
            rank,
        });
    }
    members.sort_unstable_by_key(|member| member.account_id);
    members.dedup_by_key(|member| member.account_id);
    members.sort_unstable_by_key(|member| member.name.to_ascii_lowercase());
    Ok(members)
}

fn optional_validated_text(value: Option<&[u8]>, label: &str) -> Result<String> {
    match value {
        None => Ok(String::new()),
        Some(value) if value.is_empty() => Ok(String::new()),
        Some(value) => validated_text(value, label),
    }
}

fn auth_request(
    endpoint: &ClassicEndpoint,
    connection: &ConnectionSession,
    session: &AuthSession,
    identity: &ClientIdentity,
    cookie: Option<&SecretBytes>,
) -> Result<Zeroizing<Vec<u8>>> {
    let region = connection
        .connected_region
        .ok_or_else(|| classic_error("BGS Connect returned no region"))?;
    let game_account = session.wc3_game_account(Some(region))?;
    let account = Message::new()
        .varint(1, session.account_id)
        .varint(2, u64::from(region))
        .varint(3, game_account.id)
        .varint(4, u64::from(game_account.title_id))
        .into_vec();
    let info = Message::new()
        .bytes(1, session.session_key.expose())
        .varint(2, 131_072)
        .varint(3, u64::from(LOCALE_FOURCC))
        .varint(4, u64::from(PLATFORM_FOURCC))
        .bytes(6, &account)
        .varint(7, u64::from(SESSION_TYPE))
        .bytes(8, connection.ciid.expose())
        .into_vec();
    let mut request = Message::new()
        .bytes(1, endpoint.ticket.expose())
        .bytes(2, &info)
        .varint(3, u64::from(TITLE_ID))
        .bytes(4, GAME_VERSION.as_bytes())
        .bytes(5, identity.as_bytes())
        .varint(6, u64::from(CLIENT_CAPABILITIES))
        .varint(7, u64::from(PLATFORM_FOURCC))
        .varint(8, 1);
    if let Some(cookie) = cookie {
        request = request.bytes(10, cookie.expose());
    }
    Ok(Zeroizing::new(request.into_vec()))
}

fn validate_online_stats(body: &[u8], session_key: &SecretBytes) -> Result<()> {
    let fields = protobuf::fields(body).collect::<Result<Vec<_>>>()?;
    let ciphertext = fields
        .iter()
        .find(|field| field.number == 3)
        .and_then(crate::platform::wire::raw::Field::bytes)
        .ok_or_else(|| {
            let shape = fields
                .iter()
                .map(|field| match field.value {
                    Value::Bytes(value) => {
                        format!("{}:bytes({})", field.number, value.len())
                    }
                    _ => format!("{}:wire({})", field.number, field.value.wire_type()),
                })
                .collect::<Vec<_>>()
                .join(", ");
            Error::Authentication(format!(
                "WC3: Classic AuthSession rejected this Battle.net session ({}-byte response; fields [{}]); retry to sign in with the same Battle.net account used by retail Warcraft III",
                body.len(),
                shape
            ))
        })?;
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err(classic_error(
            "encrypted OnlineStats is not an AES block sequence",
        ));
    }
    let digest = Sha512_256::digest(session_key.expose());
    let cipher = Aes256::new(GenericArray::from_slice(&digest));
    let mut plaintext = Zeroizing::new(ciphertext.to_vec());
    for block in plaintext.as_chunks_mut::<16>().0 {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    let length = plaintext
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(0, |index| index + 1);
    if length == 0 || protobuf::fields(&plaintext[..length]).next().is_none() {
        return Err(classic_error("decrypted OnlineStats is empty"));
    }
    // validate every protobuf field, not only the first.
    protobuf::fields(&plaintext[..length]).collect::<Result<Vec<_>>>()?;
    Ok(())
}

fn take_cookie_updates(client: &mut ClassicClient) -> Result<Option<SecretBytes>> {
    let mut latest = None;
    let mut retained = VecDeque::new();
    while let Some(callback) = client.callbacks.pop_front() {
        if callback.service_id == AUTHENTICATION_SERVICE && callback.method_id == COOKIE_UPDATE {
            let cookie = protobuf::first_bytes(&callback.body, 1)
                .filter(|cookie| !cookie.is_empty() && cookie.len() <= 16 * 1_024)
                .ok_or_else(|| classic_error("CookieUpdate has an invalid cookie"))?;
            latest = Some(SecretBytes::new(cookie.to_vec())?);
        } else {
            retained.push_back(callback);
        }
    }
    client.callbacks = retained;
    Ok(latest)
}

fn validate_toons(body: &[u8]) -> Result<()> {
    for field in protobuf::fields(body) {
        let field = field?;
        let Value::Bytes(toon) = field.value else {
            return Err(classic_error("GetToons returned a non-bytes field"));
        };
        if field.number != 1
            || protobuf::first_varint(toon, 1).is_none()
            || protobuf::first_bytes(toon, 2).is_none_or(<[u8]>::is_empty)
            || protobuf::first_varint(toon, 3).is_none()
        {
            return Err(classic_error("GetToons returned a malformed toon"));
        }
    }
    Ok(())
}

fn parse_public_channels(body: &[u8]) -> Result<Vec<PublicChannel>> {
    let mut channels = Vec::new();
    for field in protobuf::fields(body) {
        let field = field?;
        let Value::Bytes(identifier) = field.value else {
            return Err(classic_error("public channel identifier is not bytes"));
        };
        if field.number != 1
            || identifier.is_empty()
            || identifier.len() > MAX_CHANNEL_IDENTIFIER_BYTES
            || channels.len() == MAX_PUBLIC_CHANNELS
        {
            return Err(classic_error("public channel list is malformed"));
        }
        channels.push(PublicChannel {
            identifier: SecretBytes::new(identifier.to_vec())?,
        });
    }
    Ok(channels)
}

fn parse_channel_added(body: &[u8]) -> Result<ParsedChannel> {
    let info = protobuf::first_bytes(body, 1)
        .ok_or_else(|| classic_error("ChannelAdded has no channel info"))?;
    let channel_id = protobuf::first_bytes(info, 1)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| classic_error("channel has no id"))?;
    let name = validated_text(
        protobuf::first_bytes(info, 2).ok_or_else(|| classic_error("channel has no name"))?,
        "channel name",
    )?;
    let mut members = Vec::new();
    for field in protobuf::fields(info) {
        let field = field?;
        if field.number != 5 {
            continue;
        }
        let Value::Bytes(member) = field.value else {
            return Err(classic_error("channel member is not bytes"));
        };
        if members.len() == MAX_MEMBERS {
            return Err(classic_error("channel roster exceeds the safety limit"));
        }
        let member = parse_member_info(member)?;
        if !members
            .iter()
            .any(|current: &ChatMember| current.identity == member.identity)
        {
            members.push(member);
        }
    }
    sort_members(&mut members);
    Ok(ParsedChannel {
        channel_id: SecretBytes::new(channel_id.to_vec())?,
        name,
        members,
    })
}

fn parse_member_info(body: &[u8]) -> Result<ChatMember> {
    let name = validated_text(
        protobuf::first_bytes(body, 2)
            .ok_or_else(|| classic_error("channel member has no name"))?,
        "member name",
    )?;
    let handle = protobuf::first_varint(body, 6)
        .ok_or_else(|| classic_error("channel member has no handle"))?;
    let identity = protobuf::first_bytes(body, 7)
        .ok_or_else(|| classic_error("channel member has no toon identity"))?;
    Ok(ChatMember {
        handle,
        name,
        presence: ChatPresence::Online,
        avatar_id: None,
        clan_abbreviation: None,
        identity: parse_member_identity(identity)?,
    })
}

fn parse_member_identity(body: &[u8]) -> Result<ChatMemberIdentity> {
    let account_id = protobuf::first_varint(body, 1)
        .ok_or_else(|| classic_error("toon identity has no account id"))?;
    let title_id = protobuf::first_varint(body, 2)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| classic_error("toon identity has no valid title id"))?;
    let region = protobuf::first_varint(body, 3)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| classic_error("toon identity has no valid region"))?;
    Ok(ChatMemberIdentity {
        account_id,
        title_id,
        region,
    })
}

fn apply_member_presence(channel: &mut ChatChannel, body: &[u8]) -> Result<()> {
    if body.is_empty() {
        return Ok(());
    }
    let identity = protobuf::first_bytes(body, 2)
        .ok_or_else(|| classic_error("member presence has no toon identity"))?;
    let identity = parse_member_identity(identity)?;
    let Some(member) = channel
        .members
        .iter_mut()
        .find(|member| member.identity == identity)
    else {
        return Ok(());
    };

    if let Some(avatar_id) = protobuf::first_bytes(body, 4) {
        member.avatar_id = parse_avatar_id(avatar_id)?;
    }
    if let Some(clan) = protobuf::first_bytes(body, 5) {
        member.clan_abbreviation = if clan.is_empty() {
            None
        } else {
            Some(validated_text(clan, "clan abbreviation")?)
        };
    }
    if let Some(status) = protobuf::first_bytes(body, 9) {
        // OnlineStatusV3 is four protobuf booleans. The retail SDK bridge
        // exposes field 1 as online, field 3 as busy, and field 4 as away;
        // field 2 is retained by the SDK but is not a WebUI activity state.
        let online = protobuf_bool(status, 1)?;
        let busy = protobuf_bool(status, 3)?;
        let away = protobuf_bool(status, 4)?;
        member.presence = if !online {
            ChatPresence::Offline
        } else if away {
            ChatPresence::Away
        } else if busy {
            ChatPresence::Busy
        } else {
            ChatPresence::Online
        };
    }
    Ok(())
}

fn protobuf_bool(body: &[u8], number: u32) -> Result<bool> {
    match protobuf::first_varint(body, number).unwrap_or(0) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(classic_error("OnlineStatusV3 contains a non-boolean value")),
    }
}

fn parse_avatar_id(value: &[u8]) -> Result<Option<String>> {
    if value.is_empty() {
        return Ok(None);
    }
    let value = validated_text(value, "avatar id")?;
    let valid = value.strip_prefix('p').is_some_and(|digits| {
        digits.len() == 3 && digits.bytes().all(|byte| byte.is_ascii_digit())
    });
    if !valid {
        return Err(classic_error("member presence has an invalid avatar id"));
    }
    Ok(Some(value))
}

fn find_member_info(body: &[u8]) -> Result<Option<ChatMember>> {
    for field in protobuf::fields(body) {
        let field = field?;
        let Some(candidate) = field.bytes() else {
            continue;
        };
        if protobuf::first_varint(candidate, 4).is_some()
            && protobuf::first_varint(candidate, 6).is_some()
            && protobuf::first_bytes(candidate, 7).is_some()
            && let Ok(member) = parse_member_info(candidate)
        {
            return Ok(Some(member));
        }
    }
    Ok(None)
}

fn find_member_name(body: &[u8]) -> Result<Option<String>> {
    if let Some(member) = find_member_info(body)? {
        return Ok(Some(member.name));
    }
    for field in protobuf::fields(body) {
        let field = field?;
        let Some(candidate) = field.bytes() else {
            continue;
        };
        if protobuf::first_varint(candidate, 4).is_some()
            && let Some(name) = protobuf::first_bytes(candidate, 2)
            && let Ok(name) = validated_text(name, "member name")
        {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

fn parse_chat_message(body: &[u8]) -> Result<String> {
    let message = protobuf::first_bytes(body, 2)
        .ok_or_else(|| classic_error("chat callback has no message"))?;
    validated_text(
        protobuf::first_bytes(message, 1)
            .ok_or_else(|| classic_error("chat message has no text"))?,
        "chat message",
    )
}

/// builds WC3's `aurora_chat.SendWhisperRequest` from the installed retail
/// SDK's generated serializer: account id is varint field 1 and message is
/// string field 2.
fn send_whisper_request(account_id: u64, text: &str) -> Result<Vec<u8>> {
    if account_id == 0 {
        return Err(classic_error("whisper account id must be positive"));
    }
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_CHAT_BYTES || text.chars().any(char::is_control) {
        return Err(classic_error(
            "whisper message is empty, too long, or contains controls",
        ));
    }
    Ok(Message::new()
        .varint(1, account_id)
        .bytes(2, text.as_bytes())
        .into_vec())
}

struct ParsedWhisper {
    account_id: u64,
    peer: Option<String>,
    body: String,
}

/// WC3 uses one generated `WhisperReceivedRequest` for receive and echo. The
/// retail bridge reads account id from field 1 and message from field 2; an
/// inbound callback additionally supplies the sender BattleTag in field 3.
fn parse_whisper(body: &[u8], outgoing: bool) -> Result<ParsedWhisper> {
    let account_id = protobuf::first_varint(body, 1)
        .filter(|account_id| *account_id != 0)
        .ok_or_else(|| classic_error("whisper callback has no account id"))?;
    let message = protobuf::first_bytes(body, 2)
        .ok_or_else(|| classic_error("whisper callback has no message"))?;
    let peer = protobuf::first_bytes(body, 3)
        .map(|value| validated_text(value, "whisper peer"))
        .transpose()?;
    if !outgoing && peer.is_none() {
        return Err(classic_error("received whisper has no sender BattleTag"));
    }
    Ok(ParsedWhisper {
        account_id,
        peer,
        body: validated_text(message, "whisper message")?,
    })
}

enum FriendUpdate {
    Upsert(ChatFriend),
    Remove(u64),
}

/// `BatchFriendUpdatedRequest` repeats `FriendUpdatedRequest` in field 1.
/// Each update carries `FriendInfo` in field 1 and `FriendRole` in field 2.
/// The installed retail SDK's batch bridge adds role 0 to the friend cache and
/// removes role 1 by the `FriendInfo` account ID; other roles are rejected
/// until their meaning is established. The `FriendInfo` serializer defines
/// account id, BattleTag, current program, and in-program as fields 1, 2, 4,
/// and 5 respectively.
fn parse_friend_update(body: &[u8]) -> Result<FriendUpdate> {
    let friend = protobuf::first_bytes(body, 1)
        .ok_or_else(|| classic_error("friend update has no FriendInfo"))?;
    let account_id = protobuf::first_varint(friend, 1)
        .filter(|account_id| *account_id != 0)
        .ok_or_else(|| classic_error("FriendInfo has no account id"))?;
    match protobuf::first_varint(body, 2).unwrap_or(0) {
        1 => return Ok(FriendUpdate::Remove(account_id)),
        0 => {}
        _ => return Err(classic_error("friend update has an unknown FriendRole")),
    }
    let name = validated_text(
        protobuf::first_bytes(friend, 2)
            .ok_or_else(|| classic_error("FriendInfo has no BattleTag"))?,
        "friend BattleTag",
    )?;
    let in_program = protobuf_bool(friend, 5)?;
    Ok(FriendUpdate::Upsert(ChatFriend {
        account_id,
        name,
        presence: if in_program {
            FriendPresence::Online
        } else {
            FriendPresence::Offline
        },
    }))
}

/// metadata-only protocol tracing for live validation. Raw callback bodies,
/// identifiers, names, and chat text deliberately never enter the trace.
fn trace_classic(message: impl std::fmt::Display) {
    let message = message.to_string();
    if crate::trace_enabled() {
        eprintln!("superiority: {message}");
    }
    if let Some(path) = std::env::var_os("SUPERIORITY_TRACE_FILE")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        use std::io::Write as _;
        let _ = writeln!(file, "superiority: {message}");
    }
}

fn validated_text(value: &[u8], role: &str) -> Result<String> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES.max(MAX_CHAT_BYTES) {
        return Err(classic_error(format!("{role} has an invalid length")));
    }
    let text =
        std::str::from_utf8(value).map_err(|_| classic_error(format!("{role} is not UTF-8")))?;
    if text.chars().any(char::is_control) {
        return Err(classic_error(format!("{role} contains control characters")));
    }
    Ok(text.into())
}

fn sort_members(members: &mut [ChatMember]) {
    members.sort_unstable_by_key(|member| member.name.to_lowercase());
}

fn require_empty(body: &[u8], label: &str) -> Result<()> {
    if body.is_empty() {
        Ok(())
    } else {
        Err(classic_error(format!(
            "{label} returned an unexpected body"
        )))
    }
}

fn request_trace() -> Vec<u8> {
    let mut raw = [0_u8; 16];
    rand::rng().fill_bytes(&mut raw);
    let group = |bytes: &[u8]| {
        bytes
            .iter()
            .fold(0_u64, |value, byte| value << 8 | u64::from(*byte))
    };
    format!(
        "RT-{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
        group(&raw[..4]),
        group(&raw[4..6]),
        group(&raw[6..8]),
        group(&raw[8..10]),
        group(&raw[10..])
    )
    .into_bytes()
}

fn classic_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(format!("WC3: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_recovered_roster_and_message_shapes() {
        let identity = Message::new()
            .varint(1, 42)
            .varint(2, u64::from(TITLE_ID))
            .varint(3, 1)
            .into_vec();
        let alice = Message::new()
            .bytes(2, b"Alice")
            .varint(4, 1)
            .varint(6, 77)
            .bytes(7, &identity)
            .into_vec();
        let info = Message::new()
            .bytes(1, b"opaque")
            .bytes(2, b"General")
            .bytes(5, &alice)
            .into_vec();
        let channel = parse_channel_added(&Message::new().bytes(1, &info).into_vec()).unwrap();
        assert_eq!(channel.name, "General");
        assert_eq!(channel.members.len(), 1);
        assert_eq!(channel.members[0].name, "Alice");
        assert_eq!(channel.members[0].handle, 77);

        let message = Message::new().bytes(1, b"hello").into_vec();
        let inbound = Message::new().bytes(2, &message).into_vec();
        assert_eq!(parse_chat_message(&inbound).unwrap(), "hello");
    }

    #[test]
    fn retains_multiple_channels_and_routes_callback_ids_independently() {
        let mut state = ChatState::default();
        let general_wire = Message::new().bytes(2, b"general").varint(4, 1).into_vec();
        let clan_wire = Message::new().bytes(2, b"clan").varint(4, 2).into_vec();

        let general = state
            .reserve_channel(
                SecretBytes::new(general_wire.clone()).unwrap(),
                "General".into(),
            )
            .unwrap();
        let clan = state
            .reserve_channel(SecretBytes::new(clan_wire.clone()).unwrap(), "Clan".into())
            .unwrap();

        assert_ne!(general, clan);
        assert_eq!(state.channels.len(), 2);
        assert_eq!(state.focused_channel, Some(clan));
        assert_eq!(
            state.callback_channel_id(&Message::new().bytes(1, &general_wire).into_vec()),
            Some(general)
        );
        assert_eq!(
            state.callback_channel_id(&Message::new().bytes(1, &clan_wire).into_vec()),
            Some(clan)
        );

        let (announced, first) = state
            .apply_channel(ParsedChannel {
                channel_id: SecretBytes::new(general_wire.clone()).unwrap(),
                name: "General".into(),
                members: Vec::new(),
            })
            .unwrap();
        assert_eq!(announced, general);
        assert!(first);
        let (_, first) = state
            .apply_channel(ParsedChannel {
                channel_id: SecretBytes::new(general_wire).unwrap(),
                name: "General".into(),
                members: Vec::new(),
            })
            .unwrap();
        assert!(!first);
        assert_eq!(state.announced_channels().count(), 1);

        assert_eq!(state.remove_channel(clan).unwrap().id, clan);
        assert_eq!(state.channels.len(), 1);
        assert_eq!(state.focused_channel, Some(general));
    }

    #[test]
    fn member_callbacks_skip_the_binary_channel_id_before_the_member() {
        // MemberAdded begins with a ChatChannelId. It also has fields 2 and 4,
        // but field 2 is an opaque nested identifier rather than UTF-8. The
        // actual ChatMemberInfo follows it and carries the display name.
        let channel_id = Message::new()
            .bytes(2, &[0xff, 0x00, 0x81])
            .varint(4, 1)
            .into_vec();
        let identity = Message::new()
            .varint(1, 42)
            .varint(2, u64::from(TITLE_ID))
            .varint(3, 1)
            .into_vec();
        let member = Message::new()
            .bytes(2, b"Alice")
            .varint(4, 1)
            .varint(6, 77)
            .bytes(7, &identity)
            .into_vec();
        let callback = Message::new()
            .bytes(1, &channel_id)
            .bytes(2, &member)
            .into_vec();
        assert_eq!(
            find_member_name(&callback).unwrap().as_deref(),
            Some("Alice")
        );
        assert_eq!(
            find_member_name(&Message::new().bytes(1, &channel_id).into_vec()).unwrap(),
            None
        );
    }

    #[test]
    fn merges_recovered_presence_and_avatar_by_toon_identity() {
        let identity = Message::new()
            .varint(1, 42)
            .varint(2, u64::from(TITLE_ID))
            .varint(3, 1)
            .into_vec();
        let member = Message::new()
            .bytes(2, b"Alice")
            .varint(4, 1)
            .varint(6, 77)
            .bytes(7, &identity)
            .into_vec();
        let info = Message::new()
            .bytes(1, b"opaque")
            .bytes(2, b"General")
            .bytes(5, &member)
            .into_vec();
        let parsed = parse_channel_added(&Message::new().bytes(1, &info).into_vec()).unwrap();
        let mut channel = ChatChannel {
            id: 0,
            name: parsed.name,
            members: parsed.members,
        };
        let status = Message::new()
            .varint(1, 1)
            .varint(2, 1)
            .varint(3, 0)
            .varint(4, 1)
            .into_vec();
        let update = Message::new()
            .bytes(2, &identity)
            .varint(3, 3)
            .bytes(4, b"p126")
            .bytes(5, b"MOB")
            .varint(8, 10)
            .bytes(9, &status)
            .into_vec();
        apply_member_presence(&mut channel, &update).unwrap();
        assert_eq!(channel.members[0].presence, ChatPresence::Away);
        assert_eq!(channel.members[0].avatar_id.as_deref(), Some("p126"));
        assert_eq!(channel.members[0].clan_abbreviation.as_deref(), Some("MOB"));
    }

    #[test]
    fn public_identifiers_are_preserved_but_redacted() {
        let body = Message::new().bytes(1, b"General").into_vec();
        let channels = parse_public_channels(&body).unwrap();
        assert_eq!(channels[0].display_name(), Some("General"));
        assert!(!format!("{:?}", channels[0].identifier).contains("General"));
    }

    #[test]
    fn builds_the_retail_wc3_whisper_request() {
        assert_eq!(
            send_whisper_request(300, "  hello  ").unwrap(),
            [0x08, 0xac, 0x02, 0x12, 0x05, b'h', b'e', b'l', b'l', b'o']
        );
        assert!(send_whisper_request(0, "hello").is_err());
        assert!(send_whisper_request(42, "\n").is_err());
    }

    #[test]
    fn decodes_receive_and_echo_whispers_without_guessing_identity() {
        let incoming = Message::new()
            .varint(1, 42)
            .bytes(2, b"hello")
            .bytes(3, b"Friend#1234")
            .into_vec();
        let incoming = parse_whisper(&incoming, false).unwrap();
        assert_eq!(incoming.account_id, 42);
        assert_eq!(incoming.peer.as_deref(), Some("Friend#1234"));
        assert_eq!(incoming.body, "hello");

        let echo = Message::new().varint(1, 42).bytes(2, b"hi back").into_vec();
        let echo = parse_whisper(&echo, true).unwrap();
        assert_eq!(echo.account_id, 42);
        assert_eq!(echo.peer, None);
        assert_eq!(echo.body, "hi back");
        let missing_sender = Message::new().varint(1, 42).bytes(2, b"hello").into_vec();
        assert!(parse_whisper(&missing_sender, false).is_err());
    }

    #[test]
    fn decodes_wc3_batch_friend_updates() {
        let friend = Message::new()
            .varint(1, 42)
            .bytes(2, b"Friend#1234")
            .bytes(4, b"W3")
            .varint(5, 1)
            .into_vec();
        let update = Message::new().bytes(1, &friend).varint(2, 0).into_vec();
        let FriendUpdate::Upsert(update) = parse_friend_update(&update).unwrap() else {
            panic!("role 0 was not decoded as an upsert");
        };
        assert_eq!(
            update,
            ChatFriend {
                account_id: 42,
                name: "Friend#1234".into(),
                presence: FriendPresence::Online,
            }
        );

        let removed_friend = Message::new().varint(1, 42).into_vec();
        let removed = Message::new()
            .bytes(1, &removed_friend)
            .varint(2, 1)
            .into_vec();
        assert!(matches!(
            parse_friend_update(&removed).unwrap(),
            FriendUpdate::Remove(42)
        ));
        let unknown = Message::new().bytes(1, &friend).varint(2, 2).into_vec();
        assert!(parse_friend_update(&unknown).is_err());
    }

    #[test]
    fn empty_received_my_clan_callback_is_authoritative_no_membership() {
        assert_eq!(parse_my_clan(&[]).unwrap(), ClanMembership::None);
        assert!(parse_my_clan(&Message::new().varint(4, 1).into_vec()).is_err());
    }

    #[test]
    fn decodes_the_retail_clan_descriptor_and_roster_shapes() {
        let clan_id = Message::new().bytes(1, b"MOB").varint(2, 91).into_vec();
        let clan_info = Message::new()
            .bytes(1, &clan_id)
            .bytes(2, b"Mortar Board")
            .bytes(3, b"Ready for battle")
            .bytes(4, b"A test clan")
            .into_vec();
        let response = Message::new().bytes(2, &clan_info).into_vec();
        assert_eq!(
            parse_my_clan(&response).unwrap(),
            ClanMembership::Member(ClanInfo {
                id: 91,
                tag: "MOB".into(),
                name: "Mortar Board".into(),
                motd: "Ready for battle".into(),
                description: "A test clan".into(),
            })
        );

        // retail may serialize an optional descriptor string as a present,
        // zero-length protobuf field. That is the same semantic value as an
        // omitted optional string, not a malformed required display name.
        let empty_description = Message::new()
            .bytes(1, &clan_id)
            .bytes(2, b"Mortar Board")
            .bytes(3, b"Ready for battle")
            .bytes(4, b"")
            .into_vec();
        let response = Message::new().bytes(2, &empty_description).into_vec();
        let ClanMembership::Member(info) = parse_my_clan(&response).unwrap() else {
            panic!("descriptor with an empty optional description lost membership");
        };
        assert_eq!(info.description, "");

        let alice_id = Message::new()
            .varint(1, 7)
            .varint(2, u64::from(TITLE_ID))
            .bytes(3, b"Alice")
            .varint(4, 1)
            .into_vec();
        let bob_id = Message::new()
            .varint(1, 8)
            .varint(2, u64::from(TITLE_ID))
            .bytes(3, b"Bob")
            .varint(4, 1)
            .into_vec();
        let alice = Message::new().bytes(1, &alice_id).varint(2, 3).into_vec();
        let bob = Message::new().bytes(1, &bob_id).varint(2, 1).into_vec();
        let roster = Message::new()
            .bytes(1, &clan_id)
            .bytes(2, &bob)
            .bytes(2, &alice)
            .into_vec();
        assert_eq!(
            parse_clan_members(&roster, 91).unwrap(),
            vec![
                ClanMember {
                    account_id: 7,
                    name: "Alice".into(),
                    rank: 3,
                },
                ClanMember {
                    account_id: 8,
                    name: "Bob".into(),
                    rank: 1,
                },
            ]
        );
        assert!(parse_clan_members(&roster, 92).is_err());

        let roster_without_id = Message::new().bytes(2, &bob).bytes(2, &alice).into_vec();
        assert_eq!(
            parse_clan_members(&roster_without_id, 91).unwrap(),
            parse_clan_members(&roster, 91).unwrap()
        );
        assert!(parse_clan_members(&[], 91).unwrap().is_empty());
    }
}
