// classic service bodies are protobuf lite, so field meanings come from the
// sdk's registration table and the retail client's consumers. decoding is
// tolerant: unmodelled fields are ignored and an unreadable record is skipped,
// leaving the rest of the callback intact.

use std::collections::{BTreeMap, BTreeSet};

use super::profile::Avatar;
use crate::{
    Error, Result,
    games::scr::{
        catalog::{method, service},
        rpc::Frame,
    },
    platform::wire::raw::{self as protobuf, Message},
};

pub const MAX_MESSAGE_BYTES: usize = 255;
pub const MAX_CHANNEL_NAME_BYTES: usize = 255;
pub const MAX_COMMAND_ARGUMENT_BYTES: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Whisper,
    Talk,
    Broadcast,
    Information,
    Error,
    Emote,
}

impl EventKind {
    #[must_use]
    pub fn from_method_id(method_id: u32) -> Option<Self> {
        Some(match method_id {
            method::CHAT_WHISPER_MESSAGE => Self::Whisper,
            method::CHAT_TALK_MESSAGE => Self::Talk,
            method::CHAT_BROADCAST_MESSAGE => Self::Broadcast,
            method::CHAT_INFORMATION_MESSAGE => Self::Information,
            method::CHAT_ERROR_MESSAGE => Self::Error,
            method::CHAT_EMOTE_MESSAGE => Self::Emote,
            _ => return None,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChatAttribute {
    pub name: String,
    pub value: String,
}

// Attribute values can contain account identifiers (including BattleTags) and
// rich-presence payloads. Keep them available to product adapters, but do not
// spill them into routine debug and error logs.
impl std::fmt::Debug for ChatAttribute {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChatAttribute")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatUser {
    pub name: String,
    pub flags: Option<u64>,
    pub is_operator: bool,
    /// The selected SC:R profile avatar, hydrated through ToonProfile after
    /// LegacyChat supplies the roster identity.
    pub avatar: Option<Avatar>,
    /// Product-owned identity and presence data attached by LegacyChat.
    ///
    /// These are structured name/value pairs, not opaque padding. Avatar
    /// lookup is a separate ToonProfile/Url service, but the profile and
    /// presence adapters need these values to identify and describe a member.
    pub attributes: Vec<ChatAttribute>,
}

impl ChatUser {
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| attribute.value.as_str())
    }

    /// The account this toon belongs to, when LegacyChat attached it. It is the
    /// only thing that ties a roster entry to the signed-in account — the toon
    /// name and the BattleTag need not resemble each other.
    #[must_use]
    pub fn battle_tag(&self) -> Option<&str> {
        self.attribute("battle_tag")
    }

    /// What this member is doing, read from the presence attributes LegacyChat
    /// carries. The attribute names are the edge's own; this is the one place
    /// they are spelled, so every surface that shows presence agrees.
    #[must_use]
    pub fn presence(&self) -> MemberPresence {
        let status = self
            .attribute("presence")
            .or_else(|| self.attribute("status"))
            .map(str::trim)
            .unwrap_or_default();
        if status.eq_ignore_ascii_case("away") {
            return MemberPresence::Away;
        }
        if status.eq_ignore_ascii_case("busy")
            || status.eq_ignore_ascii_case("dnd")
            || status.eq_ignore_ascii_case("do_not_disturb")
        {
            return MemberPresence::Busy;
        }
        if self
            .attribute("game_info")
            .is_some_and(|value| !value.trim().is_empty())
        {
            return MemberPresence::InGame;
        }
        if self
            .attribute("lobby_info")
            .is_some_and(|value| !value.trim().is_empty())
        {
            return MemberPresence::InLobby;
        }
        MemberPresence::Online
    }
}

/// A channel member's presence, as the classic edge describes it. Remastered
/// has no "offline" here: a member who is not online is not in the roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberPresence {
    Online,
    Away,
    Busy,
    InGame,
    InLobby,
}

impl MemberPresence {
    /// The word Live's wire uses for it.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Online => "available",
            Self::Away => "away",
            Self::Busy => "busy",
            Self::InGame => "in_game",
            Self::InLobby => "in_lobby",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatChannel {
    pub channel_id: u32,
    pub name: String,
    pub display_name: Option<String>,
    pub is_public: bool,
    pub users: Vec<ChatUser>,
}

impl ChatChannel {
    #[must_use]
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEvent {
    pub kind: EventKind,
    pub channel_id: Option<u32>,
    pub sender: Option<String>,
    pub text: Option<String>,
    /// Account identity attached by SC:R's AuroraChat whisper callbacks.
    /// LegacyChat events leave this absent because they address toon names.
    pub aurora_whisper: Option<AuroraWhisper>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuroraWhisper {
    pub account_id: u32,
    pub outgoing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendPresence {
    Online,
    InGame,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFriend {
    pub name: String,
    pub presence: FriendPresence,
    pub game: Option<String>,
    /// AuroraFriends' stable account identity, when this is an account friend.
    pub account_id: Option<u32>,
}

/// The server-owned classic command catalogue fetched after chat connects.
///
/// Retail requests both lists before rendering `/help`. Keep their ordering:
/// the whitelist response is already arranged the way the client presents it,
/// including short aliases beside their long forms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandCatalog {
    whitelist: Vec<String>,
    blacklist: Vec<String>,
}

impl CommandCatalog {
    #[must_use]
    pub fn from_responses(whitelist: &[u8], blacklist: &[u8]) -> Self {
        Self {
            whitelist: parse_command_list(whitelist),
            blacklist: parse_command_list(blacklist),
        }
    }

    #[must_use]
    pub fn whitelist(&self) -> &[String] {
        &self.whitelist
    }

    #[must_use]
    pub fn blacklist(&self) -> &[String] {
        &self.blacklist
    }

    #[must_use]
    pub fn allows(&self, command: &str) -> bool {
        let command = command.trim().trim_start_matches('/');
        self.whitelist
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(command))
    }

    #[must_use]
    pub fn help_text(&self) -> String {
        if self.whitelist.is_empty() {
            return "Battle.net did not advertise any chat commands.".to_owned();
        }
        let commands = self
            .whitelist
            .iter()
            .map(|command| format!("/{command}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("Available commands: {commands}")
    }

    pub(crate) fn replace_whitelist(&mut self, response: &[u8]) -> bool {
        let next = parse_command_list(response);
        if self.whitelist == next {
            return false;
        }
        self.whitelist = next;
        true
    }

    pub(crate) fn replace_blacklist(&mut self, response: &[u8]) -> bool {
        let next = parse_command_list(response);
        if self.blacklist == next {
            return false;
        }
        self.blacklist = next;
        true
    }
}

#[derive(Debug, Default)]
pub struct ChatState {
    channels: BTreeMap<u32, ChatChannel>,
    friends: BTreeMap<String, ChatFriend>,
    aurora_friend_keys: BTreeMap<u64, String>,
    events: Vec<ChatEvent>,
    roster_revision: u64,
    friends_revision: u64,
}

impl ChatState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn channels(&self) -> impl Iterator<Item = &ChatChannel> {
        self.channels.values()
    }

    #[must_use]
    pub fn channel(&self, channel_id: u32) -> Option<&ChatChannel> {
        self.channels.get(&channel_id)
    }

    #[must_use]
    pub fn find_channel(&self, name: &str) -> Option<&ChatChannel> {
        self.channels.values().find(|channel| {
            channel.name.eq_ignore_ascii_case(name)
                || channel
                    .display_name
                    .as_deref()
                    .is_some_and(|display| display.eq_ignore_ascii_case(name))
        })
    }

    pub fn friends(&self) -> impl Iterator<Item = &ChatFriend> {
        self.friends.values()
    }

    #[must_use]
    pub fn friends_revision(&self) -> u64 {
        self.friends_revision
    }

    // battle.net confirms a join by sending a ChannelsUpdated callback; the
    // session waits for this to advance.
    #[must_use]
    pub fn roster_revision(&self) -> u64 {
        self.roster_revision
    }

    #[must_use]
    pub fn take_events(&mut self) -> Vec<ChatEvent> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn push_information(&mut self, text: impl Into<String>) {
        self.events.push(ChatEvent {
            kind: EventKind::Information,
            channel_id: None,
            sender: None,
            text: Some(text.into()),
            aurora_whisper: None,
        });
    }

    /// Merges ToonProfile data into the LegacyChat roster without pretending
    /// the profile value was a chat attribute on the wire.
    pub(crate) fn set_avatar(
        &mut self,
        channel_id: u32,
        toon: &str,
        avatar: Option<Avatar>,
    ) -> bool {
        let Some(user) = self.channels.get_mut(&channel_id).and_then(|channel| {
            channel
                .users
                .iter_mut()
                .find(|user| user.name.eq_ignore_ascii_case(toon))
        }) else {
            return false;
        };
        if user.avatar == avatar {
            return false;
        }
        user.avatar = avatar;
        true
    }

    pub fn apply(&mut self, frame: &Frame) -> bool {
        let header = &frame.header;
        if header.is_response() {
            return false;
        }
        if header.service_id == service::AURORA_FRIENDS {
            return header.method_id == method::FRIEND_UPDATED
                && self.apply_aurora_friend_updated(&frame.body);
        }
        if header.service_id == service::AURORA_CHAT {
            return matches!(
                header.method_id,
                method::WHISPER_RECEIVED | method::WHISPER_ECHO_RECEIVED
            ) && self.apply_aurora_whisper(header.method_id, &frame.body);
        }
        if header.service_id != service::LEGACY_CHAT {
            return false;
        }
        match header.method_id {
            method::CHANNELS_UPDATED => {
                self.apply_channels_updated(&frame.body);
                self.roster_revision += 1;
                true
            }
            method::LEFT_CHANNEL => protobuf::first_varint(&frame.body, 1)
                .and_then(|id| u32::try_from(id).ok())
                .is_some_and(|id| self.channels.remove(&id).is_some()),
            method::FORCE_JOIN_CHANNEL => {
                let Some(channel) = parse_force_join_channel(&frame.body) else {
                    return false;
                };
                self.channels.insert(channel.channel_id, channel);
                self.roster_revision += 1;
                true
            }
            method::CHAT_FRIEND_ENTER
            | method::CHAT_FRIEND_EXIT
            | method::CHAT_FRIEND_NOTIFY_GAME => {
                let Some((mut friend, text)) = friend_update(header.method_id, &frame.body) else {
                    return false;
                };
                let key = friend.name.to_ascii_lowercase();
                // A same-named LegacyChat presence update must not erase the
                // stable account route previously supplied by AuroraFriends.
                friend.account_id = self.friends.get(&key).and_then(|friend| friend.account_id);
                if self.friends.get(&key) != Some(&friend) {
                    self.friends.insert(key, friend);
                    self.friends_revision = self.friends_revision.wrapping_add(1);
                }
                self.push_information(text);
                true
            }
            method_id => match EventKind::from_method_id(method_id) {
                Some(kind) => {
                    self.events.push(parse_event(kind, &frame.body));
                    true
                }
                None => false,
            },
        }
    }

    /// Applies the account-level friend roster delivered before LegacyChat
    /// starts producing activity notices.
    ///
    /// The retail client names FriendInfo fields 3-5 `fullName`,
    /// `currentProgram`, and `inProgram`. Live RPC captures establish fields 1
    /// and 2 as the stable account ID and BattleTag. Friend role zero is the
    /// roster role observed for both the initial snapshot and later presence
    /// changes; other roles remain ignored until their meaning is established.
    fn apply_aurora_friend_updated(&mut self, body: &[u8]) -> bool {
        let Some(update) = parse_aurora_friend_update(body) else {
            return false;
        };
        if update.role != 0 {
            return false;
        }

        let key = update.battle_tag.to_ascii_lowercase();
        if let Some(previous_key) = self
            .aurora_friend_keys
            .insert(update.account_id, key.clone())
            .filter(|previous_key| previous_key != &key)
        {
            self.friends.remove(&previous_key);
        }
        let friend = ChatFriend {
            name: update.battle_tag,
            presence: if update.in_program {
                FriendPresence::Online
            } else {
                FriendPresence::Offline
            },
            game: None,
            account_id: u32::try_from(update.account_id).ok(),
        };
        if self.friends.get(&key) != Some(&friend) {
            self.friends.insert(key, friend);
            self.friends_revision = self.friends_revision.wrapping_add(1);
        }
        true
    }

    /// Applies SC:R's account-level whisper callback. The installed SC:R SDK's
    /// protobuf-lite parser requires field 1 as fixed32 and field 2 as a
    /// length-delimited string for both receive and echo callbacks.
    fn apply_aurora_whisper(&mut self, method_id: u32, body: &[u8]) -> bool {
        let Some(account_id) = protobuf::first_fixed32(body, 1) else {
            return false;
        };
        let Some(message) = protobuf::first_bytes(body, 2).and_then(text) else {
            return false;
        };
        let sender = self
            .aurora_friend_keys
            .get(&u64::from(account_id))
            .and_then(|key| self.friends.get(key))
            .map(|friend| friend.name.clone())
            .unwrap_or_else(|| format!("Battle.net account {account_id}"));
        self.events.push(ChatEvent {
            kind: EventKind::Whisper,
            channel_id: None,
            sender: Some(sender),
            text: Some(message),
            aurora_whisper: Some(AuroraWhisper {
                account_id,
                outgoing: method_id == method::WHISPER_ECHO_RECEIVED,
            }),
        });
        true
    }

    fn apply_channels_updated(&mut self, body: &[u8]) {
        for update in protobuf::fields(body)
            .flatten()
            .filter(|field| field.number == 1)
            .filter_map(|field| field.bytes())
        {
            let mut operation = 0;
            let mut channel = None;
            for field in protobuf::fields(update).flatten() {
                match field.number {
                    1 => operation = field.varint().unwrap_or(0),
                    2 => channel = field.bytes().and_then(parse_channel),
                    _ => {}
                }
            }
            let Some(channel) = channel else { continue };
            if operation == 1 {
                self.channels.remove(&channel.channel_id);
            } else {
                self.channels.insert(channel.channel_id, channel);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct AuroraFriendUpdate {
    account_id: u64,
    battle_tag: String,
    in_program: bool,
    role: u64,
}

fn parse_aurora_friend_update(body: &[u8]) -> Option<AuroraFriendUpdate> {
    let friend = protobuf::first_bytes(body, 1)?;
    let mut account_id = None;
    let mut battle_tag = None;
    let mut in_program = false;
    for field in protobuf::fields(friend).flatten() {
        match field.number {
            1 => account_id = field.varint(),
            2 => battle_tag = field.bytes().and_then(text),
            5 => in_program = field.varint().is_some_and(|value| value != 0),
            _ => {}
        }
    }
    Some(AuroraFriendUpdate {
        account_id: account_id?,
        battle_tag: battle_tag?,
        in_program,
        role: protobuf::first_varint(body, 2)?,
    })
}

pub fn send_message_request(channel_id: u32, text: &str) -> Result<Vec<u8>> {
    let message = validated_message(text)?;
    Ok(Message::new()
        .varint(1, u64::from(channel_id))
        .bytes(2, message.as_bytes())
        .into_vec())
}

/// Builds the shared `SendMessageRequest` for `SendMessageToAllFriends`.
///
/// The generated SDK uses the same request type as channel talk. Its
/// one-argument friends method sets message field 2 and leaves channel field 1
/// absent, which distinguishes a friends broadcast from channel `0`.
pub fn send_message_to_all_friends_request(text: &str) -> Result<Vec<u8>> {
    let message = validated_message(text)?;
    Ok(Message::new().bytes(2, message.as_bytes()).into_vec())
}

/// Builds SC:R's `aurora_chat.SendWhisperRequest`. The retail SDK's concrete
/// method takes `(unsigned int, char const*)`; its generated serializer writes
/// that account id as fixed32 field 1 and the message as string field 2.
pub fn send_account_whisper_request(account_id: u32, text: &str) -> Result<Vec<u8>> {
    let message = validated_message(text)?;
    Ok(Message::new()
        .fixed32(1, account_id)
        .bytes(2, message.as_bytes())
        .into_vec())
}

pub(crate) fn validated_message(text: &str) -> Result<&str> {
    let message = text.trim();
    if message.is_empty() {
        return Err(classic_error("chat message cannot be empty"));
    }
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(classic_error(format!(
            "chat message exceeds {MAX_MESSAGE_BYTES} UTF-8 bytes"
        )));
    }
    Ok(message)
}

/// Builds `legacy_chat.SendCommandRequest`.
///
/// The generated SDK serializer establishes the complete layout: channel id
/// in field 1, command name in field 2, and one string per command argument in
/// repeated field 3. `ILegacyChatArgV1` is only the SDK's C++ input adapter;
/// each adapter's value is copied directly into that repeated string field.
pub fn send_command_request(channel_id: u32, command: &str, arguments: &[&str]) -> Result<Vec<u8>> {
    if channel_id == 0 {
        return Err(classic_error("chat channel ID must be positive"));
    }
    let command = command.trim().strip_prefix('/').unwrap_or(command.trim());
    if command.is_empty() {
        return Err(classic_error("chat command cannot be empty"));
    }
    if command.chars().any(char::is_whitespace) {
        return Err(classic_error("chat command name cannot contain whitespace"));
    }
    let mut request = Message::new()
        .varint(1, u64::from(channel_id))
        .bytes(2, command.as_bytes());
    for argument in arguments {
        if argument.is_empty() {
            return Err(classic_error("chat command arguments cannot be empty"));
        }
        if argument.len() > MAX_COMMAND_ARGUMENT_BYTES {
            return Err(classic_error(format!(
                "chat command argument exceeds {MAX_COMMAND_ARGUMENT_BYTES} UTF-8 bytes"
            )));
        }
        request = request.bytes(3, argument.as_bytes());
    }
    Ok(request.into_vec())
}

/// Decodes `legacy_chat.CommandList`: repeated command names in field 1.
/// Unknown fields and malformed entries are ignored so a newer gateway can
/// extend the response without making chat startup fail.
fn parse_command_list(body: &[u8]) -> Vec<String> {
    let mut commands = Vec::new();
    let mut seen = BTreeSet::new();
    for command in protobuf::fields(body)
        .flatten()
        .filter(|field| field.number == 1)
        .filter_map(|field| field.bytes())
        .filter_map(text)
    {
        let command = command.trim().trim_start_matches('/').to_ascii_lowercase();
        if command.is_empty() || command.chars().any(char::is_whitespace) {
            continue;
        }
        if seen.insert(command.clone()) {
            commands.push(command);
        }
    }
    commands
}

// join and leave both carry only the channel id.
pub fn channel_request(channel_id: u32) -> Result<Vec<u8>> {
    if channel_id == 0 {
        return Err(classic_error("chat channel ID must be positive"));
    }
    Ok(Message::new().varint(1, u64::from(channel_id)).into_vec())
}

/// Builds the shared request used by `JoinCustomChannel`,
/// `JoinCustomChannelByName`, and `CreateAndJoinCustomChannel`.
///
/// The first two RPCs use `JoinCustomChannelRequest`; create uses its own
/// generated type. Both serializers contain the same sole field: name = 1.
pub fn named_channel_request(name: &str) -> Result<Vec<u8>> {
    let name = name.trim();
    if name.is_empty() {
        return Err(classic_error("chat channel name cannot be empty"));
    }
    if name.len() > MAX_CHANNEL_NAME_BYTES {
        return Err(classic_error(format!(
            "chat channel name exceeds {MAX_CHANNEL_NAME_BYTES} UTF-8 bytes"
        )));
    }
    Ok(Message::new().bytes(1, name.as_bytes()).into_vec())
}

/// Decodes the `ChatChannelInfo` carried in field 1 of
/// `JoinedChannelResponse`.
#[must_use]
pub fn parse_joined_channel_response(body: &[u8]) -> Option<ChatChannel> {
    protobuf::first_bytes(body, 1).and_then(parse_channel)
}

/// Decodes `ForceJoinChannelRequest`.
///
/// The SDK parser accepts exactly tag `0x12`: a `ChatChannelInfo` nested in
/// field 2. There is no field-1 channel id or name in this callback.
#[must_use]
pub fn parse_force_join_channel(body: &[u8]) -> Option<ChatChannel> {
    protobuf::first_bytes(body, 2).and_then(parse_channel)
}

fn printable_text(value: &[u8]) -> Option<String> {
    let decoded = std::str::from_utf8(value).ok()?;
    let printable = decoded
        .chars()
        .all(|character| matches!(character, '\r' | '\n' | '\t') || !character.is_control());
    printable.then(|| decoded.to_owned())
}

fn text(value: &[u8]) -> Option<String> {
    printable_text(value).filter(|decoded| !decoded.is_empty())
}

fn parse_attribute(data: &[u8]) -> Option<ChatAttribute> {
    let mut name = String::new();
    let mut value = None;
    for field in protobuf::fields(data).flatten() {
        match field.number {
            1 => name = field.bytes().and_then(text).unwrap_or_default(),
            2 => value = field.bytes().and_then(printable_text),
            _ => {}
        }
    }
    Some(ChatAttribute {
        name: (!name.is_empty()).then_some(name)?,
        value: value?,
    })
}

fn parse_user(data: &[u8]) -> Option<ChatUser> {
    let mut user = ChatUser {
        name: String::new(),
        flags: None,
        is_operator: false,
        avatar: None,
        attributes: Vec::new(),
    };
    for field in protobuf::fields(data).flatten() {
        match field.number {
            1 => user.name = field.bytes().and_then(text).unwrap_or_default(),
            2 => user.flags = field.varint(),
            3 => user
                .attributes
                .extend(field.bytes().and_then(parse_attribute)),
            4 => user.is_operator = field.varint().is_some_and(|value| value != 0),
            _ => {}
        }
    }
    (!user.name.is_empty()).then_some(user)
}

fn parse_channel(data: &[u8]) -> Option<ChatChannel> {
    let mut channel_id = None;
    let mut name = String::new();
    let mut display_name = None;
    let mut is_public = false;
    let mut users = Vec::new();
    for field in protobuf::fields(data).flatten() {
        match field.number {
            1 => channel_id = field.varint().and_then(|id| u32::try_from(id).ok()),
            2 => name = field.bytes().and_then(text).unwrap_or_default(),
            3 => users.extend(field.bytes().and_then(parse_user)),
            4 => is_public = field.varint().is_some_and(|value| value != 0),
            5 => display_name = field.bytes().and_then(text),
            _ => {}
        }
    }
    let channel_id = channel_id?;
    (!name.is_empty()).then_some(ChatChannel {
        channel_id,
        name,
        display_name,
        is_public,
        users,
    })
}

fn printable_strings(data: &[u8], depth: usize) -> Vec<String> {
    let mut values = Vec::new();
    for field in protobuf::fields(data).flatten() {
        let Some(bytes) = field.bytes() else { continue };
        if let Some(value) = text(bytes) {
            values.push(value);
        } else if depth < 2 {
            values.extend(printable_strings(bytes, depth + 1));
        }
    }
    values
}

// field layout varies by event kind, so sender and message are recovered
// positionally: the last displayable string is the message, a preceding one is
// the sender.
fn parse_event(kind: EventKind, body: &[u8]) -> ChatEvent {
    let channel_id = protobuf::first_varint(body, 1).and_then(|id| u32::try_from(id).ok());
    let strings = printable_strings(body, 0);
    ChatEvent {
        kind,
        channel_id,
        sender: (strings.len() >= 2).then(|| strings[0].clone()),
        text: strings.last().cloned(),
        aurora_whisper: None,
    }
}

fn friend_update(method_id: u32, body: &[u8]) -> Option<(ChatFriend, String)> {
    let strings = protobuf::fields(body)
        .flatten()
        .filter_map(|field| field.bytes().and_then(text))
        .collect::<Vec<_>>();
    let name = strings.first()?.clone();
    let reported_game = strings.get(1).cloned();
    let (presence, text) = match method_id {
        method::CHAT_FRIEND_ENTER => (
            FriendPresence::Online,
            format!("Your friend {name} has entered StarCraft."),
        ),
        method::CHAT_FRIEND_EXIT => (
            FriendPresence::Offline,
            format!("Your friend {name} has exited StarCraft."),
        ),
        method::CHAT_FRIEND_NOTIFY_GAME => (
            FriendPresence::InGame,
            reported_game.as_ref().map_or_else(
                || format!("Your friend {name} has entered a game."),
                |game| format!("Your friend {name} has entered a game called {game}."),
            ),
        ),
        _ => return None,
    };
    Some((
        ChatFriend {
            name,
            presence,
            game: (presence == FriendPresence::InGame)
                .then_some(reported_game)
                .flatten(),
            account_id: None,
        },
        text,
    ))
}

fn classic_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::scr::rpc::Header;

    fn user(name: &str, operator: bool) -> Vec<u8> {
        let battle_tag = Message::new()
            .bytes(1, b"battle_tag")
            .bytes(2, b"battletag#1234")
            .into_vec();
        let presence = Message::new()
            .bytes(1, b"game_info")
            .bytes(2, b"Brood War")
            .into_vec();
        Message::new()
            .bytes(1, name.as_bytes())
            .varint(2, 0)
            .bytes(3, &battle_tag)
            .bytes(3, &presence)
            .varint(4, u64::from(operator))
            .into_vec()
    }

    fn channel(id: u32, name: &str, users: &[Vec<u8>]) -> Vec<u8> {
        let mut message = Message::new()
            .varint(1, u64::from(id))
            .bytes(2, name.as_bytes());
        for user in users {
            message = message.bytes(3, user);
        }
        message
            .varint(4, 1)
            .bytes(5, "Public Chat 1".as_bytes())
            .into_vec()
    }

    fn callback(method_id: u32, body: Vec<u8>) -> Frame {
        Frame {
            header: Header {
                service_id: service::LEGACY_CHAT,
                method_id,
                token: 1,
                is_response: Some(false),
                ..Header::default()
            },
            body,
        }
    }

    fn aurora_friend_callback(
        account_id: u64,
        battle_tag: &str,
        current_program: &str,
        in_program: bool,
        role: u64,
    ) -> Frame {
        let friend = Message::new()
            .varint(1, account_id)
            .bytes(2, battle_tag.as_bytes())
            .bytes(3, b"")
            .bytes(4, current_program.as_bytes())
            .varint(5, u64::from(in_program))
            .varint(6, 0)
            .varint(7, 0)
            .bytes(8, b"")
            .bytes(9, b"")
            .into_vec();
        Frame {
            header: Header {
                service_id: service::AURORA_FRIENDS,
                method_id: method::FRIEND_UPDATED,
                token: 1,
                is_response: Some(false),
                ..Header::default()
            },
            body: Message::new().bytes(1, &friend).varint(2, role).into_vec(),
        }
    }

    fn channels_updated(operation: u64, channel: &[u8]) -> Frame {
        let update = Message::new()
            .varint(1, operation)
            .bytes(2, channel)
            .into_vec();
        callback(
            method::CHANNELS_UPDATED,
            Message::new().bytes(1, &update).into_vec(),
        )
    }

    #[test]
    fn applies_a_channel_roster_and_bumps_the_revision() {
        let mut state = ChatState::new();
        let frame = channels_updated(
            0,
            &channel(
                9,
                "Public Chat 1",
                &[user("Zerg", true), user("Toss", false)],
            ),
        );
        assert!(state.apply(&frame));
        assert_eq!(state.roster_revision(), 1);

        let channel = state.channel(9).expect("channel present");
        assert_eq!(channel.label(), "Public Chat 1");
        assert!(channel.is_public);
        assert_eq!(
            channel.users,
            [
                ChatUser {
                    name: "Zerg".into(),
                    flags: Some(0),
                    is_operator: true,
                    avatar: None,
                    attributes: vec![
                        ChatAttribute {
                            name: "battle_tag".into(),
                            value: "battletag#1234".into(),
                        },
                        ChatAttribute {
                            name: "game_info".into(),
                            value: "Brood War".into(),
                        },
                    ],
                },
                ChatUser {
                    name: "Toss".into(),
                    flags: Some(0),
                    is_operator: false,
                    avatar: None,
                    attributes: vec![
                        ChatAttribute {
                            name: "battle_tag".into(),
                            value: "battletag#1234".into(),
                        },
                        ChatAttribute {
                            name: "game_info".into(),
                            value: "Brood War".into(),
                        },
                    ],
                },
            ]
        );
        assert_eq!(
            channel.users[0].attribute("BATTLE_TAG"),
            Some("battletag#1234")
        );
        assert_eq!(channel.users[0].attribute("game_info"), Some("Brood War"));
        assert!(!format!("{channel:?}").contains("battletag"));
        assert_eq!(
            state.find_channel("public chat 1").map(|c| c.channel_id),
            Some(9)
        );
    }

    #[test]
    fn preserves_an_explicitly_empty_attribute_value() {
        let attribute = Message::new()
            .bytes(1, b"lobby_info")
            .bytes(2, b"")
            .into_vec();
        assert_eq!(
            parse_attribute(&attribute),
            Some(ChatAttribute {
                name: "lobby_info".into(),
                value: String::new(),
            })
        );
    }

    #[test]
    fn removes_channels_on_delete_and_on_left_channel() {
        let mut state = ChatState::new();
        state.apply(&channels_updated(0, &channel(9, "Public Chat 1", &[])));
        state.apply(&channels_updated(1, &channel(9, "Public Chat 1", &[])));
        assert!(state.channel(9).is_none());

        state.apply(&channels_updated(0, &channel(9, "Public Chat 1", &[])));
        let left = callback(method::LEFT_CHANNEL, Message::new().varint(1, 9).into_vec());
        assert!(state.apply(&left));
        assert!(state.channel(9).is_none());
    }

    #[test]
    fn decodes_a_talk_message() {
        let mut state = ChatState::new();
        let body = Message::new()
            .varint(1, 9)
            .bytes(2, b"Zerg")
            .bytes(3, "gg wp".as_bytes())
            .into_vec();
        assert!(state.apply(&callback(method::CHAT_TALK_MESSAGE, body)));

        assert_eq!(
            state.take_events(),
            [ChatEvent {
                kind: EventKind::Talk,
                channel_id: Some(9),
                sender: Some("Zerg".into()),
                text: Some("gg wp".into()),
                aurora_whisper: None,
            }]
        );
        assert!(state.take_events().is_empty(), "events drain once");
    }

    #[test]
    fn decodes_a_whisper_for_the_social_adapter() {
        let mut state = ChatState::new();
        let body = Message::new()
            .varint(1, 9)
            .bytes(2, b"Friend#1234")
            .bytes(3, "meet in channel 1".as_bytes())
            .into_vec();
        assert!(state.apply(&callback(method::CHAT_WHISPER_MESSAGE, body)));

        assert_eq!(
            state.take_events(),
            [ChatEvent {
                kind: EventKind::Whisper,
                channel_id: Some(9),
                sender: Some("Friend#1234".into()),
                text: Some("meet in channel 1".into()),
                aurora_whisper: None,
            }]
        );
    }

    #[test]
    fn decodes_account_whisper_receive_and_echo_callbacks() {
        let mut state = ChatState::new();
        assert!(state.apply(&aurora_friend_callback(41, "Observer#1234", "", false, 0)));
        let whisper = |method_id, message: &str| Frame {
            header: Header {
                service_id: service::AURORA_CHAT,
                method_id,
                token: 2,
                is_response: Some(false),
                ..Header::default()
            },
            body: Message::new()
                .fixed32(1, 41)
                .bytes(2, message.as_bytes())
                .into_vec(),
        };

        assert!(state.apply(&whisper(method::WHISPER_RECEIVED, "hello")));
        assert!(state.apply(&whisper(method::WHISPER_ECHO_RECEIVED, "hi back")));
        assert_eq!(
            state.take_events(),
            [
                ChatEvent {
                    kind: EventKind::Whisper,
                    channel_id: None,
                    sender: Some("Observer#1234".into()),
                    text: Some("hello".into()),
                    aurora_whisper: Some(AuroraWhisper {
                        account_id: 41,
                        outgoing: false,
                    }),
                },
                ChatEvent {
                    kind: EventKind::Whisper,
                    channel_id: None,
                    sender: Some("Observer#1234".into()),
                    text: Some("hi back".into()),
                    aurora_whisper: Some(AuroraWhisper {
                        account_id: 41,
                        outgoing: true,
                    }),
                },
            ]
        );
    }

    #[test]
    fn rejects_an_aurora_whisper_with_the_wrong_account_wire_type() {
        let mut state = ChatState::new();
        let frame = Frame {
            header: Header {
                service_id: service::AURORA_CHAT,
                method_id: method::WHISPER_RECEIVED,
                token: 2,
                is_response: Some(false),
                ..Header::default()
            },
            body: Message::new()
                .varint(1, 41)
                .bytes(2, b"not the SDK schema")
                .into_vec(),
        };
        assert!(!state.apply(&frame));
        assert!(state.take_events().is_empty());
    }

    #[test]
    fn decodes_the_channel_information_packet() {
        let mut state = ChatState::new();
        let body = Message::new()
            .varint(1, 9)
            .bytes(2, b"Welcome to StarCraft: Remastered!")
            .into_vec();
        assert!(state.apply(&callback(method::CHAT_INFORMATION_MESSAGE, body)));

        assert_eq!(
            state.take_events(),
            [ChatEvent {
                kind: EventKind::Information,
                channel_id: Some(9),
                sender: None,
                text: Some("Welcome to StarCraft: Remastered!".into()),
                aurora_whisper: None,
            }]
        );
    }

    #[test]
    fn queues_client_owned_information_with_no_channel() {
        let mut state = ChatState::new();
        state.push_information("Connected to chat service.");
        assert_eq!(
            state.take_events(),
            [ChatEvent {
                kind: EventKind::Information,
                channel_id: None,
                sender: None,
                text: Some("Connected to chat service.".into()),
                aurora_whisper: None,
            }]
        );
    }

    #[test]
    fn ignores_responses_and_unrelated_services() {
        let mut state = ChatState::new();
        let mut response = channels_updated(0, &channel(9, "Public Chat 1", &[]));
        response.header.is_response = Some(true);
        assert!(!state.apply(&response));

        let mut other = channels_updated(0, &channel(9, "Public Chat 1", &[]));
        other.header.service_id = service::AUTHENTICATION;
        assert!(!state.apply(&other));
        assert_eq!(state.channels().count(), 0);
    }

    #[test]
    fn builds_the_friend_roster_from_aurora_friend_updates() {
        let mut state = ChatState::new();
        let offline = aurora_friend_callback(41, "Observer#1234", "", false, 0);
        assert!(state.apply(&offline));
        assert_eq!(state.friends_revision(), 1);
        assert_eq!(
            state.friends().cloned().collect::<Vec<_>>(),
            [ChatFriend {
                name: "Observer#1234".into(),
                presence: FriendPresence::Offline,
                game: None,
                account_id: Some(41),
            }]
        );

        // Replayed snapshot records are acknowledged without making the UI
        // rebuild an unchanged list.
        assert!(state.apply(&offline));
        assert_eq!(state.friends_revision(), 1);

        let online = aurora_friend_callback(41, "Observer#1234", "BSAp", true, 0);
        assert!(state.apply(&online));
        assert_eq!(state.friends_revision(), 2);
        assert_eq!(
            state.friends().next().map(|friend| friend.presence),
            Some(FriendPresence::Online)
        );
    }

    #[test]
    fn uses_the_aurora_account_id_to_replace_a_renamed_friend() {
        let mut state = ChatState::new();
        assert!(state.apply(&aurora_friend_callback(41, "OldHandle#1234", "", false, 0)));
        assert!(state.apply(&aurora_friend_callback(
            41,
            "NewHandle#1234",
            "App",
            true,
            0
        )));
        assert_eq!(state.friends().count(), 1);
        assert_eq!(
            state.friends().next().map(|friend| friend.name.as_str()),
            Some("NewHandle#1234")
        );
    }

    #[test]
    fn does_not_guess_unobserved_aurora_friend_roles() {
        let mut state = ChatState::new();
        assert!(!state.apply(&aurora_friend_callback(41, "Pending#1234", "", false, 1)));
        assert_eq!(state.friends().count(), 0);
        assert_eq!(state.friends_revision(), 0);
    }

    #[test]
    fn validates_outgoing_messages() {
        assert_eq!(
            send_message_request(9, "  hello  ").expect("valid"),
            Message::new().varint(1, 9).bytes(2, b"hello").into_vec()
        );
        assert!(send_message_request(9, "   ").is_err());
        assert!(send_message_request(9, &"a".repeat(MAX_MESSAGE_BYTES + 1)).is_err());
        assert!(channel_request(0).is_err());
        assert!(channel_request(9).is_ok());
        assert_eq!(
            send_message_to_all_friends_request("  hello friends  ").expect("valid"),
            Message::new().bytes(2, b"hello friends").into_vec()
        );
        assert_eq!(
            send_account_whisper_request(41, "  hello account  ").expect("valid"),
            Message::new()
                .fixed32(1, 41)
                .bytes(2, b"hello account")
                .into_vec()
        );
    }

    #[test]
    fn encodes_server_commands_and_repeated_string_arguments() {
        let body = send_command_request(9, "/whisper", &["somebody", "hello there"])
            .expect("valid command");
        assert_eq!(protobuf::first_varint(&body, 1), Some(9));
        assert_eq!(protobuf::first_bytes(&body, 2), Some(b"whisper".as_slice()));
        assert_eq!(
            protobuf::fields(&body)
                .flatten()
                .filter(|field| field.number == 3)
                .filter_map(|field| field.bytes())
                .collect::<Vec<_>>(),
            [b"somebody".as_slice(), b"hello there".as_slice()]
        );
        assert!(send_command_request(9, "/stats somebody", &[]).is_err());
        assert!(send_command_request(9, "/whoami", &[""]).is_err());
        assert!(send_command_request(0, "/whoami", &[]).is_err());
    }

    #[test]
    fn encodes_named_channel_requests_as_field_one_strings() {
        let body = named_channel_request("  Op Superiority  ").expect("valid name");
        assert_eq!(
            protobuf::first_bytes(&body, 1),
            Some(b"Op Superiority".as_slice())
        );
        assert!(named_channel_request("  ").is_err());
    }

    #[test]
    fn decodes_the_channel_from_a_join_response() {
        let response = Message::new()
            .bytes(1, &channel(77, "Op Superiority", &[]))
            .into_vec();
        let joined = parse_joined_channel_response(&response).expect("joined channel");
        assert_eq!(joined.channel_id, 77);
        assert_eq!(joined.name, "Op Superiority");
    }

    #[test]
    fn decodes_and_applies_a_forced_channel_join_from_field_two() {
        let body = Message::new()
            .bytes(2, &channel(77, "Op Superiority", &[]))
            .into_vec();
        let joined = parse_force_join_channel(&body).expect("forced channel");
        assert_eq!(joined.channel_id, 77);

        let mut state = ChatState::new();
        assert!(state.apply(&callback(method::FORCE_JOIN_CHANNEL, body)));
        assert_eq!(state.roster_revision(), 1);
        assert_eq!(
            state.channel(77).map(|channel| channel.name.as_str()),
            Some("Op Superiority")
        );
    }

    #[test]
    fn decodes_the_server_command_catalogue_in_wire_order() {
        let whitelist = Message::new()
            .bytes(1, b"whisper")
            .bytes(1, b"m")
            .bytes(1, b"HELP")
            .bytes(1, b"help")
            .bytes(1, b"not a command")
            .bytes(2, b"ignored")
            .into_vec();
        let blacklist = Message::new().bytes(1, b"disconnectclient").into_vec();
        let catalog = CommandCatalog::from_responses(&whitelist, &blacklist);

        assert_eq!(catalog.whitelist(), ["whisper", "m", "help"]);
        assert_eq!(catalog.blacklist(), ["disconnectclient"]);
        assert!(catalog.allows("/HELP"));
        assert!(!catalog.allows("/disconnectclient"));
        assert_eq!(catalog.help_text(), "Available commands: /whisper /m /help");
    }

    #[test]
    fn replaces_command_catalogue_halves_from_push_updates() {
        let mut catalog = CommandCatalog::from_responses(
            &Message::new().bytes(1, b"help").into_vec(),
            &Message::new().bytes(1, b"forbidden").into_vec(),
        );
        let whitelist = Message::new()
            .bytes(1, b"help")
            .bytes(1, b"whois")
            .into_vec();
        let blacklist = Message::new().bytes(1, b"blocked").into_vec();

        assert!(catalog.replace_whitelist(&whitelist));
        assert!(catalog.replace_blacklist(&blacklist));
        assert!(!catalog.replace_whitelist(&whitelist));
        assert_eq!(catalog.whitelist(), ["help", "whois"]);
        assert_eq!(catalog.blacklist(), ["blocked"]);
    }

    #[test]
    fn renders_classic_friend_callbacks_as_information_lines() {
        let mut state = ChatState::new();
        let entered = callback(
            method::CHAT_FRIEND_ENTER,
            Message::new()
                .bytes(1, b"Darko2")
                .bytes(2, b"unused")
                .into_vec(),
        );
        let exited = callback(
            method::CHAT_FRIEND_EXIT,
            Message::new().bytes(1, b"Darko2").into_vec(),
        );
        let game = callback(
            method::CHAT_FRIEND_NOTIFY_GAME,
            Message::new()
                .bytes(1, b"Darko2")
                .bytes(2, b"The Hunters")
                .bytes(3, b"unused")
                .into_vec(),
        );
        assert!(state.apply(&entered));
        assert_eq!(state.friends_revision(), 1);
        assert!(state.apply(&exited));
        assert_eq!(state.friends_revision(), 2);
        assert!(state.apply(&game));
        assert_eq!(state.friends_revision(), 3);
        assert_eq!(
            state.friends().cloned().collect::<Vec<_>>(),
            [ChatFriend {
                name: "Darko2".into(),
                presence: FriendPresence::InGame,
                game: Some("The Hunters".into()),
                account_id: None,
            }]
        );
        assert_eq!(
            state
                .take_events()
                .into_iter()
                .filter_map(|event| event.text)
                .collect::<Vec<_>>(),
            [
                "Your friend Darko2 has entered StarCraft.",
                "Your friend Darko2 has exited StarCraft.",
                "Your friend Darko2 has entered a game called The Hunters."
            ]
        );
    }
}
