use std::{collections::HashMap, path::Path};

use crate::{
    Error, Result,
    bgs::{NativeHandoff, fourcc},
    bsn::{
        bits::{BitReader, BitWriter, RoutingHeader},
        codec::{Codec, DecodedField},
        value::{BsnField, BsnStruct, BsnValue},
    },
    metadata::{Metadata, Schema, TypeKind, read_largest_metadata, read_metadata},
    native::{decode, model::Payload, schema, wire_layout},
};

#[derive(Clone, Debug)]
pub(crate) struct DecodedIncoming {
    pub type_id: u32,
    pub payload: Payload,
    pub provenance: Vec<DecodedField>,
}

pub const AUTHENTICATION_SLOT: u8 = 0;
pub const CONNECTION_SLOT: u8 = 1;
pub const ACHIEVEMENT_SLOT: u8 = 8;
pub const AUTH_LOGON_COMMAND: u8 = 0;
pub const AUTH_RESUME_COMMAND: u8 = 1;
pub const AUTH_PROOF_COMMAND: u8 = 2;
pub const AUTH_CONFIGURATION_COMMAND: u8 = 18;
pub const AUTH_GENERATE_WEB_TOKEN_COMMAND: u8 = 16;
pub const AUTH_SINGLE_SIGN_ON_COMMAND: u8 = 17;
pub const CONNECTION_BOOM_COMMAND: u8 = 1;
pub const CONNECTION_SERVER_VERSION_COMMAND: u8 = 3;
pub const CONNECTION_ENABLE_ENCRYPTION_COMMAND: u8 = 5;
pub const CONNECTION_LOGOUT_COMMAND: u8 = 6;
pub const CONNECTION_PING_COMMAND: u8 = 10;
pub const CONNECTION_REGULATOR_COMMAND: u8 = 11;
pub const CONNECTION_PONG_COMMAND: u8 = 12;
pub const CONNECTION_MESSAGE_FRAME_COMMAND: u8 = 13;
pub const CONNECTION_GAME_SITE_INFO_COMMAND: u8 = 14;
pub const FRIENDS_SLOT: u8 = 3;
pub const PRESENCE_SLOT: u8 = 4;
pub const CHAT_SLOT: u8 = 5;
pub const S2_MASTER_SLOT: u8 = 10;
pub const CACHE_SLOT: u8 = 11;
pub const PARTY_SLOT: u8 = 12;
pub const S2_MULTIPLAYER_SLOT: u8 = 13;
pub const PROFILE_SLOT: u8 = 14;
pub const TOON_SLOT: u8 = 15;
pub const CHANNEL_INDEX_COUNT: usize = 7;
pub const MAX_JOINED_CHANNELS: usize = 6;
pub const CHAT_JOIN_REQUEST_COMMAND: u8 = 0;
pub const CHAT_LEAVE_REQUEST_COMMAND: u8 = 2;
pub const CHAT_INVITE_NOTIFY_COMMAND: u8 = 4;
pub const CHAT_INVITE_ACCEPT_COMMAND: u8 = 5;
pub const CHAT_INVITE_DECLINE_COMMAND: u8 = 6;
pub const CHAT_STATUS_CHANGE_COMMAND: u8 = 9;
pub const CHAT_CREATE_AND_INVITE_COMMAND: u8 = 10;
pub const CHAT_MESSAGE_COMMAND: u8 = 11;
pub const CHAT_DATAGRAM_CONNECTION_UPDATE_COMMAND: u8 = 13;
pub const CHAT_WHISPER_SEND_COMMAND: u8 = 19;
pub const CHAT_WHISPER_RECV_COMMAND: u8 = 19;
pub const CHAT_WHISPER_UNDELIVERABLE_COMMAND: u8 = 20;
pub const CHAT_WHISPER_ECHO_COMMAND: u8 = 30;
pub const CHAT_MODIFY_CHANNEL_LIST_COMMAND: u8 = 32;
pub const CHAT_MODIFY_CHANNEL_LIST_RESPONSE_COMMAND: u8 = 33;
pub const CHAT_CHANNEL_LIST_REQUEST_COMMAND: u8 = 21;
pub const CHAT_ENUM_CONFERENCES_COMMAND: u8 = 25;
pub const CHAT_ENUM_CATEGORIES_COMMAND: u8 = 23;
pub const CHAT_CATEGORY_DESCRIPTIONS_COMMAND: u8 = 24;
pub const CACHE_GET_STREAM_ITEMS_COMMAND: u8 = 9;
pub const TOON_SELECT_COMMAND: u8 = 5;
pub const FRIENDS_LIST_COMMAND: u8 = 30;
pub const FRIENDS_TOONS_COMMAND: u8 = 6;
pub const FRIENDS_ACCOUNT_BLOCK_COMMAND: u8 = 31;
pub const FRIENDS_TOON_BLOCK_COMMAND: u8 = 33;
pub const PRESENCE_UPDATE_COMMAND: u8 = 0;
pub const PRESENCE_FIELDS_COMMAND: u8 = 1;
pub const PRESENCE_STATISTICS_SUBSCRIBE_COMMAND: u8 = 2;
pub const PRESENCE_STATISTICS_UPDATE_COMMAND: u8 = 3;
pub const PRESENCE_TEMPORARY_COMMAND: u8 = 4;
pub const CHAT_MEMBERSHIP_COMMAND: u8 = 1;
pub const CHAT_CHANNEL_LIST_RESPONSE_COMMAND: u8 = 22;
pub const CHAT_CONFERENCES_RESPONSE_COMMAND: u8 = 26;
pub const CHAT_JOIN_NOTIFY_COMMAND: u8 = 27;
pub const PARTY_NON_LOBBY_ATTRIBUTE_CHANGE_COMMAND: u8 = 0;
pub const PARTY_BEGIN_READY_PROCESS_COMMAND: u8 = 12;
pub const PARTY_READY_PROCESS_UPDATE_COMMAND: u8 = 14;
pub const PARTY_MODIFY_NON_LOBBY_ATTRIBUTE_LIST_COMMAND: u8 = 17;
pub const PARTY_MODIFY_MAP_OPTIONS_COMMAND: u8 = 19;
pub const PARTY_MAP_OPTIONS_CHANGE_COMMAND: u8 = 20;
pub const S2_MASTER_MMQ_SUBSCRIBE_COMMAND: u8 = 11;
pub const S2_MASTER_MMQ_ANNOUNCE_COMMAND: u8 = 36;
pub const S2_MASTER_CURRENT_SEASON_COMMAND: u8 = 27;
pub const S2_MASTER_MMQ_GET_INFO_COMMAND: u8 = 17;
pub const S2_MASTER_MMQ_GET_LIST_COMMAND: u8 = 28;
pub const S2_MASTER_SITE_LATENCY_INFO_COMMAND: u8 = 35;
pub const S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND: u8 = 57;
pub const S2_MULTIPLAYER_CLUB_SUBSCRIBE_COMMAND: u8 = 47;
pub const S2_MULTIPLAYER_CLUB_CHANGE_NOTIFICATION_COMMAND: u8 = 49;
pub const S2_MAP_GAME_GROUP_SUBSCRIBE_COMMAND: u8 = 11;
pub const S2_MAP_GAME_GROUP_UPDATE_COMMAND: u8 = 12;
pub const S2_MAP_LIST_FAVORITES_COMMAND: u8 = 13;
pub const ACHIEVEMENT_LISTEN_COMMAND: u8 = 0;
pub const ACHIEVEMENT_DATA_COMMAND: u8 = 2;
pub const S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND: u8 = 53;
pub const S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND: u8 = 46;
pub const S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND: u8 = 51;
pub const S2_MULTIPLAYER_INVITE_ACTION_COMMAND: u8 = 54;
pub const S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND: u8 = 55;
pub const INVITE_RECORD_BYTES: usize = 26;
pub const PROFILE_SETTINGS_AVAILABLE_COMMAND: u8 = 4;
pub const PROFILE_CHANGE_SETTINGS_COMMAND: u8 = 5;
pub const PROFILE_READ_COMMAND: u8 = 0;
pub const PROFILE_ADDRESS_QUERY_COMMAND: u8 = 1;
pub const PROFILE_RESOLVE_TOON_HANDLE_REQUEST_COMMAND: u8 = 2;
pub const PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND: u8 = 3;
pub const PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND: u8 = 3;
pub const PROFILE_SEND_STATS_UI_EVENTS_COMMAND: u8 = 8;
pub const TOON_LIST_COMMAND: u8 = 0;
pub const TOON_CREATE_INIT_COMMAND: u8 = 1;
pub const TOON_CREATE_SETUP_COMMAND: u8 = 2;
pub const TOON_CREATE_FINAL_COMMAND: u8 = 3;
pub const TOON_CREATE_CANCEL_COMMAND: u8 = 4;
pub const TOON_SELECTED_COMMAND: u8 = 6;
pub const TOON_CREATED_COMMAND: u8 = 7;
pub const TOON_FAILURE_COMMAND: u8 = 8;
pub const TOON_WELCOME_COMMAND: u8 = 10;
pub const TOON_BILLING_UPDATE_COMMAND: u8 = 13;
pub const TOON_INITIAL_NOTIFIES_COMPLETE_COMMAND: u8 = 14;
pub const TOON_CAIS_TIME_UPDATE_COMMAND: u8 = 23;

pub const SC2_NATIVE_VERSION: u64 = 0x000a_16a7;
pub const SC2_MACOS_NATIVE_VERSIONS: [(&str, &str, u64); 5] = [
    ("S2", "NGD1", 0x5bc8_dcc1),
    ("S2", "NGD2", 0xfade_3a32),
    ("S2", "NGD3", 0x0c12_9365),
    ("S2", "NGD4", 0x86b7_c0ed),
    ("Bnet", "Mc64", SC2_NATIVE_VERSION),
];

const INCOMING_TYPES: &[((u8, u8), &str)] = &[
    (
        (AUTHENTICATION_SLOT, AUTH_LOGON_COMMAND),
        "Battlenet::Client::Authentication::LogonResponse3",
    ),
    (
        (AUTHENTICATION_SLOT, AUTH_CONFIGURATION_COMMAND),
        "Battlenet::Client::Authentication::Configuration",
    ),
    (
        (AUTHENTICATION_SLOT, AUTH_PROOF_COMMAND),
        "Battlenet::Client::Authentication::ProofRequest",
    ),
    (
        (AUTHENTICATION_SLOT, AUTH_RESUME_COMMAND),
        "Battlenet::Client::Authentication::ResumeResponse",
    ),
    (
        (AUTHENTICATION_SLOT, AUTH_GENERATE_WEB_TOKEN_COMMAND),
        "Battlenet::Client::Authentication::GenerateWebTokenResponse",
    ),
    (
        (CONNECTION_SLOT, CONNECTION_BOOM_COMMAND),
        "Battlenet::Client::Connection::Boom",
    ),
    (
        (CONNECTION_SLOT, CONNECTION_SERVER_VERSION_COMMAND),
        "Battlenet::Client::Connection::ServerVersion",
    ),
    (
        (CONNECTION_SLOT, CONNECTION_REGULATOR_COMMAND),
        "Battlenet::Client::Connection::RegulatorUpdate",
    ),
    (
        (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND),
        "Battlenet::Client::Connection::MessageFrame",
    ),
    (
        (CONNECTION_SLOT, CONNECTION_GAME_SITE_INFO_COMMAND),
        "Battlenet::Client::Connection::GameSiteInfo",
    ),
    (
        (ACHIEVEMENT_SLOT, ACHIEVEMENT_DATA_COMMAND),
        "Battlenet::Client::Achievement::Data",
    ),
    (
        (FRIENDS_SLOT, FRIENDS_TOONS_COMMAND),
        "Battlenet::Client::Friends::ToonsOfFriendsNotify",
    ),
    (
        (FRIENDS_SLOT, FRIENDS_LIST_COMMAND),
        "Battlenet::Client::Friends::FriendsListNotify5",
    ),
    (
        (FRIENDS_SLOT, FRIENDS_ACCOUNT_BLOCK_COMMAND),
        "Battlenet::Client::Friends::AccountBlockAddedNotify",
    ),
    (
        (FRIENDS_SLOT, FRIENDS_TOON_BLOCK_COMMAND),
        "Battlenet::Client::Friends::ToonBlockNotify",
    ),
    (
        (PRESENCE_SLOT, PRESENCE_UPDATE_COMMAND),
        "Battlenet::Client::Presence::UpdateNotify",
    ),
    (
        (PRESENCE_SLOT, PRESENCE_FIELDS_COMMAND),
        "Battlenet::Client::Presence::FieldSpecAnnounce",
    ),
    (
        (PRESENCE_SLOT, PRESENCE_STATISTICS_UPDATE_COMMAND),
        "Battlenet::Client::Presence::StatisticsUpdate",
    ),
    (
        (PRESENCE_SLOT, PRESENCE_TEMPORARY_COMMAND),
        "Battlenet::Client::Presence::TemporaryPresenceResponse",
    ),
    (
        (CHAT_SLOT, CHAT_MEMBERSHIP_COMMAND),
        "Battlenet::Client::Chat::MembershipChangeNotify",
    ),
    (
        (CHAT_SLOT, CHAT_MESSAGE_COMMAND),
        "Battlenet::Client::Chat::MessageRecv",
    ),
    (
        (CHAT_SLOT, CHAT_DATAGRAM_CONNECTION_UPDATE_COMMAND),
        "Battlenet::Client::Chat::DatagramConnectionUpdate",
    ),
    (
        (CHAT_SLOT, CHAT_WHISPER_RECV_COMMAND),
        "Battlenet::Client::Chat::WhisperRecv",
    ),
    (
        (CHAT_SLOT, CHAT_WHISPER_ECHO_COMMAND),
        "Battlenet::Client::Chat::WhisperEchoRecv",
    ),
    (
        (CHAT_SLOT, CHAT_CHANNEL_LIST_RESPONSE_COMMAND),
        "Battlenet::Client::Chat::ChannelListResponse",
    ),
    (
        (CHAT_SLOT, CHAT_CATEGORY_DESCRIPTIONS_COMMAND),
        "Battlenet::Client::Chat::CategoryDescriptions",
    ),
    (
        (CHAT_SLOT, CHAT_CONFERENCES_RESPONSE_COMMAND),
        "Battlenet::Client::Chat::ConferenceDescriptions",
    ),
    (
        (CHAT_SLOT, CHAT_JOIN_NOTIFY_COMMAND),
        "Battlenet::Client::Chat::JoinNotify2",
    ),
    (
        (CHAT_SLOT, CHAT_MODIFY_CHANNEL_LIST_RESPONSE_COMMAND),
        "Battlenet::Client::Chat::ModifyChannelListResponse2",
    ),
    (
        (CHAT_SLOT, CHAT_INVITE_NOTIFY_COMMAND),
        "Battlenet::Client::Chat::InviteNotify",
    ),
    (
        (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND),
        "Battlenet::Client::Cache::GetStreamItemsResponse",
    ),
    (
        (PARTY_SLOT, PARTY_BEGIN_READY_PROCESS_COMMAND),
        "Battlenet::Client::Party::BeginReadyProcess",
    ),
    (
        (PARTY_SLOT, PARTY_READY_PROCESS_UPDATE_COMMAND),
        "Battlenet::Client::Party::ReadyProcessUpdate",
    ),
    (
        (PARTY_SLOT, PARTY_MAP_OPTIONS_CHANGE_COMMAND),
        "Battlenet::Client::Party::MapOptionsChange",
    ),
    (
        (S2_MASTER_SLOT, S2_MASTER_CURRENT_SEASON_COMMAND),
        "Battlenet::Client::S2Master::CurrentSeasonResponse",
    ),
    (
        (S2_MASTER_SLOT, S2_MASTER_MMQ_GET_LIST_COMMAND),
        "Battlenet::Client::S2Master::MMQGetListResponse",
    ),
    (
        (S2_MASTER_SLOT, S2_MASTER_MMQ_ANNOUNCE_COMMAND),
        "Battlenet::Client::S2Master::MMQAnnounce",
    ),
    (
        (S2_MULTIPLAYER_SLOT, S2_MAP_LIST_FAVORITES_COMMAND),
        "Battlenet::Client::S2Map::S2ListMapFavoritesResponse",
    ),
    (
        (
            S2_MULTIPLAYER_SLOT,
            S2_MULTIPLAYER_CLUB_CHANGE_NOTIFICATION_COMMAND,
        ),
        "Battlenet::Client::Club::ClubChangeNotification",
    ),
    (
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND),
        "Battlenet::Client::Club::ClubSettings",
    ),
    (
        (
            S2_MULTIPLAYER_SLOT,
            S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND,
        ),
        "Battlenet::Client::Club::GetMemberClanTagsResponse",
    ),
    (
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND),
        "Battlenet::Client::Club::GetToonClubsResponse",
    ),
    (
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND),
        "Battlenet::Client::Club::SearchClubsResponse",
    ),
    (
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_INVITE_ACTION_COMMAND),
        "Battlenet::Client::Club::InviteAction",
    ),
    (
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND),
        "Battlenet::Client::Club::GetClubInfoResponse",
    ),
    (
        (PROFILE_SLOT, PROFILE_READ_COMMAND),
        "Battlenet::Client::Profile::ReadResponse",
    ),
    (
        (PROFILE_SLOT, PROFILE_ADDRESS_QUERY_COMMAND),
        "Battlenet::Client::Profile::AddressQueryResponse",
    ),
    (
        (PROFILE_SLOT, PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND),
        "Battlenet::Client::Profile::ResolveToonNameToHandleResponse",
    ),
    (
        (PROFILE_SLOT, PROFILE_SETTINGS_AVAILABLE_COMMAND),
        "Battlenet::Client::Profile::SettingsAvailable",
    ),
    (
        (TOON_SLOT, TOON_LIST_COMMAND),
        "Battlenet::Client::Toon::ToonList",
    ),
    (
        (TOON_SLOT, TOON_CREATE_SETUP_COMMAND),
        "Battlenet::Client::Toon::ToonCreateSetup",
    ),
    (
        (TOON_SLOT, TOON_CREATED_COMMAND),
        "Battlenet::Client::Toon::ToonCreated",
    ),
    (
        (TOON_SLOT, TOON_SELECTED_COMMAND),
        "Battlenet::Client::Toon::ToonSelected",
    ),
    (
        (TOON_SLOT, TOON_FAILURE_COMMAND),
        "Battlenet::Client::Toon::Failure",
    ),
    (
        (TOON_SLOT, TOON_WELCOME_COMMAND),
        "Battlenet::Client::Toon::Welcome",
    ),
    (
        (TOON_SLOT, TOON_BILLING_UPDATE_COMMAND),
        "Battlenet::Client::Toon::BillingUpdateNotify",
    ),
    (
        (TOON_SLOT, TOON_INITIAL_NOTIFIES_COMPLETE_COMMAND),
        "Battlenet::Client::Toon::InitialNotifiesComplete",
    ),
];

const fn has_custom_incoming_decoder(route: (u8, u8)) -> bool {
    matches!(
        route,
        (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND)
            | (
                CHAT_SLOT,
                CHAT_CONFERENCES_RESPONSE_COMMAND
                    | CHAT_CHANNEL_LIST_RESPONSE_COMMAND
                    | CHAT_JOIN_NOTIFY_COMMAND
                    | CHAT_INVITE_NOTIFY_COMMAND
                    | CHAT_MEMBERSHIP_COMMAND
                    | CHAT_MESSAGE_COMMAND
                    | CHAT_WHISPER_RECV_COMMAND
                    | CHAT_WHISPER_ECHO_COMMAND
            )
            | (
                PRESENCE_SLOT,
                PRESENCE_FIELDS_COMMAND | PRESENCE_UPDATE_COMMAND
            )
            | (S2_MASTER_SLOT, S2_MASTER_CURRENT_SEASON_COMMAND)
            | (
                S2_MULTIPLAYER_SLOT,
                S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND
                    | S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND
                    | S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND
                    | S2_MULTIPLAYER_INVITE_ACTION_COMMAND
                    | S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND
                    | S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND
            )
            | (
                CONNECTION_SLOT,
                CONNECTION_MESSAGE_FRAME_COMMAND | CONNECTION_GAME_SITE_INFO_COMMAND
            )
            | (
                FRIENDS_SLOT,
                FRIENDS_LIST_COMMAND
                    | FRIENDS_TOONS_COMMAND
                    | FRIENDS_ACCOUNT_BLOCK_COMMAND
                    | FRIENDS_TOON_BLOCK_COMMAND
            )
            | (
                PROFILE_SLOT,
                PROFILE_SETTINGS_AVAILABLE_COMMAND
                    | PROFILE_READ_COMMAND
                    | PROFILE_ADDRESS_QUERY_COMMAND
                    | PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND
            )
            | (
                TOON_SLOT,
                TOON_LIST_COMMAND | TOON_SELECTED_COMMAND | TOON_WELCOME_COMMAND
            )
    )
}

#[derive(Clone, Debug)]
pub struct Record {
    pub header: RoutingHeader,
    pub type_id: u32,
    pub value: Payload,
    pub payload_bit_count: usize,
    pub byte_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogonParameters {
    pub ping_timeout: i128,
    pub game_account_region: u8,
    pub logon_failures: i128,
    pub raf_present: bool,
    pub encoded_bit_count: usize,
}

#[derive(Clone, Debug)]
pub struct Protocol {
    codec: Codec,
    incoming_types: HashMap<(u8, u8), u32>,
    resume_request_type: u32,
    logon_request_type: u32,
    single_sign_on_request_type: u32,
    proof_response_type: u32,
    enable_encryption_type: u32,
    ping_type: u32,
    message_frame_type: u32,
    front_logon_response_type: u32,
    chat_channel_list_request_type: u32,
    chat_enum_conferences_type: u32,
    chat_status_change_type: u32,
    friends_toons_request_type: u32,
    profile_address_query_request_type: u32,
    profile_resolve_toon_name_request_type: u32,
    presence_statistics_subscribe_type: u32,
    temporary_presence_request_type: u32,
    toon_full_name_type: u32,
    profile_record_address_type: u32,
    profile_data_response_type: u32,
    token_type: u32,
}

impl Protocol {
    pub fn current() -> Result<Self> {
        Self::from_schema(Schema::Static(&schema::wire::SCHEMA))
    }

    pub fn from_executable(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(read_largest_metadata(path)?)
    }

    pub fn from_metadata_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(read_metadata(path)?)
    }

    pub fn new(metadata: Metadata) -> Result<Self> {
        Self::from_schema(metadata.into())
    }

    fn from_schema(metadata: Schema) -> Result<Self> {
        let incoming_types = INCOMING_TYPES
            .iter()
            .copied()
            .map(|(route, name)| Ok((route, metadata.unique_type_id(name)?)))
            .collect::<Result<HashMap<_, _>>>()?;
        let resume_request_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::ResumeRequest")?;
        let logon_request_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::LogonRequest3")?;
        let single_sign_on_request_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::SingleSignOnRequest3")?;
        let proof_response_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::ProofResponse")?;
        let enable_encryption_type =
            metadata.unique_type_id("Battlenet::Client::Connection::EnableEncryption")?;
        let ping_type = metadata.unique_type_id("Battlenet::Client::Connection::Ping")?;
        let message_frame_type =
            metadata.unique_type_id("Battlenet::Client::Connection::MessageFrame")?;
        let front_logon_response_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::LogonResponse3")?;
        let chat_channel_list_request_type =
            metadata.unique_type_id("Battlenet::Client::Chat::ChannelListRequest")?;
        let chat_enum_conferences_type =
            metadata.unique_type_id("Battlenet::Client::Chat::EnumConferenceDescriptions")?;
        let chat_status_change_type =
            metadata.unique_type_id("Battlenet::Client::Chat::StatusChangeRequest")?;
        let friends_toons_request_type =
            metadata.unique_type_id("Battlenet::Client::Friends::ToonsOfFriendsRequest")?;
        let profile_address_query_request_type =
            metadata.unique_type_id("Battlenet::Client::Profile::AddressQueryRequest")?;
        let profile_resolve_toon_name_request_type = metadata
            .unique_type_id("Battlenet::Client::Profile::ResolveToonNameToHandleRequest")?;
        let presence_statistics_subscribe_type =
            metadata.unique_type_id("Battlenet::Client::Presence::StatisticsSubscribe")?;
        let temporary_presence_request_type =
            metadata.unique_type_id("Battlenet::Client::Presence::TemporaryPresenceRequest")?;
        let toon_full_name_type = metadata.unique_type_id("Battlenet::Toon::FullName")?;
        let profile_record_address_type =
            metadata.unique_type_id("Battlenet::Profile::RecordAddress")?;
        let profile_data_response_type =
            metadata.unique_type_id("Battlenet::Profile::ProfileDataResponse")?;
        let token_type = metadata.unique_type_id("Battlenet::Token")?;
        let mut codec = Codec::from_schema(metadata);
        wire_layout::register(&mut codec)?;
        Ok(Self {
            codec,
            incoming_types,
            resume_request_type,
            logon_request_type,
            single_sign_on_request_type,
            proof_response_type,
            enable_encryption_type,
            ping_type,
            message_frame_type,
            front_logon_response_type,
            chat_channel_list_request_type,
            chat_enum_conferences_type,
            chat_status_change_type,
            friends_toons_request_type,
            profile_address_query_request_type,
            profile_resolve_toon_name_request_type,
            presence_statistics_subscribe_type,
            temporary_presence_request_type,
            toon_full_name_type,
            profile_record_address_type,
            profile_data_response_type,
            token_type,
        })
    }

    #[must_use]
    pub const fn codec(&self) -> &Codec {
        &self.codec
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per inbound native route, a flat dispatch table"
    )]
    pub fn decode_incoming_from(
        &self,
        reader: &mut BitReader<'_>,
        header: RoutingHeader,
    ) -> Result<(u32, Payload)> {
        let slot = header
            .service_slot
            .ok_or_else(|| native_error("native server record has no service slot"))?;
        let route = (slot, header.command_id);
        let type_id = self.incoming_type(slot, header.command_id)?;
        let payload_start = reader.position();
        let payload_result: Result<Payload> = (|| {
            if !has_custom_incoming_decoder(route) {
                return Ok(Payload::Reflected(self.codec.decode_from(reader, type_id)?));
            }
            Ok(match route {
                (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND) => decode::cache_stream_items(reader)?,
                (CHAT_SLOT, CHAT_CONFERENCES_RESPONSE_COMMAND) => {
                    decode::conference_descriptions(reader)?
                }
                (CHAT_SLOT, CHAT_CHANNEL_LIST_RESPONSE_COMMAND) => decode::channel_list(reader)?,
                (CHAT_SLOT, CHAT_JOIN_NOTIFY_COMMAND) => decode::chat_join(reader)?,
                (CHAT_SLOT, CHAT_INVITE_NOTIFY_COMMAND) => decode::chat_invite(reader)?,
                (PRESENCE_SLOT, PRESENCE_FIELDS_COMMAND) => decode::presence_fields(reader)?,
                (PRESENCE_SLOT, PRESENCE_UPDATE_COMMAND) => decode::presence_update(reader)?,
                (S2_MASTER_SLOT, S2_MASTER_CURRENT_SEASON_COMMAND) => {
                    decode::current_season(reader)?
                }
                (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND) => {
                    decode::club_settings(reader)?
                }
                (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND) => {
                    decode::club_summaries(self, type_id, reader)?
                }
                (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND) => {
                    decode::club_search(self, type_id, reader)?
                }
                (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_INVITE_ACTION_COMMAND) => {
                    decode::club_invite_action(self, type_id, reader)?
                }
                (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND) => {
                    decode::club_info(self, type_id, reader)?
                }
                (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND) => {
                    decode::member_clan_tag(self, type_id, reader)?
                }
                (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND) => {
                    decode::message_frame(self, type_id, reader)?
                }
                (CONNECTION_SLOT, CONNECTION_GAME_SITE_INFO_COMMAND) => {
                    decode::game_site_info(self, type_id, reader)?
                }
                (FRIENDS_SLOT, FRIENDS_LIST_COMMAND) => decode::friends_list(self, reader)?,
                (FRIENDS_SLOT, FRIENDS_TOONS_COMMAND) => {
                    decode::friend_toons(self, type_id, reader)?
                }
                (FRIENDS_SLOT, FRIENDS_ACCOUNT_BLOCK_COMMAND) => {
                    decode::account_blocks(self, reader)?
                }
                (FRIENDS_SLOT, FRIENDS_TOON_BLOCK_COMMAND) => decode::toon_blocks(reader)?,
                (CHAT_SLOT, CHAT_MEMBERSHIP_COMMAND) => {
                    decode::chat_membership(self, type_id, reader)?
                }
                (CHAT_SLOT, CHAT_MESSAGE_COMMAND) => decode::chat_message(self, type_id, reader)?,
                (CHAT_SLOT, CHAT_WHISPER_RECV_COMMAND) => {
                    decode::chat_whisper(self, type_id, reader, false)?
                }
                (CHAT_SLOT, CHAT_WHISPER_ECHO_COMMAND) => {
                    decode::chat_whisper(self, type_id, reader, true)?
                }
                (PROFILE_SLOT, PROFILE_SETTINGS_AVAILABLE_COMMAND) => {
                    decode::profile_settings(self, type_id, reader)?
                }
                (PROFILE_SLOT, PROFILE_READ_COMMAND) => decode::profile_read(
                    self,
                    self.profile_data_response_type,
                    self.token_type,
                    reader,
                )?,
                (PROFILE_SLOT, PROFILE_ADDRESS_QUERY_COMMAND) => {
                    decode::profile_address_query(self, type_id, reader)?
                }
                (PROFILE_SLOT, PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND) => {
                    decode::toon_name_resolved(self, type_id, reader)?
                }
                (TOON_SLOT, TOON_LIST_COMMAND) => decode::toon_list(self, type_id, reader)?,
                (TOON_SLOT, TOON_SELECTED_COMMAND) => decode::toon_selected(self, type_id, reader)?,
                (TOON_SLOT, TOON_WELCOME_COMMAND) => decode::toon_welcome(self, type_id, reader)?,
                _ => unreachable!("custom native route has no decoder"),
            })
        })();
        let root_type_name = self
            .codec
            .schema()
            .type_metadata(type_id)
            .ok()
            .and_then(|metadata| metadata.name)
            .unwrap_or_else(|| "unnamed".to_owned());
        let payload = payload_result.map_err(|error| {
            append_route_context(
                error,
                slot,
                header.command_id,
                type_id,
                payload_start,
                reader.position(),
                &root_type_name,
            )
        })?;
        Ok((type_id, payload))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per inbound native route, a flat dispatch table"
    )]
    pub(crate) fn decode_incoming_with_provenance_from(
        &self,
        reader: &mut BitReader<'_>,
        header: RoutingHeader,
    ) -> Result<DecodedIncoming> {
        let slot = header
            .service_slot
            .ok_or_else(|| native_error("native server record has no service slot"))?;
        let route = (slot, header.command_id);
        let type_id = self.incoming_type(slot, header.command_id)?;
        let payload_start = reader.position();
        let decoded = match route {
            (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND) => {
                Some(decode::cache_stream_items_with_provenance(reader)?)
            }
            (CHAT_SLOT, CHAT_INVITE_NOTIFY_COMMAND) => {
                Some(decode::chat_invite_with_provenance(reader)?)
            }
            (CHAT_SLOT, CHAT_CONFERENCES_RESPONSE_COMMAND) => {
                Some(decode::conference_descriptions_with_provenance(reader)?)
            }
            (CHAT_SLOT, CHAT_CHANNEL_LIST_RESPONSE_COMMAND) => {
                Some(decode::channel_list_with_provenance(reader)?)
            }
            (CHAT_SLOT, CHAT_JOIN_NOTIFY_COMMAND) => {
                Some(decode::chat_join_with_provenance(reader)?)
            }
            (PRESENCE_SLOT, PRESENCE_FIELDS_COMMAND) => {
                Some(decode::presence_fields_with_provenance(reader)?)
            }
            (PRESENCE_SLOT, PRESENCE_UPDATE_COMMAND) => {
                Some(decode::presence_update_with_provenance(reader)?)
            }
            (S2_MASTER_SLOT, S2_MASTER_CURRENT_SEASON_COMMAND) => {
                Some(decode::current_season_with_provenance(reader)?)
            }
            (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND) => {
                Some(decode::club_settings_with_provenance(reader)?)
            }
            (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND) => {
                Some(decode::club_search_with_provenance(self, type_id, reader)?)
            }
            (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_INVITE_ACTION_COMMAND) => Some(
                decode::club_invite_action_with_provenance(self, type_id, reader)?,
            ),
            (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND) => Some(
                decode::club_summaries_with_provenance(self, type_id, reader)?,
            ),
            (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND) => {
                Some(decode::club_info_with_provenance(self, type_id, reader)?)
            }
            (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND) => Some(
                decode::member_clan_tag_with_provenance(self, type_id, reader)?,
            ),
            (CONNECTION_SLOT, CONNECTION_GAME_SITE_INFO_COMMAND) => Some(
                decode::game_site_info_with_provenance(self, type_id, reader)?,
            ),
            (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND) => Some(
                decode::message_frame_with_provenance(self, type_id, reader)?,
            ),
            (FRIENDS_SLOT, FRIENDS_TOONS_COMMAND) => {
                Some(decode::friend_toons_with_provenance(self, type_id, reader)?)
            }
            (FRIENDS_SLOT, FRIENDS_LIST_COMMAND) => {
                Some(decode::friends_list_with_provenance(self, reader)?)
            }
            (FRIENDS_SLOT, FRIENDS_ACCOUNT_BLOCK_COMMAND) => {
                Some(decode::account_blocks_with_provenance(self, reader)?)
            }
            (FRIENDS_SLOT, FRIENDS_TOON_BLOCK_COMMAND) => {
                Some(decode::toon_blocks_with_provenance(reader)?)
            }
            (PROFILE_SLOT, PROFILE_SETTINGS_AVAILABLE_COMMAND) => Some(
                decode::profile_settings_with_provenance(self, type_id, reader)?,
            ),
            (PROFILE_SLOT, PROFILE_READ_COMMAND) => Some(decode::profile_read_with_provenance(
                self,
                self.profile_data_response_type,
                self.token_type,
                reader,
            )?),
            (PROFILE_SLOT, PROFILE_ADDRESS_QUERY_COMMAND) => Some(
                decode::profile_address_query_with_provenance(self, type_id, reader)?,
            ),
            (PROFILE_SLOT, PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND) => Some(
                decode::toon_name_resolved_with_provenance(self, type_id, reader)?,
            ),
            (TOON_SLOT, TOON_LIST_COMMAND) => {
                Some(decode::toon_list_with_provenance(self, type_id, reader)?)
            }
            (TOON_SLOT, TOON_SELECTED_COMMAND) => Some(decode::toon_selected_with_provenance(
                self, type_id, reader,
            )?),
            (TOON_SLOT, TOON_WELCOME_COMMAND) => {
                Some(decode::toon_welcome_with_provenance(self, type_id, reader)?)
            }
            (CHAT_SLOT, CHAT_MESSAGE_COMMAND) => {
                Some(decode::chat_message_with_provenance(self, type_id, reader)?)
            }
            (CHAT_SLOT, CHAT_MEMBERSHIP_COMMAND) => Some(decode::chat_membership_with_provenance(
                self, type_id, reader,
            )?),
            (CHAT_SLOT, CHAT_WHISPER_RECV_COMMAND) => Some(decode::chat_whisper_with_provenance(
                self, type_id, reader, false,
            )?),
            (CHAT_SLOT, CHAT_WHISPER_ECHO_COMMAND) => Some(decode::chat_whisper_with_provenance(
                self, type_id, reader, true,
            )?),
            _ => None,
        };
        debug_assert_eq!(
            decoded.is_some(),
            has_custom_incoming_decoder(route),
            "custom incoming route {slot}:{} must provide provenance",
            header.command_id
        );
        let (payload, custom_provenance) = if let Some(decoded) = decoded {
            (decoded.payload, decoded.provenance)
        } else {
            let (_, payload) = self.decode_incoming_from(reader, header)?;
            (payload, Vec::new())
        };
        let payload_end = reader.position();
        let provenance = if custom_provenance.is_empty() {
            self.trace_payload(reader.data(), type_id, payload_start, payload_end)
        } else {
            custom_provenance
        };
        Ok(DecodedIncoming {
            type_id,
            payload,
            provenance,
        })
    }

    fn trace_payload(
        &self,
        bytes: &[u8],
        type_id: u32,
        start_bit: usize,
        end_bit: usize,
    ) -> Vec<DecodedField> {
        let Ok(mut reader) = BitReader::new(bytes, Some(end_bit)) else {
            return Vec::new();
        };
        if reader.set_position(start_bit).is_err() {
            return Vec::new();
        }
        let Ok(decoded) = self.codec.decode_traced_from(&mut reader, type_id) else {
            return Vec::new();
        };
        if reader.position() == end_bit {
            decoded.fields
        } else {
            Vec::new()
        }
    }

    pub(crate) fn incoming_type(&self, slot: u8, command: u8) -> Result<u32> {
        self.incoming_types
            .get(&(slot, command))
            .copied()
            .ok_or(Error::UnmappedNativeRoute { slot, command })
    }

    pub fn resume_request(
        &self,
        bootstrap: &NativeHandoff,
        game_account_region: Option<u8>,
    ) -> Result<Vec<u8>> {
        let common = self.request_common(self.resume_request_type, "RequestCommon")?;
        let value = self.struct_value(
            self.resume_request_type,
            vec![
                ("RequestCommon", common),
                (
                    "m_account",
                    BsnValue::Bytes(bootstrap.account_mail().as_bytes().to_vec()),
                ),
                (
                    "m_gameAccountRegion",
                    BsnValue::Integer(i128::from(
                        game_account_region.unwrap_or(bootstrap.account_region),
                    )),
                ),
                (
                    "m_gameAccountName",
                    BsnValue::Bytes(bootstrap.game_account_name().as_bytes().to_vec()),
                ),
            ],
        )?;
        self.encode_record(
            AUTH_RESUME_COMMAND,
            AUTHENTICATION_SLOT,
            self.resume_request_type,
            &value,
        )
    }

    pub fn logon_request(&self, account_mail: &str) -> Result<Vec<u8>> {
        if account_mail.is_empty() {
            return Err(native_error("native logon account is empty"));
        }
        let common = self.request_common(self.logon_request_type, "m_requestCommon")?;
        let value = self.struct_value(
            self.logon_request_type,
            vec![
                ("m_requestCommon", common),
                (
                    "m_account",
                    BsnValue::Bytes(account_mail.as_bytes().to_vec()),
                ),
                (
                    "m_compatibility",
                    BsnValue::Integer(i128::from(SC2_NATIVE_VERSION)),
                ),
            ],
        )?;
        self.encode_record(
            AUTH_LOGON_COMMAND,
            AUTHENTICATION_SLOT,
            self.logon_request_type,
            &value,
        )
    }

    pub fn single_sign_on_request(&self, identifier: &[u8]) -> Result<Vec<u8>> {
        if identifier.is_empty() || identifier.len() > 512 {
            return Err(native_error(
                "native SSO identifier length is outside 1..=512 bytes",
            ));
        }
        let common = self.request_common(self.single_sign_on_request_type, "m_requestCommon")?;
        let value = self.struct_value(
            self.single_sign_on_request_type,
            vec![
                ("m_requestCommon", common),
                ("m_ssoId", BsnValue::Bytes(identifier.to_vec())),
                (
                    "m_compatibility",
                    BsnValue::Integer(i128::from(SC2_NATIVE_VERSION)),
                ),
            ],
        )?;
        self.encode_record(
            AUTH_SINGLE_SIGN_ON_COMMAND,
            AUTHENTICATION_SLOT,
            self.single_sign_on_request_type,
            &value,
        )
    }

    pub fn proof_response(&self, outputs: &[&[u8]]) -> Result<Vec<u8>> {
        let response_field = self.member_type(self.proof_response_type, "m_response")?;
        let item_type = self.array_element(response_field)?;
        let responses = outputs
            .iter()
            .map(|output| {
                self.struct_value(
                    item_type,
                    vec![("m_data", BsnValue::Bytes(output.to_vec()))],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let value = self.struct_value(
            self.proof_response_type,
            vec![("m_response", BsnValue::Array(responses))],
        )?;
        self.encode_record(
            AUTH_PROOF_COMMAND,
            AUTHENTICATION_SLOT,
            self.proof_response_type,
            &value,
        )
    }

    pub fn enable_encryption(&self) -> Result<Vec<u8>> {
        let value = self.struct_value(self.enable_encryption_type, Vec::new())?;
        self.encode_record(
            CONNECTION_ENABLE_ENCRYPTION_COMMAND,
            CONNECTION_SLOT,
            self.enable_encryption_type,
            &value,
        )
    }

    pub fn club_subscribe(
        &self,
        token: u32,
        handle: crate::native::model::ToonHandle,
    ) -> Result<Vec<u8>> {
        let mut writer =
            Self::record_writer(S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND, S2_MULTIPLAYER_SLOT)?;
        writer.write(u64::from(token), 32)?;
        writer.write(u64::from(handle.program_id), 32)?;
        writer.write(u64::from(handle.region), 8)?;
        writer.write(u64::from(handle.realm), 32)?;
        writer.write(handle.id, 64)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn club_search_request(&self, token: u32, name: &str) -> Result<Vec<u8>> {
        const CLUB_TYPE_GROUP: i128 = 1;
        const CATEGORY_ANY: i128 = 0;
        const SEARCH_BY_NAME: i128 = 0;

        let request_type = self
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::SearchClubsRequest")?;
        let name_type = self
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::SearchClubsRequest::Search::Name")?;
        let base_type = self
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::SearchClubs")?;
        let search = self.struct_value(
            name_type,
            vec![
                ("m_name", BsnValue::String(name.to_owned())),
                ("m_type", BsnValue::Integer(CLUB_TYPE_GROUP)),
                ("m_category", BsnValue::Integer(CATEGORY_ANY)),
                ("m_locale", BsnValue::FourCc(0)),
            ],
        )?;
        let base = self.struct_value(
            base_type,
            vec![("m_token", BsnValue::Integer(i128::from(token)))],
        )?;
        let value = self.struct_value(
            request_type,
            vec![
                ("SearchClubs", base),
                ("m_search", BsnValue::choice(SEARCH_BY_NAME, search)),
            ],
        )?;
        self.encode_record(
            S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND,
            S2_MULTIPLAYER_SLOT,
            request_type,
            &value,
        )
    }

    pub fn club_invite_answer(program: u32, club_id: u32, code: u8) -> Result<Vec<u8>> {
        const RESERVED: u64 = 4;
        let mut writer =
            Self::record_writer(S2_MULTIPLAYER_INVITE_ACTION_COMMAND, S2_MULTIPLAYER_SLOT)?;
        writer.write(u64::from(code), 2)?;
        writer.write(u64::from(program), 32)?;
        writer.write(0, 8)?;
        writer.write(0, 32)?;
        writer.write(0, 64)?;
        writer.write(u64::from(club_id), 32)?;
        writer.write(RESERVED, 11)?;
        writer.write(0, 16)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn club_info_request(&self, token: u32, club_ids: &[u32]) -> Result<Vec<u8>> {
        let request_type =
            self.incoming_type(S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND)?;
        let request_type = self
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::GetClubInfoRequest")
            .unwrap_or(request_type);
        let clubs = club_ids
            .iter()
            .map(|id| BsnValue::Integer(i128::from(*id)))
            .collect::<Vec<_>>();
        let value = self.struct_value(
            request_type,
            vec![
                ("m_token", BsnValue::Integer(i128::from(token))),
                ("m_clubs", BsnValue::Array(clubs)),
            ],
        )?;
        self.encode_record(
            S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND,
            S2_MULTIPLAYER_SLOT,
            request_type,
            &value,
        )
    }

    pub fn ping(&self, unix_time_micros: i64) -> Result<Vec<u8>> {
        let value = self.struct_value(
            self.ping_type,
            vec![(
                "m_timeData",
                BsnValue::Integer(i128::from(unix_time_micros)),
            )],
        )?;
        self.encode_record(
            CONNECTION_PING_COMMAND,
            CONNECTION_SLOT,
            self.ping_type,
            &value,
        )
    }

    pub fn transport_control_maintenance(&self, correlation_id: u32) -> Result<[Vec<u8>; 2]> {
        Ok([
            self.transport_control_frame(3, correlation_id, vec![0; 4])?,
            self.transport_control_frame(
                2,
                correlation_id.wrapping_sub(1),
                vec![
                    0x03, 0x00, 0x80, 0x05, 0x7e, 0x40, 0x02, 0x02, 0x0a, 0x03, 0x01,
                ],
            )?,
        ])
    }

    fn transport_control_frame(
        &self,
        command: u8,
        correlation_id: u32,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let header_list_type = self.member_type(self.message_frame_type, "m_headers")?;
        let header_type = self.array_element(header_list_type)?;
        let data_type = self.member_type(header_type, "m_data")?;
        let (route_index, route_type) = self.choice_variant(data_type, "route")?;
        let (correlation_index, correlation_type) =
            self.choice_variant(data_type, "correlation")?;
        let (content_index, content_type) = self.choice_variant(data_type, "content")?;
        let (stream_index, stream_type) = self.choice_variant(data_type, "stream")?;

        let route = self.struct_value(
            route_type,
            vec![
                ("m_name", BsnValue::FourCc(fourcc("TrnC"))),
                ("m_hash", BsnValue::Integer(0)),
                ("m_command", BsnValue::Integer(i128::from(command))),
                ("m_node", BsnValue::none()),
            ],
        )?;
        let correlation = self.struct_value(
            correlation_type,
            vec![
                ("m_id", BsnValue::Integer(i128::from(correlation_id))),
                ("m_reply", BsnValue::Bool(false)),
            ],
        )?;
        let content = self.struct_value(
            content_type,
            vec![
                ("m_size", BsnValue::Integer(payload.len() as i128)),
                ("m_encoding", BsnValue::Integer(1)),
            ],
        )?;
        let stream = self.struct_value(
            stream_type,
            vec![
                ("m_sequenceId", BsnValue::Integer(1)),
                ("m_more", BsnValue::Bool(false)),
            ],
        )?;
        let headers = [
            (route_index, route),
            (correlation_index, correlation),
            (content_index, content),
            (stream_index, stream),
        ]
        .into_iter()
        .map(|(index, value)| {
            self.struct_value(
                header_type,
                vec![("m_data", BsnValue::choice(index, value))],
            )
        })
        .collect::<Result<Vec<_>>>()?;

        let payload_type = self.member_type(self.message_frame_type, "m_payload")?;
        let frame_type = self.member_type(self.message_frame_type, "m_frameType")?;
        let mut writer = Self::record_writer(CONNECTION_MESSAGE_FRAME_COMMAND, CONNECTION_SLOT)?;
        self.codec
            .encode_reflected_into(&mut writer, payload_type, &BsnValue::Bytes(payload))?;
        self.codec
            .encode_reflected_into(&mut writer, frame_type, &BsnValue::Integer(129))?;
        self.codec.encode_reflected_into(
            &mut writer,
            header_list_type,
            &BsnValue::Array(headers),
        )?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn decode_logon_parameters(&self, bootstrap: &NativeHandoff) -> Result<LogonParameters> {
        let encoded = self.codec.decode(
            self.front_logon_response_type,
            bootstrap.logon_response(),
            None,
            0,
        )?;
        let mut padding = BitReader::new(bootstrap.logon_response(), None)?;
        padding.set_position(encoded.bit_count)?;
        if padding.remaining() != 0 && padding.read(padding.remaining())? != 0 {
            return Err(native_error(
                "Front logon response has non-zero trailing padding",
            ));
        }
        let wrapper = value_struct(&encoded.value, "Front LogonResponse3")?;
        let response = value_struct(
            required_field(wrapper, "LogonResponse")?,
            "Front LogonResponse wrapper",
        )?;
        let (result_index, result) =
            value_choice(required_field(response, "m_result")?, "Front logon result")?;
        if result_index != 0 {
            return Err(native_error("Front native logon was rejected"));
        }
        let success = value_struct(result, "Front logon success")?;
        let common = value_struct(
            required_field(success, "ResponseSuccessCommon")?,
            "Front logon common state",
        )?;
        let account_region = value_integer(
            required_field(success, "m_accountRegion")?,
            "Front account region",
        )?;
        if account_region != i128::from(bootstrap.account_region) {
            return Err(native_error("Front logon account region is inconsistent"));
        }
        let game_account_region = u8::try_from(value_integer(
            required_field(success, "m_gameAccountRegion")?,
            "Front game-account region",
        )?)
        .map_err(|_| native_error("Front game-account region is outside byte range"))?;
        let game_account_name = value_bytes(
            required_field(success, "m_gameAccountName")?,
            "Front game-account name",
        )?;
        if game_account_name != bootstrap.game_account_name().as_bytes() {
            return Err(native_error(
                "Front logon game-account name is inconsistent",
            ));
        }
        let ping_timeout = value_integer(
            required_field(common, "m_pingTimeout")?,
            "Front ping timeout",
        )?;
        let logon_failures = value_integer(
            required_field(success, "m_logonFailures")?,
            "Front logon failure count",
        )?;
        let raf_present = !matches!(required_field(wrapper, "m_raf")?, BsnValue::Optional(None));
        Ok(LogonParameters {
            ping_timeout,
            game_account_region,
            logon_failures,
            raf_present,
            encoded_bit_count: encoded.bit_count,
        })
    }

    pub fn cache_get_stream_items(
        &self,
        token: u32,
        channel: &str,
        item_name: &str,
        locale: &str,
    ) -> Result<Vec<u8>> {
        if channel.len() != 4 || item_name.len() != 4 || locale.len() != 4 {
            return Err(native_error(
                "cache stream channel, item, and locale must be four-character FourCCs",
            ));
        }
        let mut writer = Self::record_writer(CACHE_GET_STREAM_ITEMS_COMMAND, CACHE_SLOT)?;
        writer.write(u64::from(token), 32)?;
        write_generated_checksum(&mut writer, 23, 7)?;
        writer.write(0, 6)?;
        writer.write(1, 1)?;
        writer.write(u64::from(fourcc(channel)), 32)?;
        writer.write(u64::from(fourcc(item_name)), 32)?;
        writer.write(u64::from(fourcc(locale)), 32)?;
        writer.write(u64::from(0x7fff_ffff_u32 ^ 0x8000_0000), 32)?;
        writer.write(0, 1)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_enum_conference_descriptions(&self) -> Result<Vec<u8>> {
        let value = self.struct_value(self.chat_enum_conferences_type, Vec::new())?;
        self.encode_record(
            CHAT_ENUM_CONFERENCES_COMMAND,
            CHAT_SLOT,
            self.chat_enum_conferences_type,
            &value,
        )
    }

    pub fn chat_channel_list(&self) -> Result<Vec<u8>> {
        let value = self.struct_value(self.chat_channel_list_request_type, Vec::new())?;
        self.encode_record(
            CHAT_CHANNEL_LIST_REQUEST_COMMAND,
            CHAT_SLOT,
            self.chat_channel_list_request_type,
            &value,
        )
    }

    pub fn friend_toons(&self, account_id: u32) -> Result<Vec<u8>> {
        let marker_type =
            self.member_type(self.friends_toons_request_type, "ToonsOfFriendPacket")?;
        let marker = self.struct_value(marker_type, Vec::new())?;
        let value = self.struct_value(
            self.friends_toons_request_type,
            vec![
                ("ToonsOfFriendPacket", marker),
                ("m_accountId", BsnValue::Integer(i128::from(account_id))),
            ],
        )?;
        self.encode_record(
            FRIENDS_TOONS_COMMAND,
            FRIENDS_SLOT,
            self.friends_toons_request_type,
            &value,
        )
    }

    pub fn presence_statistics_subscribe(&self, on: bool) -> Result<Vec<u8>> {
        let value = self.struct_value(
            self.presence_statistics_subscribe_type,
            vec![("m_on", BsnValue::Bool(on))],
        )?;
        self.encode_record(
            PRESENCE_STATISTICS_SUBSCRIBE_COMMAND,
            PRESENCE_SLOT,
            self.presence_statistics_subscribe_type,
            &value,
        )
    }

    pub fn profile_address_query(&self, request_id: u32, account_id: u32) -> Result<Vec<u8>> {
        let player_target_type =
            self.member_type(self.profile_address_query_request_type, "m_playerTarget")?;
        let (account_variant, _) = self.choice_variant(player_target_type, "accountId")?;
        let value = self.struct_value(
            self.profile_address_query_request_type,
            vec![
                ("m_requestId", BsnValue::Integer(i128::from(request_id))),
                (
                    "m_playerTarget",
                    BsnValue::choice(account_variant, BsnValue::Integer(i128::from(account_id))),
                ),
            ],
        )?;
        self.encode_record(
            PROFILE_ADDRESS_QUERY_COMMAND,
            PROFILE_SLOT,
            self.profile_address_query_request_type,
            &value,
        )
    }

    pub fn resolve_toon_name(&self, name: &crate::native::model::ToonFullName) -> Result<Vec<u8>> {
        let marker_type = self.member_type(
            self.profile_resolve_toon_name_request_type,
            "ResolveToonNameToHandle",
        )?;
        let marker = self.struct_value(marker_type, Vec::new())?;
        let name = self.toon_full_name_value(name)?;
        let value = self.struct_value(
            self.profile_resolve_toon_name_request_type,
            vec![("ResolveToonNameToHandle", marker), ("m_name", name)],
        )?;
        self.encode_record(
            PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND,
            PROFILE_SLOT,
            self.profile_resolve_toon_name_request_type,
            &value,
        )
    }

    pub fn temporary_presence(
        &self,
        handles: &[crate::native::model::ToonHandle],
    ) -> Result<Vec<u8>> {
        if handles.is_empty() || handles.len() > 10 {
            return Err(native_error(
                "temporary presence requires between one and ten toon handles",
            ));
        }
        let marker_type =
            self.member_type(self.temporary_presence_request_type, "TemporaryPresence")?;
        let marker = self.struct_value(marker_type, Vec::new())?;
        let toon_list_type =
            self.member_type(self.temporary_presence_request_type, "m_toonList")?;
        let toon_handle_type = self.array_element(toon_list_type)?;
        let handles = handles
            .iter()
            .map(|handle| {
                self.struct_value(
                    toon_handle_type,
                    vec![
                        ("m_region", BsnValue::Integer(i128::from(handle.region))),
                        ("m_programId", BsnValue::FourCc(handle.program_id)),
                        ("m_realm", BsnValue::Integer(i128::from(handle.realm))),
                        ("m_id", BsnValue::Integer(i128::from(handle.id))),
                    ],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let value = self.struct_value(
            self.temporary_presence_request_type,
            vec![
                ("TemporaryPresence", marker),
                ("m_toonList", BsnValue::Array(handles)),
            ],
        )?;
        self.encode_record(
            PRESENCE_TEMPORARY_COMMAND,
            PRESENCE_SLOT,
            self.temporary_presence_request_type,
            &value,
        )
    }

    pub fn chat_join_public(
        &self,
        channel_name_id: u16,
        token: u32,
        locale: &str,
    ) -> Result<Vec<u8>> {
        if locale.len() != 4 {
            return Err(native_error("chat locale must be a four-character FourCC"));
        }
        let mut writer = Self::record_writer(CHAT_JOIN_REQUEST_COMMAND, CHAT_SLOT)?;
        writer.write(2, 2)?;
        writer.write(u64::from(fourcc(locale)), 32)?;
        writer.write(u64::from(channel_name_id), 16)?;
        writer.write(u64::from(token), 32)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_join_private(&self, name: &str, token: u32) -> Result<Vec<u8>> {
        if name.is_empty() {
            return Err(native_error("private chat channel name cannot be empty"));
        }
        let mut writer = Self::record_writer(CHAT_JOIN_REQUEST_COMMAND, CHAT_SLOT)?;
        writer.write(0, 2)?;
        encode_generated_utf8(&mut writer, name, 7, 124, 31)?;
        writer.write(u64::from(token), 32)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_join_club(&self, club_id: u32, token: u32) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(CHAT_JOIN_REQUEST_COMMAND, CHAT_SLOT)?;
        writer.write(3, 2)?;
        writer.write(0, 16)?;
        writer.write(u64::from(club_id), 32)?;
        writer.write(u64::from(token), 32)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_leave(&self, channel_index: u8) -> Result<Vec<u8>> {
        if usize::from(channel_index) >= CHANNEL_INDEX_COUNT {
            return Err(native_error("chat channel index must be between 0 and 6"));
        }
        let mut writer = Self::record_writer(CHAT_LEAVE_REQUEST_COMMAND, CHAT_SLOT)?;
        writer.write(u64::from(channel_index), 3)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_invite_answer(&self, channel_index: u8, accept: bool) -> Result<Vec<u8>> {
        if usize::from(channel_index) >= CHANNEL_INDEX_COUNT {
            return Err(native_error("chat channel index must be between 0 and 6"));
        }
        let command = if accept {
            CHAT_INVITE_ACCEPT_COMMAND
        } else {
            CHAT_INVITE_DECLINE_COMMAND
        };
        let mut writer = Self::record_writer(command, CHAT_SLOT)?;
        writer.write(u64::from(channel_index), 3)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_party_online(&self, channel_index: u8, member_handle: u32) -> Result<Vec<u8>> {
        if usize::from(channel_index) >= CHANNEL_INDEX_COUNT {
            return Err(native_error("chat channel index must be between 0 and 6"));
        }
        let status_type = self.member_type(self.chat_status_change_type, "m_statusChange")?;
        let (party_index, party_type) = self.choice_variant(status_type, "Party")?;
        let party = self.struct_value(
            party_type,
            vec![
                ("m_partyStatus", BsnValue::Integer(1)),
                ("m_expansionLevel", BsnValue::some(BsnValue::Integer(3))),
                ("m_captain", BsnValue::Bool(false)),
            ],
        )?;
        let value = self.struct_value(
            self.chat_status_change_type,
            vec![
                (
                    "m_channelIndex",
                    BsnValue::Integer(i128::from(channel_index)),
                ),
                (
                    "m_memberHandle",
                    BsnValue::Integer(i128::from(member_handle)),
                ),
                ("m_statusChange", BsnValue::choice(party_index, party)),
            ],
        )?;
        self.encode_record(
            CHAT_STATUS_CHANGE_COMMAND,
            CHAT_SLOT,
            self.chat_status_change_type,
            &value,
        )
    }

    pub fn toon_select(&self, toon_name: &str, realm: u32) -> Result<Vec<u8>> {
        let raw_name = toon_name.as_bytes();
        if !(2..=25).contains(&toon_name.chars().count()) || !(2..=100).contains(&raw_name.len()) {
            return Err(native_error(
                "toon name must contain 2..=25 characters and 2..=100 UTF-8 bytes",
            ));
        }
        let mut writer = Self::record_writer(TOON_SELECT_COMMAND, TOON_SLOT)?;
        writer.write((raw_name.len() - 2) as u64, 7)?;
        writer.write_bytes(raw_name, true)?;
        write_generated_checksum(&mut writer, 10, 2)?;
        writer.write(u64::from(realm), 32)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_message(&self, channel_index: u8, body: &str) -> Result<Vec<u8>> {
        if usize::from(channel_index) >= CHANNEL_INDEX_COUNT {
            return Err(native_error("chat channel index must be between 0 and 6"));
        }
        let mut writer = Self::record_writer(CHAT_MESSAGE_COMMAND, CHAT_SLOT)?;
        encode_generated_utf8(&mut writer, body, 10, 1020, 255)?;
        writer.write(u64::from(channel_index), 3)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_whisper(
        &self,
        target: &crate::native::model::WhisperTarget,
        body: &str,
    ) -> Result<Vec<u8>> {
        use crate::native::model::WhisperTarget;

        let mut writer = Self::record_writer(CHAT_WHISPER_SEND_COMMAND, CHAT_SLOT)?;
        match target {
            WhisperTarget::Presence(id) => {
                writer.write(0, 3)?;
                writer.write(u64::from(*id), 32)?;
            }
            WhisperTarget::Account(id) => {
                writer.write(3, 3)?;
                writer.write(u64::from(*id), 32)?;
            }
            WhisperTarget::ToonHandle(handle) => {
                writer.write(5, 3)?;
                writer.write(u64::from(handle.program_id), 32)?;
                writer.write(u64::from(handle.region), 8)?;
                writer.write(u64::from(handle.realm), 32)?;
                writer.write(handle.id, 64)?;
            }
            WhisperTarget::ToonName(name) => {
                let bytes = name.name.as_bytes();
                if !(2..=100).contains(&bytes.len()) {
                    return Err(native_error(
                        "whisper toon name must contain between 2 and 100 UTF-8 bytes",
                    ));
                }
                writer.write(1, 3)?;
                writer.write(u64::from(name.region), 8)?;
                writer.write(u64::from(name.program_id), 32)?;
                writer.write(u64::from(name.realm), 32)?;
                writer.write((bytes.len() - 2) as u64, 7)?;
                writer.write_bytes(bytes, true)?;
            }
            WhisperTarget::Name(_) => {
                return Err(native_error(
                    "whisper target has no resolved Battle.net identity",
                ));
            }
        }
        encode_generated_utf8(&mut writer, body, 10, 1020, 255)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn profile_read(
        &self,
        request_id: u32,
        address: crate::native::model::ProfileAddress,
        path: &[u8],
    ) -> Result<Vec<u8>> {
        if path.len() > u8::MAX as usize {
            return Err(native_error("profile read path is longer than 255 bytes"));
        }
        let address = self.struct_value(
            self.profile_record_address_type,
            vec![
                ("m_label", BsnValue::Integer(i128::from(address.label))),
                ("m_id", BsnValue::Integer(i128::from(address.id))),
            ],
        )?;
        let mut writer = Self::record_writer(PROFILE_READ_COMMAND, PROFILE_SLOT)?;

        writer.write(0, 32)?;
        writer.write(u64::from(request_id), 32)?;
        self.codec.encode_reflected_into(
            &mut writer,
            self.profile_record_address_type,
            &address,
        )?;
        writer.write(0, 5)?;
        writer.write(path.len() as u64, 8)?;
        writer.write_bytes(path, true)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    fn request_common(&self, root_type: u32, field_name: &str) -> Result<BsnValue> {
        let common_type = self.member_type(root_type, field_name)?;
        let versions_field_type = self.member_type(common_type, "m_versions")?;
        let version_element_type = self.array_element(versions_field_type)?;
        let versions = SC2_MACOS_NATIVE_VERSIONS
            .into_iter()
            .map(|(program, component, version)| {
                self.struct_value(
                    version_element_type,
                    vec![
                        ("m_programId", BsnValue::FourCc(fourcc(program))),
                        ("m_component", BsnValue::FourCc(fourcc(component))),
                        ("m_version", BsnValue::Integer(i128::from(version))),
                    ],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        self.struct_value(
            common_type,
            vec![
                ("m_program", BsnValue::FourCc(fourcc("S2"))),
                ("m_platform", BsnValue::FourCc(fourcc("Mc64"))),
                ("m_locale", BsnValue::FourCc(fourcc("enUS"))),
                ("m_versions", BsnValue::Array(versions)),
            ],
        )
    }

    fn toon_full_name_value(&self, name: &crate::native::model::ToonFullName) -> Result<BsnValue> {
        self.struct_value(
            self.toon_full_name_type,
            vec![
                ("m_region", BsnValue::Integer(i128::from(name.region))),
                ("m_programId", BsnValue::FourCc(name.program_id)),
                ("m_realm", BsnValue::Integer(i128::from(name.realm))),
                ("m_name", BsnValue::String(name.name.clone())),
            ],
        )
    }

    fn encode_record(
        &self,
        command: u8,
        service_slot: u8,
        type_id: u32,
        value: &BsnValue,
    ) -> Result<Vec<u8>> {
        let mut writer = BitWriter::new();
        writer.write(u64::from(command), 6)?;
        writer.write(1, 1)?;
        writer.write(u64::from(service_slot), 4)?;
        self.codec
            .encode_reflected_into(&mut writer, type_id, value)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    fn record_writer(command: u8, service_slot: u8) -> Result<BitWriter> {
        let mut writer = BitWriter::new();
        writer.write(u64::from(command), 6)?;
        writer.write(1, 1)?;
        writer.write(u64::from(service_slot), 4)?;
        Ok(writer)
    }

    fn struct_value(&self, type_id: u32, values: Vec<(&str, BsnValue)>) -> Result<BsnValue> {
        let type_id = self.peel_alias(type_id)?;
        let shape = self.codec.schema().shape(type_id)?;
        if shape.kind != TypeKind::Struct {
            return Err(native_error(format!("BSN type {type_id} is not a struct")));
        }
        let mut fields = Vec::with_capacity(values.len());
        for (name, value) in values {
            let position = shape
                .member_names
                .iter()
                .position(|candidate| candidate.as_deref() == Some(name))
                .ok_or_else(|| {
                    native_error(format!("BSN struct {type_id} has no field named {name}"))
                })?;
            fields.push(BsnField::named(
                shape.index_values[position],
                name,
                self.coerce_field_value(shape.member_types[position], value)?,
            ));
        }
        Ok(BsnValue::Struct(BsnStruct::new(type_id, fields)))
    }

    pub(crate) fn member_type(&self, type_id: u32, field_name: &str) -> Result<u32> {
        let type_id = self.peel_alias(type_id)?;
        let shape = self.codec.schema().shape(type_id)?;
        let position = shape
            .member_names
            .iter()
            .position(|name| name.as_deref() == Some(field_name))
            .ok_or_else(|| {
                native_error(format!(
                    "BSN struct {type_id} has no field named {field_name}"
                ))
            })?;
        Ok(shape.member_types[position])
    }

    pub(crate) fn array_element(&self, type_id: u32) -> Result<u32> {
        let type_id = self.peel_alias(type_id)?;
        let shape = self.codec.schema().shape(type_id)?;
        if shape.kind != TypeKind::Array {
            return Err(native_error(format!("BSN type {type_id} is not an array")));
        }
        shape
            .element_type
            .ok_or_else(|| native_error(format!("BSN array {type_id} has no element type")))
    }

    fn choice_variant(&self, type_id: u32, variant_name: &str) -> Result<(i128, u32)> {
        let type_id = self.peel_alias(type_id)?;
        let shape = self.codec.schema().shape(type_id)?;
        if shape.kind != TypeKind::Choice {
            return Err(native_error(format!("BSN type {type_id} is not a choice")));
        }
        let position = shape
            .member_names
            .iter()
            .position(|name| name.as_deref() == Some(variant_name))
            .ok_or_else(|| {
                native_error(format!(
                    "BSN choice {type_id} has no variant named {variant_name}"
                ))
            })?;
        Ok((shape.index_values[position], shape.member_types[position]))
    }

    pub(crate) fn peel_alias(&self, mut type_id: u32) -> Result<u32> {
        let mut depth = 0;
        loop {
            let shape = self.codec.schema().shape(type_id)?;
            if shape.kind != TypeKind::Alias {
                return Ok(type_id);
            }
            type_id = shape
                .element_type
                .ok_or_else(|| native_error("native alias has no element type"))?;
            depth += 1;
            if depth > self.codec.schema().type_count() {
                return Err(native_error("native metadata contains an alias cycle"));
            }
        }
    }

    fn coerce_field_value(&self, mut type_id: u32, value: BsnValue) -> Result<BsnValue> {
        loop {
            let shape = self.codec.schema().shape(type_id)?;
            match shape.kind {
                TypeKind::Alias => {
                    type_id = shape
                        .element_type
                        .ok_or_else(|| native_error("native alias has no element type"))?;
                }
                TypeKind::Optional if !matches!(value, BsnValue::Optional(_)) => {
                    return Ok(BsnValue::some(value));
                }
                _ => return Ok(value),
            }
        }
    }
}

fn write_generated_checksum(writer: &mut BitWriter, width: usize, seed: u32) -> Result<()> {
    if !(1..=32).contains(&width) || writer.position() < 32 {
        return Err(native_error("generated checksum parameters are invalid"));
    }
    let byte_index = writer.position() / 8;
    if byte_index < 4 {
        return Err(native_error(
            "generated checksum requires four preceding encoded bytes",
        ));
    }
    let encoded = writer.as_bytes();
    let preceding_four = u32::from_le_bytes(
        encoded[byte_index - 4..byte_index]
            .try_into()
            .expect("slice is four bytes"),
    );
    let preceding_two = u32::from(u16::from_le_bytes(
        encoded[byte_index - 2..byte_index]
            .try_into()
            .expect("slice is two bytes"),
    ));
    let checksum = seed
        .wrapping_add(preceding_four)
        .wrapping_add(preceding_two)
        .rotate_left(8);
    let mask = if width == 32 {
        u32::MAX
    } else {
        (1_u32 << width) - 1
    };
    writer.write(u64::from(checksum & mask), width)
}

fn encode_generated_utf8(
    writer: &mut BitWriter,
    value: &str,
    length_bits: usize,
    maximum_bytes: usize,
    maximum_characters: usize,
) -> Result<()> {
    let bytes = value.as_bytes();
    if bytes.len() > maximum_bytes || value.chars().count() > maximum_characters {
        return Err(native_error(
            "generated native string exceeds its schema bound",
        ));
    }
    writer.write(bytes.len() as u64, length_bits)?;
    writer.write_bytes(bytes, true)
}

fn transparent_value(value: &BsnValue) -> &BsnValue {
    match value {
        BsnValue::Optional(Some(value)) => transparent_value(value),
        _ => value,
    }
}

fn required_field<'a>(value: &'a BsnStruct, name: &str) -> Result<&'a BsnValue> {
    value
        .get(name)
        .ok_or_else(|| native_error(format!("native struct is missing {name}")))
}

fn value_struct<'a>(value: &'a BsnValue, label: &str) -> Result<&'a BsnStruct> {
    transparent_value(value)
        .as_struct()
        .ok_or_else(|| native_error(format!("{label} is not a struct")))
}

fn value_choice<'a>(value: &'a BsnValue, label: &str) -> Result<(i128, &'a BsnValue)> {
    match transparent_value(value) {
        BsnValue::Choice { index, value } => Ok((*index, value)),
        _ => Err(native_error(format!("{label} is not a choice"))),
    }
}

fn value_integer(value: &BsnValue, label: &str) -> Result<i128> {
    match transparent_value(value) {
        BsnValue::Integer(value) => Ok(*value),
        _ => Err(native_error(format!("{label} is not an integer"))),
    }
}

fn value_bytes<'a>(value: &'a BsnValue, label: &str) -> Result<&'a [u8]> {
    match transparent_value(value) {
        BsnValue::Bytes(value) => Ok(value),
        _ => Err(native_error(format!("{label} is not bytes"))),
    }
}

fn native_error(message: impl Into<String>) -> Error {
    Error::Native(message.into())
}

fn append_route_context(
    error: Error,
    slot: u8,
    command: u8,
    type_id: u32,
    payload_start: usize,
    failure_position: usize,
    type_name: &str,
) -> Error {
    if matches!(error, Error::IncompleteFrame(_)) {
        return error;
    }
    let context = format!(
        "native route slot={slot} command={command}, root {} (#{type_id}), payload bits {}..{}",
        type_name,
        0,
        failure_position.saturating_sub(payload_start)
    );
    match error {
        Error::BsnWire(message) => Error::BsnWire(format!("{message}; {context}")),
        Error::Native(message) => Error::Native(format!("{message}; {context}")),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bsn::{
        bits::{BitReader, decode_routing_header},
        codec::WireLayoutSupport,
    };

    const CURRENT_METADATA: &str = "../protocol/bsn/sc2-97364-metadata.bin";

    fn protocol() -> Protocol {
        Protocol::current().unwrap()
    }

    #[test]
    fn generated_wire_schema_matches_its_metadata_source() {
        if !Path::new(CURRENT_METADATA).is_file() {
            return;
        }
        let generated = protocol();
        let parsed = Protocol::from_metadata_file(CURRENT_METADATA).unwrap();
        let generated_schema = generated.codec().schema();
        let parsed_schema = parsed.codec().schema();
        let type_ids = generated_schema.known_type_ids();

        assert_eq!(type_ids.len(), 885);
        for type_id in type_ids {
            assert_eq!(
                generated_schema.type_metadata(type_id).unwrap().name,
                parsed_schema.type_metadata(type_id).unwrap().name,
                "type name mismatch for #{type_id}"
            );
            assert_eq!(
                generated_schema.shape(type_id).unwrap(),
                parsed_schema.shape(type_id).unwrap(),
                "type shape mismatch for #{type_id}"
            );
        }
    }

    const INVITATION: &str = "f6050002991201000000010000000006acf90600415669560000";
    const RETAIL_INVITE: &str = "f6050002991201000000010000000004220601004156c9560000";

    fn invitation_field(record: &[u8], offset: usize, width: usize) -> u64 {
        let mut reader = BitReader::new(record, None).unwrap();
        reader.set_position(offset).unwrap();
        reader.read(width).unwrap()
    }

    fn same_bits(left: &[u8], right: &[u8], range: std::ops::Range<usize>) -> bool {
        range
            .clone()
            .all(|bit| invitation_field(left, bit, 1) == invitation_field(right, bit, 1))
    }

    #[test]
    fn every_obfuscated_incoming_type_has_an_explicit_wire_strategy() {
        let protocol = protocol();
        for &(route, name) in INCOMING_TYPES {
            let type_id = protocol.incoming_type(route.0, route.1).unwrap();
            let shape = protocol.codec.schema().shape(type_id).unwrap();
            if !shape.obfuscated {
                continue;
            }
            let support = protocol.codec.wire_layout_support(type_id).unwrap();
            assert!(
                has_custom_incoming_decoder(route)
                    || support != WireLayoutSupport::UnsupportedObfuscated,
                "obfuscated incoming type {name} has neither a custom route decoder nor a codec registration"
            );
        }

        let invite = protocol
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::InviteAction")
            .unwrap();
        assert!(matches!(
            protocol.codec.wire_layout_support(invite).unwrap(),
            WireLayoutSupport::Custom("identity Client::Club::InviteAction")
        ));

        let clubs = protocol
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::GetToonClubsResponse")
            .unwrap();
        assert_eq!(
            protocol.codec.wire_layout_support(clubs).unwrap(),
            WireLayoutSupport::UnsupportedObfuscated
        );
    }

    const RETAIL_GET_TOON_CLUBS: &str = "ee0500000000000a6602010000000100000000 1ab3e406";
    const RETAIL_TOON_CLUBS_REPLY: &str = concat!(
        "ee056156e55503000000020082ac09010c546573742047726f75702041c12bfaeabe7b8164",
        "5400000000000000010100014cb200000080a1f32002000000000000000000000000211201",
        "08110100000000"
    );

    #[test]
    fn a_group_search_encodes_as_a_record_the_service_can_read() {
        let protocol = protocol();

        let packet = protocol.club_search_request(7, "barcraft").unwrap();

        let mut reader = BitReader::new(&packet, None).unwrap();
        assert_eq!(
            u8::try_from(reader.read(6).unwrap()).unwrap(),
            S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND
        );
        assert_eq!(reader.read(1).unwrap(), 1);
        assert_eq!(
            u8::try_from(reader.read(4).unwrap()).unwrap(),
            S2_MULTIPLAYER_SLOT
        );

        let request_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Client::Club::SearchClubsRequest")
            .unwrap();
        let value = protocol
            .codec()
            .decode_reflected_from(&mut reader, request_type)
            .unwrap();
        let mut found = Vec::new();
        collect_strings(&value, &mut found);
        assert_eq!(found, vec!["barcraft".to_owned()]);
    }

    fn collect_strings(value: &BsnValue, found: &mut Vec<String>) {
        match value {
            BsnValue::String(text) => found.push(text.clone()),
            BsnValue::Array(items) => {
                for item in items {
                    collect_strings(item, found);
                }
            }
            BsnValue::Optional(Some(inner)) => collect_strings(inner, found),
            BsnValue::Choice { value, .. } => collect_strings(value, found),
            BsnValue::Struct(fields) => {
                for field in &fields.fields {
                    collect_strings(&field.value, found);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn a_club_request_matches_the_one_retail_sends() {
        let retail = hex::decode(RETAIL_GET_TOON_CLUBS.replace(' ', "")).unwrap();
        let handle = crate::native::model::ToonHandle {
            region: 1,
            program_id: 0x5332,
            realm: 1,
            id: 0x00d5_9f26,
        };
        assert_eq!(protocol().club_subscribe(0, handle).unwrap(), retail);

        let header = decode_routing_header(&retail, None).unwrap();
        assert_eq!(header.command_id, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND);
        assert_eq!(header.service_slot, Some(S2_MULTIPLAYER_SLOT));
        assert_eq!(invitation_field(&retail, 11, 32), 0, "the token");
        assert_eq!(invitation_field(&retail, 43, 32), 0x5332, "program id");
        assert_eq!(invitation_field(&retail, 115, 64), 0x00d5_9f26, "toon id");
    }

    const THREE_CLUB_REPLY: &str = concat!(
        "ee056356e55503000000010082ab09010463656377f72bfaeabe6f45645400000000000000",
        "000100014cb200000000d32963020000000000000000000000602b72aa130000000200415609",
        "010c546573742047726f75702041c12bfaeabe7b81645400000000000000010100014cb20000",
        "0080a1f320020000000000000000000000602b72aa130000000100415514000a4d6964696761",
        "74696f6eee2bfaea7ed34d645400000000000000010200014cb200000040054d4447544e4283",
        "5f0400000000000000000000000003321e32c884d01e000000004d011a00010c000000000920",
        "36e353af92ff53af97100000000c01000299120000090d05090100029912000006090509000000",
        "010000000142074151c9b9030000000002c537e5efc87774cf724ca67d01835cdcab380000000",
        "03400000001c8a9d7c91f0265000101000a6602f39e460c010001"
    );

    const RECORD_BEHIND_A_CLUB_REPLY: &str = "c006000000010000201600000017";

    fn club_reply(protocol: &Protocol, hex: &str) -> Vec<(u32, String, u8, u8)> {
        let mut reply = hex::decode(hex).unwrap();
        reply.extend_from_slice(&hex::decode(RECORD_BEHIND_A_CLUB_REPLY).unwrap());
        let Payload::ClubSummaries(clubs) = decode_incoming(protocol, &reply) else {
            panic!("the club reply did not decode as summaries");
        };
        clubs
            .into_iter()
            .map(|club| {
                (
                    club.club_id,
                    club.name.unwrap_or_default(),
                    club.kind,
                    club.category,
                )
            })
            .collect()
    }

    #[test]
    fn a_club_reply_names_every_group() {
        let protocol = protocol();
        assert_eq!(
            club_reply(&protocol, RETAIL_TOON_CLUBS_REPLY),
            vec![(535_241, "Test Group A".to_owned(), 1, 1)]
        );
        assert_eq!(
            club_reply(&protocol, THREE_CLUB_REPLY),
            vec![
                (535_225, "cecw".to_owned(), 1, 1),
                (535_241, "Test Group A".to_owned(), 1, 1),
                (535_220, "Midigation".to_owned(), 2, 0),
            ]
        );
    }

    #[test]
    fn an_invitation_answer_matches_the_layout_the_client_parses() {
        let invitation = hex::decode(INVITATION).unwrap();
        assert_eq!(invitation_field(&invitation, 11, 2), 0, "INVITED");
        assert_eq!(invitation_field(&invitation, 13, 32), 0x5332, "S2");
        assert_eq!(invitation_field(&invitation, 149, 32), 535_241);

        for code in 1..=2 {
            let answer = Protocol::club_invite_answer(0x5332, 535_241, code).unwrap();
            assert_eq!(answer.len(), INVITE_RECORD_BYTES);
            let header = decode_routing_header(&answer, None).unwrap();
            assert_eq!(header.command_id, S2_MULTIPLAYER_INVITE_ACTION_COMMAND);
            assert_eq!(header.service_slot, Some(S2_MULTIPLAYER_SLOT));
            assert_eq!(invitation_field(&answer, 11, 2), u64::from(code));
            assert_eq!(invitation_field(&answer, 13, 32), 0x5332, "program id");
            assert_eq!(invitation_field(&answer, 45, 8), 0, "member kind");
            assert_eq!(invitation_field(&answer, 53, 32), 0, "member realm");
            assert_eq!(invitation_field(&answer, 85, 64), 0, "member toon id");
            assert_eq!(invitation_field(&answer, 149, 32), 535_241);
            assert_eq!(invitation_field(&answer, 192, 16), 0, "m_result");
        }
    }

    const RETAIL_CLUB_JOIN: &str = "405d08000041560980001800";

    #[test]
    fn a_group_chat_join_matches_the_one_retail_sends() {
        let retail = hex::decode(RETAIL_CLUB_JOIN).unwrap();
        let header = decode_routing_header(&retail, None).unwrap();
        assert_eq!(header.command_id, CHAT_JOIN_REQUEST_COMMAND);
        assert_eq!(header.service_slot, Some(CHAT_SLOT));
        assert_eq!(invitation_field(&retail, 11, 2), 3, "the group locator arm");
        assert_eq!(invitation_field(&retail, 29, 32), 535_241, "the group");
        let token = u32::try_from(invitation_field(&retail, 61, 32)).unwrap();

        let ours = protocol().chat_join_club(535_241, token).unwrap();
        assert_eq!(ours.len(), retail.len());
        assert!(same_bits(&ours, &retail, 0..13));
        assert!(same_bits(&ours, &retail, 29..93));
        assert_eq!(invitation_field(&ours, 13, 16), 0);
    }

    #[test]
    fn the_invitation_layout_matches_the_retail_vector() {
        let invitation = hex::decode(INVITATION).unwrap();
        let retail = hex::decode(RETAIL_INVITE).unwrap();
        assert_eq!(invitation_field(&retail, 11, 2), 0);
        assert_eq!(invitation_field(&retail, 149, 32), 535_241);
        assert_eq!(invitation_field(&retail, 192, 16), 0, "m_result");
        assert!(!same_bits(&retail, &invitation, 13..149), "m_member");
    }

    fn decode_outgoing(protocol: &Protocol, packet: &[u8], type_id: u32) -> BsnValue {
        let mut reader = BitReader::new(packet, None).unwrap();
        reader.read(6).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        reader.read(4).unwrap();
        protocol
            .codec()
            .decode_reflected_from(&mut reader, type_id)
            .unwrap()
    }

    fn decode_incoming(protocol: &Protocol, packet: &[u8]) -> Payload {
        let mut reader = BitReader::new(packet, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = Some(u8::try_from(reader.read(4).unwrap()).unwrap());
        protocol
            .decode_incoming_from(
                &mut reader,
                RoutingHeader {
                    command_id,
                    service_slot,
                    bit_count: 11,
                },
            )
            .unwrap()
            .1
    }

    #[test]
    fn current_metadata_builds_authentication_packets() {
        let protocol = protocol();
        let logon = protocol.logon_request("person@example.test").unwrap();
        let header = decode_routing_header(&logon, None).unwrap();
        assert_eq!(header.command_id, AUTH_LOGON_COMMAND);
        assert_eq!(header.service_slot, Some(AUTHENTICATION_SLOT));

        let sso = protocol.single_sign_on_request(&[7; 48]).unwrap();
        let header = decode_routing_header(&sso, None).unwrap();
        assert_eq!(header.command_id, AUTH_SINGLE_SIGN_ON_COMMAND);

        let proof = protocol.proof_response(&[&[1; 49]]).unwrap();
        let header = decode_routing_header(&proof, None).unwrap();
        assert_eq!(header.command_id, AUTH_PROOF_COMMAND);

        let marker = protocol.enable_encryption().unwrap();
        assert_eq!(marker.len(), 2);
        let header = decode_routing_header(&marker, None).unwrap();
        assert_eq!(header.command_id, CONNECTION_ENABLE_ENCRYPTION_COMMAND);
        assert_eq!(header.service_slot, Some(CONNECTION_SLOT));
    }

    #[test]
    fn current_metadata_builds_retail_identity_requests() {
        let protocol = protocol();
        let name = crate::native::model::ToonFullName {
            region: 1,
            program_id: fourcc("S2"),
            realm: 1,
            name: "Tagban#542".into(),
        };
        let resolve = protocol.resolve_toon_name(&name).unwrap();
        let header = decode_routing_header(&resolve, None).unwrap();
        assert_eq!(header.command_id, PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND);
        assert_eq!(header.service_slot, Some(PROFILE_SLOT));
        let value = decode_outgoing(
            &protocol,
            &resolve,
            protocol.profile_resolve_toon_name_request_type,
        );
        let root = value.as_struct().unwrap();
        assert_eq!(
            root.get("m_name")
                .unwrap()
                .as_struct()
                .unwrap()
                .get("m_name"),
            Some(&BsnValue::String("Tagban#542".into()))
        );
    }

    #[test]
    fn club_invitation_routes_to_the_s2_multiplayer_invite_action() {
        let packet = hex::decode("f6050002991201000000010000000006acf9060041557c550000").unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        assert_eq!(header.command_id, S2_MULTIPLAYER_INVITE_ACTION_COMMAND);
        assert_eq!(header.service_slot, Some(S2_MULTIPLAYER_SLOT));
    }

    #[test]
    fn club_invitation_reads_the_group_id_battle_net_links_use() {
        for (packet, expected) in [
            (
                "f6050002991201000000010000000006acf90600415667560000",
                535_239,
            ),
            (
                "f6050002991201000000010000000006acf9060041557c550000",
                535_228,
            ),
        ] {
            let packet = hex::decode(packet).unwrap();
            assert_eq!(packet.len(), 26);
            let header = decode_routing_header(&packet, None).unwrap();
            assert_eq!(header.command_id, S2_MULTIPLAYER_INVITE_ACTION_COMMAND);
            assert_eq!(header.service_slot, Some(S2_MULTIPLAYER_SLOT));

            let protocol = protocol();
            let Payload::ClubInviteAction(action) = decode_incoming(&protocol, &packet) else {
                panic!("club invitation did not decode as an invite action");
            };
            assert_eq!(action.club_id, expected);
        }
    }

    #[test]
    fn club_names_use_an_eight_bit_byte_count() {
        let response = hex::decode(concat!(
            "f7056156e5550300000001008",
            "2ac070106796f696e6b61e12bfaeabe4f8164540000000000000000010001",
            "4cf2a9d56aa5f528d602000000000000000000000021",
            "0020ab07",
        ))
        .unwrap();
        let protocol = protocol();
        let name_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Club::ClubName")
            .unwrap();
        let mut reader = BitReader::new(&response, None).unwrap();
        reader.set_position(124).unwrap();
        let value = protocol
            .codec()
            .decode_from(&mut reader, name_type)
            .unwrap();
        assert_eq!(value, BsnValue::String("yoinka".into()));
    }

    #[test]
    fn club_invitation_consumes_the_whole_record() {
        let packet = hex::decode("f6050002991201000000010000000006acf90600415667560000").unwrap();
        let protocol = protocol();
        let mut reader = BitReader::new(&packet, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = Some(u8::try_from(reader.read(4).unwrap()).unwrap());
        protocol
            .decode_incoming_from(
                &mut reader,
                RoutingHeader {
                    command_id,
                    service_slot,
                    bit_count: 11,
                },
            )
            .unwrap();
        assert_eq!(reader.position(), packet.len() * 8);
    }

    #[test]
    fn custom_invitation_layout_reports_exact_nested_provenance() {
        let packet = hex::decode("f6050002991201000000010000000006acf90600415667560000").unwrap();
        let protocol = protocol();
        let mut reader = BitReader::new(&packet, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = Some(u8::try_from(reader.read(4).unwrap()).unwrap());
        let decoded = protocol
            .decode_incoming_with_provenance_from(
                &mut reader,
                RoutingHeader {
                    command_id,
                    service_slot,
                    bit_count: 11,
                },
            )
            .unwrap();

        for (path, range) in [
            ("value.m_action.m_code", 11..13),
            ("value.m_action.m_member.m_programId", 13..45),
            ("value.m_action.m_member.m_region", 45..53),
            ("value.m_action.m_member.m_realm", 53..85),
            ("value.m_action.m_member.m_id", 85..149),
            ("value.m_action.m_clubId", 149..181),
            ("value.m_action.filler_before_m_result", 181..192),
            ("value.m_action.m_result", 192..208),
        ] {
            let field = decoded
                .provenance
                .iter()
                .find(|field| field.path == path)
                .unwrap_or_else(|| panic!("missing provenance for {path}"));
            assert_eq!(field.start_bit..field.end_bit, range, "{path}");
        }
    }

    #[test]
    fn temporary_presence_encodes_the_retail_toon_handle_layout() {
        let protocol = protocol();
        let handle = crate::native::model::ToonHandle {
            region: 1,
            program_id: fourcc("S2"),
            realm: 1,
            id: 0x1234_5678_90ab_cdef,
        };
        let packet = protocol.temporary_presence(&[handle]).unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        assert_eq!(header.command_id, PRESENCE_TEMPORARY_COMMAND);
        assert_eq!(header.service_slot, Some(PRESENCE_SLOT));

        let value = decode_outgoing(&protocol, &packet, protocol.temporary_presence_request_type);
        let root = value.as_struct().unwrap();
        let BsnValue::Array(handles) = root.get("m_toonList").unwrap() else {
            panic!("temporary presence toon list did not decode as an array");
        };
        assert_eq!(handles.len(), 1);
        let handle = handles[0].as_struct().unwrap();
        assert_eq!(
            handle.get("m_programId"),
            Some(&BsnValue::FourCc(fourcc("S2")))
        );
        assert_eq!(handle.get("m_region"), Some(&BsnValue::Integer(1)));
        assert_eq!(handle.get("m_realm"), Some(&BsnValue::Integer(1)));
        assert_eq!(
            handle.get("m_id"),
            Some(&BsnValue::Integer(0x1234_5678_90ab_cdef))
        );
    }

    #[test]
    fn whisper_writer_matches_the_live_generated_account_layout() {
        let protocol = protocol();
        let packet = protocol
            .chat_whisper(&crate::native::model::WhisperTarget::Account(42), "hello")
            .unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        assert_eq!(CHAT_WHISPER_SEND_COMMAND, 19);
        assert_eq!(CHAT_WHISPER_RECV_COMMAND, 19);
        assert_eq!(CHAT_WHISPER_UNDELIVERABLE_COMMAND, 20);
        assert_eq!(CHAT_WHISPER_ECHO_COMMAND, 30);
        assert_eq!(header.command_id, 19);
        assert_eq!(header.service_slot, Some(CHAT_SLOT));

        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(header.bit_count).unwrap();
        assert_eq!(reader.read(3).unwrap(), 3);
        assert_eq!(reader.read(32).unwrap(), 42);
        assert_eq!(reader.read(10).unwrap(), 5);
        assert_eq!(reader.read_bytes(5, true).unwrap(), b"hello");
    }

    #[test]
    fn whisper_writer_matches_the_live_generated_presence_layout() {
        let protocol = protocol();
        let packet = protocol
            .chat_whisper(
                &crate::native::model::WhisperTarget::Presence(0x1234_5678),
                "hi",
            )
            .unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(header.bit_count).unwrap();
        assert_eq!(reader.read(3).unwrap(), 0);
        assert_eq!(reader.read(32).unwrap(), 0x1234_5678);
        assert_eq!(reader.read(10).unwrap(), 2);
        assert_eq!(reader.read_bytes(2, true).unwrap(), b"hi");
    }

    #[test]
    fn whisper_writer_matches_the_retail_vector() {
        let protocol = protocol();
        let packet = protocol
            .chat_whisper(
                &crate::native::model::WhisperTarget::Presence(0x02b7_5a16),
                ".",
            )
            .unwrap();

        assert_eq!(hex::encode(packet), "53050add6816012e");
    }

    #[test]
    fn decodes_retail_inbound_whisper() {
        let protocol = protocol();
        let packet =
            hex::decode("5305414a682e0000000019034e656c736f6e54657374393123313435380100686f6c61")
                .unwrap();
        let payload = decode_incoming(&protocol, &packet);
        assert_eq!(
            payload,
            Payload::ChatWhisper(crate::native::model::ChatWhisper {
                peer: crate::native::model::ToonFullName {
                    region: 1,
                    program_id: u32::from_be_bytes(*b"BSAp"),
                    realm: 1,
                    name: "NelsonTest91#1458".to_owned(),
                },
                body: "hola".to_owned(),
            })
        );
    }

    #[test]
    fn typed_schema_whisper_matches_the_hand_decoder() {
        use crate::bsn::FromBsn as _;
        let protocol = protocol();
        let packet =
            hex::decode("5305414a682e0000000019034e656c736f6e54657374393123313435380100686f6c61")
                .unwrap();
        let type_id = protocol
            .incoming_type(CHAT_SLOT, CHAT_WHISPER_RECV_COMMAND)
            .unwrap();
        let decoded = protocol.codec().decode(type_id, &packet, None, 11).unwrap();
        let typed =
            crate::native::schema::chat::ClientChatWhisperRecv::from_bsn(&decoded.value).unwrap();

        assert_eq!(typed.sender.region, 1);
        assert_eq!(typed.sender.program_id.0, u32::from_be_bytes(*b"BSAp"));
        assert_eq!(typed.sender.realm, 1);
        assert_eq!(typed.sender.name, "NelsonTest91#1458");
        assert_eq!(typed.body, "hola");
    }

    #[test]
    fn decodes_party_datagram_connection_update() {
        use crate::bsn::FromBsn as _;
        let protocol = protocol();
        let packet = hex::decode(
            "4d0d0100014c32000000010c6e656c736f6e746573742334303400000000000000e00000000000000001000000000102",
        )
        .unwrap();
        let Payload::Reflected(value) = decode_incoming(&protocol, &packet) else {
            panic!("expected a reflected datagram connection update");
        };
        let typed =
            crate::native::schema::chat::ClientChatDatagramConnectionUpdate::from_bsn(&value)
                .unwrap();

        let crate::native::schema::defines::ClientDefinesPlayerTarget::ToonName(target) =
            typed.target
        else {
            panic!("expected the datagram target to be a toon name");
        };
        assert_eq!(target.region, 1);
        assert_eq!(target.program_id.0, 0x5332);
        assert_eq!(target.realm, 1);
        assert_eq!(target.name, "nelsontest#404");
        assert_eq!(typed.info.address_port.address.0, [0, 0, 224, 0]);
        assert_eq!(typed.info.bound_address_port.port.0, [1, 0]);
    }

    #[test]
    fn registered_invite_layout_makes_typed_and_native_decoders_agree() {
        use crate::bsn::FromBsn as _;
        let protocol = protocol();
        let packet = hex::decode("f6050002991201000000010000000006acf9060041557c550000").unwrap();

        let Payload::ClubInviteAction(action) = decode_incoming(&protocol, &packet) else {
            panic!("expected an invite action");
        };
        assert_eq!(action.club_id, 535_228);

        let type_id = protocol
            .incoming_type(S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_INVITE_ACTION_COMMAND)
            .unwrap();
        let decoded = protocol.codec().decode(type_id, &packet, None, 11).unwrap();
        let typed =
            crate::native::schema::club::ClientClubInviteAction::from_bsn(&decoded.value).unwrap();
        assert_eq!(typed.action.club_id, action.club_id);
        assert_eq!(decoded.bit_count, 197);

        let encoded = protocol.codec().encode(type_id, &decoded.value).unwrap();
        assert_eq!(encoded.bit_count, 197);
        let round_trip = protocol
            .codec()
            .decode(type_id, &encoded.data, Some(encoded.bit_count), 0)
            .unwrap();
        assert_eq!(round_trip.value, decoded.value);
    }

    #[test]
    fn friend_toon_lookup_includes_the_generated_request_marker() {
        let protocol = protocol();
        let packet = protocol.friend_toons(308_451_427).unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(header.bit_count).unwrap();

        let value = protocol
            .codec
            .decode_reflected_from(&mut reader, protocol.friends_toons_request_type)
            .unwrap();
        let BsnValue::Struct(value) = value else {
            panic!("friend toon lookup is not a struct");
        };

        assert!(
            value
                .fields
                .iter()
                .any(|field| field.name.as_deref() == Some("ToonsOfFriendPacket"))
        );
        assert!(value.fields.iter().any(|field| {
            field.name.as_deref() == Some("m_accountId")
                && field.value == BsnValue::Integer(308_451_427)
        }));
    }

    #[test]
    fn presence_statistics_subscription_matches_the_retail_record() {
        let protocol = protocol();
        let packet = protocol.presence_statistics_subscribe(true).unwrap();

        assert_eq!(hex::encode(packet), "420c");
        assert_eq!(
            protocol
                .incoming_type(PRESENCE_SLOT, PRESENCE_STATISTICS_UPDATE_COMMAND)
                .unwrap(),
            protocol
                .codec
                .schema()
                .find("Battlenet::Client::Presence::StatisticsUpdate", true)
                .unwrap()[0]
                .type_id
        );
    }

    #[test]
    fn whisper_writer_does_not_use_the_stale_eight_bit_body_length() {
        let protocol = protocol();
        let body = "é".repeat(200);
        let packet = protocol
            .chat_whisper(&crate::native::model::WhisperTarget::Account(42), &body)
            .unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(header.bit_count + 3 + 32).unwrap();
        assert_eq!(reader.read(10).unwrap(), 400);
        assert_eq!(reader.read_bytes(400, true).unwrap(), body.as_bytes());
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture verifies the related identity response family"
    )]
    fn current_metadata_decodes_retail_identity_responses() {
        let protocol = protocol();
        let name = crate::native::model::ToonFullName {
            region: 1,
            program_id: fourcc("S2"),
            realm: 1,
            name: "Tagban#542".into(),
        };
        let handle = crate::native::model::ToonHandle {
            region: 1,
            program_id: fourcc("S2"),
            realm: 1,
            id: 0x1234_5678_90ab_cdef,
        };

        let resolve_type = protocol
            .incoming_type(PROFILE_SLOT, PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND)
            .unwrap();
        let marker_type = protocol
            .member_type(resolve_type, "ResolveToonNameToHandle")
            .unwrap();
        let toon_handle_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Toon::Handle")
            .unwrap();
        let handle_value = protocol
            .struct_value(
                toon_handle_type,
                vec![
                    ("m_region", BsnValue::Integer(i128::from(handle.region))),
                    ("m_programId", BsnValue::FourCc(handle.program_id)),
                    ("m_realm", BsnValue::Integer(i128::from(handle.realm))),
                    ("m_id", BsnValue::Integer(i128::from(handle.id))),
                ],
            )
            .unwrap();
        let resolve_value = protocol
            .struct_value(
                resolve_type,
                vec![
                    (
                        "ResolveToonNameToHandle",
                        protocol.struct_value(marker_type, Vec::new()).unwrap(),
                    ),
                    ("m_name", protocol.toon_full_name_value(&name).unwrap()),
                    ("m_result", BsnValue::Integer(0)),
                    ("m_handle", handle_value),
                ],
            )
            .unwrap();
        let packet = protocol
            .encode_record(
                PROFILE_RESOLVE_TOON_NAME_RESPONSE_COMMAND,
                PROFILE_SLOT,
                resolve_type,
                &resolve_value,
            )
            .unwrap();
        assert_eq!(
            decode_incoming(&protocol, &packet),
            Payload::ToonNameResolved(crate::native::model::ToonNameResolved {
                name,
                result: 0,
                handle: Some(handle),
            })
        );

        let clan_type = protocol
            .incoming_type(
                S2_MULTIPLAYER_SLOT,
                S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND,
            )
            .unwrap();
        let request_type = protocol
            .member_type(clan_type, "GetMemberClanTags")
            .unwrap();
        let request = protocol
            .struct_value(request_type, vec![("m_token", BsnValue::Integer(37))])
            .unwrap();
        let clan_value = protocol
            .struct_value(
                clan_type,
                vec![
                    ("GetMemberClanTags", request),
                    ("m_result", BsnValue::Integer(0)),
                    ("m_clubId", BsnValue::Integer(91)),
                    ("m_clubTag", BsnValue::String("BNU".into())),
                ],
            )
            .unwrap();
        let packet = protocol
            .encode_record(
                S2_MULTIPLAYER_MEMBER_CLAN_TAGS_RESPONSE_COMMAND,
                S2_MULTIPLAYER_SLOT,
                clan_type,
                &clan_value,
            )
            .unwrap();
        assert_eq!(
            decode_incoming(&protocol, &packet),
            Payload::MemberClanTag(crate::native::model::MemberClanTag {
                token: 37,
                result: 0,
                club_id: Some(91),
                tag: Some("BNU".into()),
            })
        );
    }

    #[test]
    fn generated_chat_bootstrap_packets_match_retail_vectors() {
        let protocol = protocol();
        assert_eq!(
            hex::encode(
                protocol
                    .cache_get_stream_items(1, "BNET", "ERRS", "enUS")
                    .unwrap()
            ),
            "c90300000001028000432722aa44a9292965b72aa9ffffffff01"
        );
        assert_eq!(
            hex::encode(protocol.chat_enum_conference_descriptions().unwrap()),
            "5905"
        );
        assert_eq!(hex::encode(protocol.chat_channel_list().unwrap()), "5505");
        assert_eq!(hex::encode(protocol.chat_leave(0).unwrap()), "4205");
        assert_eq!(hex::encode(protocol.chat_leave(6).unwrap()), "4235");
        assert!(protocol.chat_leave(7).is_err());
        assert_eq!(
            hex::encode(protocol.chat_invite_answer(0, true).unwrap()),
            "4505"
        );
        assert_eq!(
            hex::encode(protocol.chat_invite_answer(0, false).unwrap()),
            "4605"
        );
        assert_eq!(
            hex::encode(protocol.chat_invite_answer(6, true).unwrap()),
            "4535"
        );
        assert_eq!(
            hex::encode(protocol.chat_invite_answer(6, false).unwrap()),
            "4635"
        );
        assert!(protocol.chat_invite_answer(7, true).is_err());
        let party_online = protocol.chat_party_online(6, 91).unwrap();
        let header = decode_routing_header(&party_online, None).unwrap();
        assert_eq!(header.command_id, CHAT_STATUS_CHANGE_COMMAND);
        assert_eq!(header.service_slot, Some(CHAT_SLOT));
        let decoded = protocol
            .codec()
            .decode(
                protocol.chat_status_change_type,
                &party_online,
                None,
                header.bit_count,
            )
            .unwrap();
        use crate::bsn::FromBsn as _;
        let status =
            crate::native::schema::chat::ClientChatStatusChangeRequest::from_bsn(&decoded.value)
                .unwrap();
        assert_eq!(status.channel_index, 6);
        assert_eq!(status.member_handle, 91);
        let crate::native::schema::chat::ChatMemberStatusSingle::Party(party) =
            status.status_change
        else {
            panic!("expected party membership status");
        };
        assert!(matches!(
            party.party_status,
            crate::native::schema::chat::ChatPartyMemberStatusEnum::ONLINE
        ));
        assert!(matches!(
            party.expansion_level,
            Some(crate::native::schema::starcraft2::Starcraft2ExpansionLevelEnum::LEGACYOFTHEVOID)
        ));
        assert!(!party.captain);

        assert_eq!(
            hex::encode(
                protocol
                    .chat_join_private("Custom Room", 0x1020_3040)
                    .unwrap()
            ),
            "40050b437573746f6d20526f6f6d10203040"
        );
        assert!(protocol.chat_join_private("", 0).is_err());
        assert!(protocol.chat_join_private(&"x".repeat(32), 0).is_err());
        assert!(protocol.chat_join_private(&"🛰".repeat(31), 0).is_ok());
        assert_eq!(
            hex::encode(protocol.toon_select("hotshot#994", 1).unwrap()),
            "c51701686f7473686f74233939348d0000000001"
        );
        assert_eq!(
            hex::encode(protocol.ping(1_785_773_259_159_371).unwrap()),
            "4a890065826bcc2f740b"
        );
        let maintenance = protocol
            .transport_control_maintenance(3_261_977_005)
            .unwrap();
        assert_eq!(
            hex::encode(&maintenance[0]),
            "4d0102000000000081044151c9b9030000000003c3136e8d0d000000000400000001090001"
        );
        assert_eq!(
            hex::encode(&maintenance[1]),
            "4d010501030080057e4002020a030181044151c9b9030000000002c3136e8d0c000000000b00000001090001"
        );
    }

    #[test]
    fn toon_creation_responses_use_the_embedded_schema() {
        let protocol = protocol();
        let setup_type = protocol
            .incoming_type(TOON_SLOT, TOON_CREATE_SETUP_COMMAND)
            .unwrap();
        let setup = protocol
            .encode_record(
                TOON_CREATE_SETUP_COMMAND,
                TOON_SLOT,
                setup_type,
                &protocol.struct_value(setup_type, Vec::new()).unwrap(),
            )
            .unwrap();
        assert!(matches!(
            decode_incoming(&protocol, &setup),
            Payload::Reflected(BsnValue::Struct(_))
        ));

        assert!(
            protocol
                .incoming_type(TOON_SLOT, TOON_CREATED_COMMAND)
                .is_ok()
        );

        let failure_type = protocol
            .incoming_type(TOON_SLOT, TOON_FAILURE_COMMAND)
            .unwrap();
        let failure = protocol
            .encode_record(
                TOON_FAILURE_COMMAND,
                TOON_SLOT,
                failure_type,
                &protocol
                    .struct_value(failure_type, vec![("m_error", BsnValue::Integer(42))])
                    .unwrap(),
            )
            .unwrap();
        let Payload::Reflected(BsnValue::Struct(failure)) = decode_incoming(&protocol, &failure)
        else {
            panic!("expected reflected Toon::Failure");
        };
        assert_eq!(
            failure.get("m_error").and_then(BsnValue::as_integer),
            Some(42)
        );
    }

    #[test]
    fn profile_address_query_targets_a_friend_account() {
        let protocol = protocol();
        let packet = protocol
            .profile_address_query(0x1020_3040, 50_209_335)
            .unwrap();
        let header = decode_routing_header(&packet, None).unwrap();
        assert_eq!(header.command_id, PROFILE_ADDRESS_QUERY_COMMAND);
        assert_eq!(header.service_slot, Some(PROFILE_SLOT));

        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(11).unwrap();
        let value = protocol
            .codec
            .decode_reflected_from(&mut reader, protocol.profile_address_query_request_type)
            .unwrap();
        let root = value_struct(&value, "address query request").unwrap();
        assert_eq!(
            value_integer(required_field(root, "m_requestId").unwrap(), "request id").unwrap(),
            0x1020_3040
        );
        let (variant, account_id) = value_choice(
            required_field(root, "m_playerTarget").unwrap(),
            "player target",
        )
        .unwrap();
        assert_eq!(variant, 3);
        assert_eq!(value_integer(account_id, "account id").unwrap(), 50_209_335);
    }

    #[test]
    fn generated_profile_avatar_read_matches_retail_layout() {
        let protocol = protocol();
        let encoded = protocol
            .profile_read(
                24,
                crate::native::model::ProfileAddress {
                    label: 0xcafe_babe,
                    id: 0xdc82_c80e_0000_0000,
                },
                &[4],
            )
            .unwrap();
        assert_eq!(
            hex::encode(encoded),
            "c00600000000000003c85fd757de905901c0000000000104"
        );
    }
}
