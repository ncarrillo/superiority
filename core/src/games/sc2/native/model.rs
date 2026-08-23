use std::fmt;

use serde::Serialize;

use crate::bsn::value::{BsnStruct, BsnValue};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Payload {
    Reflected(BsnValue),
    CommandResponse(CommandResponse),
    MessageFrame(ConnectionMessageFrame),
    ChatInvite(ChatInvite),
    ClubInviteAction(ClubInviteAction),
    ClubSearch(Vec<u32>),
    ClubInfo(Vec<ClubSummary>),
    ClubSummaries(Vec<ClubSummary>),
    CacheStreamItems(CacheStreamItems),
    ConferenceDescriptions(ConferenceDescriptions),
    ConferenceMemberCounts(ConferenceMemberCounts),
    ChannelList(ChannelList),
    ChatJoin(ChatJoin),
    ChatMembership(ChatMembership),
    ChatMessage(ChatMessage),
    ChatWhisper(ChatWhisper),
    ChatWhisperEcho(ChatWhisper),
    ToonList(ToonList),
    ToonSelected(ToonSelected),
    PresenceFields(PresenceFields),
    PresenceUpdate(PresenceUpdate),
    ToonNameResolved(ToonNameResolved),
    MemberClanTag(MemberClanTag),
    ProfileRead(ProfileReadResponse),
    ProfileAddressQuery(ProfileAddressQueryResponse),
    Friends(FriendsPage),
    FriendToons(FriendToonPage),
    AccountBlocks(AccountBlockPage),
    StartupSummary(StartupSummary),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CommandResponse {
    pub command_id: u8,
    pub result: u16,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FriendIdentity {
    Account(u32),
    Character {
        program_id: u32,
        region: u32,
        realm: u32,
        id: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SocialOperation {
    Add,
    Remove,
    Modify,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FriendEntry {
    pub identity: FriendIdentity,
    pub display_name: Option<String>,
    pub full_name: Option<String>,
    pub note: Option<String>,
    pub profile: Option<ProfileAddress>,
    pub toon_name: Option<ToonFullName>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FriendUpdate {
    pub operation: SocialOperation,
    pub entry: FriendEntry,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FriendsPage {
    pub updates: Vec<FriendUpdate>,
    pub complete: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FriendToon {
    pub account_id: u32,
    pub program_id: u32,
    pub profile: Option<ProfileAddress>,
    pub toon_name: ToonFullName,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FriendToonPage {
    pub entries: Vec<FriendToon>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileAddressQueryResponse {
    pub request_id: u32,
    pub address: Option<ProfileAddress>,
    pub error: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AccountBlockEntry {
    pub account_id: u32,
    pub nickname: Option<String>,
    pub full_name: Option<String>,
    pub role: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AccountBlockPage {
    pub entries: Vec<AccountBlockEntry>,
    pub complete: Option<bool>,
}

impl Payload {
    #[must_use]
    pub const fn reflected(&self) -> Option<&BsnValue> {
        if let Self::Reflected(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClubInviteAction {
    pub club_id: u32,
    pub program: u32,
    pub member_kind: u8,
    pub member_realm: u32,
    pub member_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ChatInvite {
    pub channel_type: u8,
    pub channel_index: u8,
    pub inviter_presence: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClubSummary {
    pub club_id: u32,
    pub name: Option<String>,
    pub kind: u8,
    pub category: u8,
    pub private: bool,
    /// everyone on the roster, from `ClubSummaryInfo::m_memberCount`.
    pub member_count: Option<u32>,
    /// members online right now, from `ClubOnlineStatus::m_online`. only the
    /// club-info response carries this; a plain summary does not.
    pub online: Option<u32>,
}

#[derive(Clone, PartialEq, Serialize)]
pub struct ConnectionMessageFrame {
    pub frame_type: i128,
    pub headers: Vec<BsnStruct>,
    pub payload: Vec<u8>,
}

impl fmt::Debug for ConnectionMessageFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionMessageFrame")
            .field("frame_type", &self.frame_type)
            .field("header_count", &self.headers.len())
            .field("payload_bytes", &self.payload.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub struct CacheStreamItem {
    pub publication_time: i32,
    #[serde(serialize_with = "serialize_content_handle")]
    pub content_handle: [u8; 40],
}

impl fmt::Debug for CacheStreamItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CacheStreamItem")
            .field("publication_time", &self.publication_time)
            .field("content_handle", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CacheStreamItems {
    pub items: Vec<CacheStreamItem>,
    pub offset: u16,
    pub total_items: u16,
    pub token: u32,
}

/// `Battlenet::Conference::FullConferenceDescription`, reduced to the fields
/// that name a conference.
///
/// a public chat channel is served by one or more conferences; this is what
/// ties a conference back to the channel it serves. only the public arm of
/// `Conference::LocatorKey` is decoded — a private or club conference is not a
/// listed channel and is skipped.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ConferenceDescription {
    pub conference_id: u32,
    /// `Battlenet::Locale::Id`, a FourCC such as `enUS`.
    pub locale: u32,
    /// the catalogue id the public-channel name table is keyed by.
    pub channel_name_id: u16,
    /// which room of that channel this is — `General`, `General 2`, and so on,
    /// which is what the `%d` in the catalogue's name template is for. numbered
    /// from one and contiguous per channel.
    pub shard: u16,
    pub sort_order: u16,
    /// the room's capacity, from `ConferenceConfiguration`. observed at 200.
    pub max_members: u16,
    /// how full the server lets a room get before it reports `m_isFull`, as
    /// raw `f32` bits so the record stays comparable. observed at 0.9, which is
    /// why a 200-seat room fills at 181.
    pub target_proportion_bits: u32,
}

impl ConferenceDescription {
    #[must_use]
    pub fn target_proportion(&self) -> f32 {
        f32::from_bits(self.target_proportion_bits)
    }

    /// the population at which the server starts reporting the room full.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the proportion is bounded to 0..=1 just above, so the product \
                  cannot leave the range of max_members"
    )]
    pub fn full_at(&self) -> u16 {
        let proportion = self.target_proportion();
        if !proportion.is_finite() || !(0.0..=1.0).contains(&proportion) {
            return self.max_members;
        }
        (f32::from(self.max_members) * proportion).round() as u16
    }

    #[must_use]
    pub fn locale_tag(&self) -> String {
        String::from_utf8_lossy(&self.locale.to_be_bytes()).into_owned()
    }
}

/// one page of `Battlenet::Client::Chat::ConferenceDescriptions`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConferenceDescriptions {
    pub entries: Vec<ConferenceDescription>,
    pub is_last: bool,
}

/// `Battlenet::Conference::MembershipInfo` — how many people are in one
/// conference right now. a public chat channel is served by one or more
/// conferences; `full` is the server's own verdict, which it sets at the cap
/// rather than leaving the client to compare against a maximum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ConferenceMemberCount {
    pub conference_id: u32,
    pub members: u16,
    pub full: bool,
}

/// one page of `Battlenet::Client::Chat::ConferenceMemberCounts`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConferenceMemberCounts {
    pub entries: Vec<ConferenceMemberCount>,
    pub is_last: bool,
    /// the server's stamp for the sample, when it sends one.
    pub sampled_at: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ChannelDescriptor {
    pub kind: u8,
    pub index: u16,
    pub identifier: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChannelList {
    pub entries: Vec<ChannelDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChatJoin {
    pub success: bool,
    pub channel_index: Option<u8>,
    pub member_handle: Option<u32>,
    pub channel_type: Option<u8>,
    pub channel_name_id: Option<u16>,
    pub channel_shard_index: Option<u16>,
    pub reason: Option<u16>,
    pub token: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum MembershipKind {
    Join,
    Leave,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum PartyMemberStatus {
    Invalid,
    Online,
    Offline,
    Invited,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MembershipChange {
    pub kind: MembershipKind,
    pub member_handle: u32,
    pub presence_id: Option<u32>,
    pub display_name: Option<String>,
    pub toon_name: Option<ToonFullName>,
    pub party_status: Option<PartyMemberStatus>,
    pub reason: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChatMembership {
    pub channel_index: u8,
    pub end_of_initial: bool,
    pub changes: Vec<MembershipChange>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChatMessage {
    pub channel_index: u8,
    pub member_handle: u32,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChatWhisper {
    pub peer: ToonFullName,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum WhisperTarget {
    Presence(u32),
    Account(u32),
    WarcraftAccount(u64),
    ToonHandle(ToonHandle),
    ToonName(ToonFullName),
    Name(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToonDisplay {
    pub name: String,
    pub realm: u32,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ToonFullName {
    pub region: u8,
    pub program_id: u32,
    pub realm: u32,
    pub name: String,
}

/// which game a lobby is, as `Battlenet::S2Master::AdvertHandle`: the game
/// server that owns it, when that server came up, and the advert's id on it.
/// only the last of the three survives a `lobbyLink(...)` in chat, so the other
/// two have to come from somewhere the whole handle is intact — presence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AdvertHandle {
    pub server_label: u32,
    pub server_epoch: i32,
    pub advert_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ToonHandle {
    pub region: u8,
    pub program_id: u32,
    pub realm: u32,
    pub id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToonNameResolved {
    pub name: ToonFullName,
    pub result: u16,
    pub handle: Option<ToonHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemberClanTag {
    pub token: u32,
    pub result: u16,
    pub club_id: Option<u32>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToonList {
    pub displays: Vec<ToonDisplay>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToonSelected {
    pub name: String,
    pub realm: u32,
    pub handle: ToonHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PresenceField {
    pub handle: u32,
    pub identifier: u8,
    pub flags: PresenceFieldFlags,
    pub fixed_size: Option<u16>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PresenceFieldFlags(u8);

impl PresenceFieldFlags {
    pub(crate) const WRITABLE: u8 = 1 << 0;
    pub(crate) const EPHEMERAL: u8 = 1 << 1;
    pub(crate) const SERVER_ONLY: u8 = 1 << 2;
    pub(crate) const CLIENT_ONLY: u8 = 1 << 3;

    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x0f)
    }

    #[must_use]
    pub const fn writable(self) -> bool {
        self.0 & Self::WRITABLE != 0
    }

    #[must_use]
    pub const fn ephemeral(self) -> bool {
        self.0 & Self::EPHEMERAL != 0
    }

    #[must_use]
    pub const fn server_only(self) -> bool {
        self.0 & Self::SERVER_ONLY != 0
    }

    #[must_use]
    pub const fn client_only(self) -> bool {
        self.0 & Self::CLIENT_ONLY != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenceFields {
    pub entries: Vec<PresenceField>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenceUpdate {
    pub local_presence_id: u32,
    pub master_presence_id: u32,
    pub field_data: Vec<u8>,
    pub cleared_handles: Vec<u32>,
    pub handles: Vec<u32>,
    pub variable_sizes: Vec<u16>,
    pub online: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProfileAddress {
    pub label: u32,
    pub id: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ImageTableEntry {
    pub table_id: u16,
    pub offset: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ProfileReadResult {
    Start {
        packet_count: u32,
        record_type: i128,
    },
    Block(Vec<u8>),
    Failure(u16),
    Cache,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileReadResponse {
    pub request_id: u32,
    pub result: ProfileReadResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StartupSummary {
    pub kind: &'static str,
    pub item_count: usize,
    pub complete: Option<bool>,
}

fn serialize_content_handle<S>(value: &[u8; 40], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(value)
}
