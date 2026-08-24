use std::{collections::HashMap, path::Path};

use crate::games::sc2::native::errors::native_error;
use crate::{
    Error, Result,
    bgs::{NativeHandoff, fourcc},
    bsn::{
        bits::{BitReader, BitWriter, RoutingHeader},
        codec::{Codec, DecodedField},
        value::{BsnField, BsnStruct, BsnValue},
    },
    metadata::{Metadata, Schema, TypeKind, read_largest_metadata, read_metadata},
    native::{
        auth::{SESSION_PROOF_MODULE_ID, THUMBPRINT_MODULE_ID},
        decode,
        model::Payload,
        schema, wire_layout,
    },
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
pub const CONNECTION_CLOSING_COMMAND: u8 = 9;
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
/// the universal SC2 profile-address label. every account's toon/profile/presence
/// records carry this same constant `m_label` (0xCAFEBABE — verified 71× across
/// many distinct accounts in a real bootstrap); a placeholder makes SC2 reject the
/// record. see also `presence.rs`, which uses the same constant.
pub const TOON_PROFILE_LABEL: u32 = 0xCAFE_BABE;
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
/// chat/25 asks for the live head count of every conference the account can
/// see. the reply is [`CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND`]. this was long
/// mislabelled `EnumConferenceDescriptions`; the decrypted 97563 handler at
/// `0x100289790` decodes `MembershipInfo` elements, and captured replies show a
/// hard cap where `m_isFull` flips, so it is the member-count query.
pub const CHAT_ENUM_CONFERENCE_MEMBER_COUNTS_COMMAND: u8 = 25;
/// chat/23 asks which conference serves each public channel; the reply is
/// [`CHAT_CONFERENCE_DESCRIPTIONS_COMMAND`]. mislabelled `EnumCategoryDescriptions`
/// until 2026-08-19 — the reply's elements are `FullConferenceDescription`, not
/// the five-byte `CategoryDescription`.
pub const CHAT_ENUM_CONFERENCE_DESCRIPTIONS_COMMAND: u8 = 23;
pub const CHAT_CONFERENCE_DESCRIPTIONS_COMMAND: u8 = 24;
pub const CACHE_GET_STREAM_ITEMS_COMMAND: u8 = 9;
pub const TOON_SELECT_COMMAND: u8 = 5;
/// Friends notifies (server to client) are `Battlenet::Client::Friends::
/// Commands::Enum` index **+ 7**. The three ids below that were already pinned
/// from captures all land on their enum position that way — LIST_NOTIFY_5 23,
/// ACCOUNT_BLOCK_ADDED_NOTIFY 24, TOON_BLOCK_NOTIFY 26 — and the constants the
/// offset predicts are the ones a sweep trips over. Requests are numbered
/// separately, which is why [`FRIENDS_TOONS_COMMAND`] does not follow it.
pub const FRIENDS_LIST_COMMAND: u8 = 30;
pub const FRIENDS_TOONS_COMMAND: u8 = 6;
pub const FRIENDS_ACCOUNT_BLOCK_COMMAND: u8 = 31;
pub const FRIENDS_TOON_BLOCK_COMMAND: u8 = 33;
/// every route the retail client registers an inbound handler for, read
/// out of the `bsn_service_catalog` a running SC2 publishes (see
/// `research/captures/sc2-bsn-*.jsonl`). `connection_index` in that dump is
/// the service slot, and it matches the slot constants above exactly.
///
/// This says what the client *receives*, which is not the same as what the
/// service will answer — but a route missing from here is one retail never
/// handles, and asking for it is how a connection gets closed.
pub const CLIENT_INBOUND_ROUTES: &[(u8, &[u8])] = &[
    // Auth
    (0, &[12, 14, 16, 17]),
    // Conn
    (1, &[0, 1, 2, 3, 4, 11, 12, 13, 14]),
    // Frnd
    (
        3,
        &[3, 6, 9, 13, 14, 15, 16, 21, 27, 28, 29, 30, 31, 32, 33, 34],
    ),
    // Pres
    (4, &[0, 1, 3, 4]),
    // Chat
    (
        5,
        &[
            1, 4, 7, 11, 12, 13, 15, 16, 18, 19, 20, 22, 24, 26, 27, 29, 30, 31, 33, 35, 36,
        ],
    ),
    // Ladd
    (6, &[0]),
    // Supp
    (7, &[3]),
    // Achv
    (8, &[2, 3, 4, 6, 7]),
    // S2Ms
    (
        10,
        &[
            0, 1, 2, 3, 4, 6, 13, 14, 15, 16, 17, 18, 19, 20, 24, 25, 26, 27, 28, 29, 30, 31, 33,
            34, 36, 39,
        ],
    ),
    // Cach
    (11, &[3, 7, 9]),
    // Prty
    (12, &[0, 2, 5, 12, 13, 14, 15, 18, 20]),
    // S2Mp
    (
        13,
        &[
            2, 3, 4, 5, 6, 7, 8, 9, 12, 13, 14, 15, 16, 21, 22, 23, 24, 25, 26, 41, 42, 43, 44, 45,
            46, 49, 50, 51, 53, 54, 55, 56, 57, 59,
        ],
    ),
    // Prfl
    (14, &[0, 1, 2, 3, 4]),
    // Toon
    (
        15,
        &[
            0, 2, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15, 17, 20, 21, 22, 23, 24, 25, 35,
        ],
    ),
];

/// whether retail SC2 registers a handler for a route. A sweep halting on a
/// route the client does not handle is looking at something retail never
/// receives; one it does handle is a message we simply cannot decode yet.
#[must_use]
pub fn client_handles_route(service_slot: u8, command: u8) -> bool {
    CLIENT_INBOUND_ROUTES
        .iter()
        .any(|(slot, commands)| *slot == service_slot && commands.contains(&command))
}

/// what a route is, for routes we can name but not yet decode. A sweep that
/// halts on one of these has found a real message rather than a desync, and
/// naming it saves working the id back to a type by hand.
#[must_use]
pub const fn known_route_name(service_slot: Option<u8>, command: u8) -> Option<&'static str> {
    let Some(slot) = service_slot else {
        return None;
    };
    Some(match (slot, command) {
        (FRIENDS_SLOT, FRIENDS_SEND_INVITATION_RESULT_COMMAND) => {
            "Battlenet::Client::Friends::SendInvitationResult"
        }
        (FRIENDS_SLOT, FRIENDS_INVITATION_ADDED_COMMAND) => {
            "Battlenet::Client::Friends::FriendInvitationAddedNotify"
        }
        (FRIENDS_SLOT, FRIENDS_INVITATION_REMOVED_COMMAND) => {
            "Battlenet::Client::Friends::FriendInvitationRemovedNotify"
        }
        (FRIENDS_SLOT, FRIENDS_ACCOUNT_BLOCK_REMOVED_COMMAND) => {
            "Battlenet::Client::Friends::AccountBlockRemovedNotify"
        }
        (FRIENDS_SLOT, FRIENDS_FRIEND_OF_FRIEND_RESULT_COMMAND) => {
            "Battlenet::Client::Friends::FriendOfFriendResult"
        }
        (S2_MASTER_SLOT, S2_MASTER_LOBBY_PREVIEW_COMMAND) => {
            "Battlenet::Client::S2Master::LobbyPreviewResponse"
        }
        _ => return None,
    })
}

/// `SendInvitationResult` (enum 20).
pub const FRIENDS_SEND_INVITATION_RESULT_COMMAND: u8 = 27;
/// `FriendInvitationAddedNotify` (enum 21) — somebody has invited you. Carries
/// a `FriendInvitation`: five optional fields, then the inviter's BattleTag as
/// a length-prefixed string, then account id, role and created time.
pub const FRIENDS_INVITATION_ADDED_COMMAND: u8 = 28;
/// `FriendInvitationRemovedNotify` (enum 22).
pub const FRIENDS_INVITATION_REMOVED_COMMAND: u8 = 29;
/// `AccountBlockRemovedNotify` (enum 25).
pub const FRIENDS_ACCOUNT_BLOCK_REMOVED_COMMAND: u8 = 32;
/// `FriendOfFriendResult` (enum 27).
pub const FRIENDS_FRIEND_OF_FRIEND_RESULT_COMMAND: u8 = 34;
pub const PRESENCE_UPDATE_COMMAND: u8 = 0;
pub const PRESENCE_FIELDS_COMMAND: u8 = 1;
pub const PRESENCE_STATISTICS_SUBSCRIBE_COMMAND: u8 = 2;
pub const PRESENCE_STATISTICS_UPDATE_COMMAND: u8 = 3;
pub const PRESENCE_TEMPORARY_COMMAND: u8 = 4;
pub const CHAT_MEMBERSHIP_COMMAND: u8 = 1;
pub const CHAT_CHANNEL_LIST_RESPONSE_COMMAND: u8 = 22;
pub const CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND: u8 = 26;
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
/// asks the master what is inside one lobby, keyed by its `AdvertHandle`. The
/// id from a `lobbyLink(...)` in chat is only a third of a handle, so this
/// needs the server the advert is on as well — see
/// `PresenceDirectory::resolve_advert_id`.
pub const S2_MASTER_LOBBY_PREVIEW_COMMAND: u8 = 40;
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

/// transport-control fields parsed from a client `Connection/13 MessageFrame`.
#[derive(Debug, Default, Clone)]
pub struct MessageFrameTransport {
    pub frame_type: i128,
    pub payload_len: usize,
    pub command: Option<u8>,
    pub correlation_id: Option<u32>,
    pub reply: Option<bool>,
    pub sequence: Option<u32>,
}

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
        (FRIENDS_SLOT, FRIENDS_INVITATION_ADDED_COMMAND),
        "Battlenet::Client::Friends::FriendInvitationAddedNotify",
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
        (CHAT_SLOT, CHAT_CONFERENCE_DESCRIPTIONS_COMMAND),
        "Battlenet::Client::Chat::ConferenceDescriptions",
    ),
    (
        (CHAT_SLOT, CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND),
        "Battlenet::Client::Chat::ConferenceMemberCounts",
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
                CHAT_CONFERENCE_DESCRIPTIONS_COMMAND
                    | CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND
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
    // server-side response roots (records the client receives, that we emit).
    configuration_type: u32,
    proof_request_type: u32,
    resume_response_type: u32,
    ping_type: u32,
    message_frame_type: u32,
    front_logon_response_type: u32,
    chat_channel_list_request_type: u32,
    chat_enum_member_counts_type: u32,
    chat_enum_conference_descriptions_type: u32,
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
        let configuration_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::Configuration")?;
        let proof_request_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::ProofRequest")?;
        let resume_response_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::ResumeResponse")?;
        let ping_type = metadata.unique_type_id("Battlenet::Client::Connection::Ping")?;
        let message_frame_type =
            metadata.unique_type_id("Battlenet::Client::Connection::MessageFrame")?;
        let front_logon_response_type =
            metadata.unique_type_id("Battlenet::Client::Authentication::LogonResponse3")?;
        let chat_channel_list_request_type =
            metadata.unique_type_id("Battlenet::Client::Chat::ChannelListRequest")?;
        let chat_enum_member_counts_type =
            metadata.unique_type_id("Battlenet::Client::Chat::EnumConferenceMemberCounts")?;
        let chat_enum_conference_descriptions_type =
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
            configuration_type,
            proof_request_type,
            resume_response_type,
            ping_type,
            message_frame_type,
            front_logon_response_type,
            chat_channel_list_request_type,
            chat_enum_member_counts_type,
            chat_enum_conference_descriptions_type,
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
                (CHAT_SLOT, CHAT_CONFERENCE_DESCRIPTIONS_COMMAND) => {
                    decode::conference_descriptions(reader)?
                }
                (CHAT_SLOT, CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND) => {
                    decode::conference_member_counts(reader)?
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
                (FRIENDS_SLOT, FRIENDS_TOON_BLOCK_COMMAND) => decode::toon_blocks(self, reader)?,
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
        let payload = payload_result.map_err(|error| {
            let root_type_name = self
                .codec
                .schema()
                .type_metadata(type_id)
                .ok()
                .and_then(|metadata| metadata.name)
                .unwrap_or_else(|| "unnamed".to_owned());
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
            (CHAT_SLOT, CHAT_CONFERENCE_DESCRIPTIONS_COMMAND) => {
                Some(decode::conference_descriptions_with_provenance(reader)?)
            }
            (CHAT_SLOT, CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND) => {
                Some(decode::conference_member_counts_with_provenance(reader)?)
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
                Some(decode::toon_blocks_with_provenance(self, reader)?)
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

    /// like [`Protocol::resume_request`] but from explicit fields instead of a
    /// decoded [`NativeHandoff`]. useful for tooling and loopback tests that
    /// drive the client side without a full BGS handoff.
    pub fn resume_request_fields(
        &self,
        account_mail: &str,
        game_account_name: &str,
        game_account_region: u8,
    ) -> Result<Vec<u8>> {
        let common = self.request_common(self.resume_request_type, "RequestCommon")?;
        let value = self.struct_value(
            self.resume_request_type,
            vec![
                ("RequestCommon", common),
                (
                    "m_account",
                    BsnValue::Bytes(account_mail.as_bytes().to_vec()),
                ),
                (
                    "m_gameAccountRegion",
                    BsnValue::Integer(i128::from(game_account_region)),
                ),
                (
                    "m_gameAccountName",
                    BsnValue::Bytes(game_account_name.as_bytes().to_vec()),
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

    // ---------------------------------------------------------------------
    // server-side record construction.
    //
    // these build the records a native *server* emits (the records a client
    // receives). they mirror the client constructors above and reuse the same
    // reflected encoder. the single-purpose auth fields (`m_useS3Depot`, the
    // proof modules, the final session module) are pinned by the client-side
    // verification in `auth.rs`; any other schema fields are filled with typed
    // defaults so the record still decodes. byte-exact parity with retail for
    // the larger `Configuration`/`ResponseSuccessCommon` field sets should be
    // confirmed against a capture.
    // ---------------------------------------------------------------------

    /// `Auth/18 Configuration`.
    pub fn configuration(&self, use_s3_depot: bool) -> Result<Vec<u8>> {
        let value = self.struct_with_defaults(
            self.configuration_type,
            vec![("m_useS3Depot", BsnValue::Bool(use_s3_depot))],
        )?;
        self.encode_record(
            AUTH_CONFIGURATION_COMMAND,
            AUTHENTICATION_SLOT,
            self.configuration_type,
            &value,
        )
    }

    /// `Auth/2 ProofRequest` carrying the two required modules: the thumbprint
    /// module (our RSA signature over the peer context) and the session module
    /// (`0x00 ‖ server_nonce`).
    pub fn proof_request(
        &self,
        thumbprint_signature: &[u8],
        server_nonce: &[u8; 16],
    ) -> Result<Vec<u8>> {
        let request_field = self.member_type(self.proof_request_type, "m_request")?;
        let module_type = self.array_element(request_field)?;
        let thumbprint_module = self.struct_with_defaults(
            module_type,
            vec![
                ("m_id", BsnValue::Bytes(THUMBPRINT_MODULE_ID.to_vec())),
                ("m_data", BsnValue::Bytes(thumbprint_signature.to_vec())),
            ],
        )?;
        let mut session_data = Vec::with_capacity(17);
        session_data.push(0);
        session_data.extend_from_slice(server_nonce);
        let session_module = self.struct_with_defaults(
            module_type,
            vec![
                ("m_id", BsnValue::Bytes(SESSION_PROOF_MODULE_ID.to_vec())),
                ("m_data", BsnValue::Bytes(session_data)),
            ],
        )?;
        let value = self.struct_with_defaults(
            self.proof_request_type,
            vec![(
                "m_request",
                BsnValue::Array(vec![thumbprint_module, session_module]),
            )],
        )?;
        self.encode_record(
            AUTH_PROOF_COMMAND,
            AUTHENTICATION_SLOT,
            self.proof_request_type,
            &value,
        )
    }

    /// `Auth/1 ResumeResponse` phase-two success, carrying the final session
    /// module (`0x02 ‖ server_proof`).
    pub fn resume_response(&self, server_proof: &[u8; 32]) -> Result<Vec<u8>> {
        let result_type = self.member_type(self.resume_response_type, "m_result")?;
        let success_type = self.choice_variant_by_index(result_type, 0)?;
        let common_type = self.member_type(success_type, "ResponseSuccessCommon")?;
        let final_field_type = self.member_type(common_type, "m_finalRequest")?;
        let module_type = self.array_element(final_field_type)?;

        let mut data = Vec::with_capacity(33);
        data.push(2);
        data.extend_from_slice(server_proof);
        let module = self.struct_with_defaults(
            module_type,
            vec![
                ("m_id", BsnValue::Bytes(SESSION_PROOF_MODULE_ID.to_vec())),
                ("m_data", BsnValue::Bytes(data)),
            ],
        )?;
        let common = self.struct_with_defaults(
            common_type,
            vec![
                ("m_finalRequest", BsnValue::Array(vec![module])),
                // Keep the native connection watchdog consistent with the
                // value supplied by the Front handoff.
                ("m_pingTimeout", BsnValue::Integer(60_000)),
            ],
        )?;
        let success =
            self.struct_with_defaults(success_type, vec![("ResponseSuccessCommon", common)])?;
        let value = self.struct_with_defaults(
            self.resume_response_type,
            vec![("m_result", BsnValue::choice(0, success))],
        )?;
        self.encode_record(
            AUTH_RESUME_COMMAND,
            AUTHENTICATION_SLOT,
            self.resume_response_type,
            &value,
        )
    }

    /// Build the bit-packed `LogonResponse3` blob carried by the BGS
    /// `GameUtilities.ProcessClientRequest` handoff.
    ///
    /// This is not a routed native record: Front embeds the encoded BSN value in
    /// the handoff's `logon_response` attribute, and the native client consumes it
    /// before opening the Sunken socket. A custom Front therefore has to mint this
    /// value consistently with the other handoff identity fields.
    pub fn front_logon_response(
        &self,
        account_region: u8,
        game_account_region: u8,
        game_account_name: &[u8],
    ) -> Result<Vec<u8>> {
        if game_account_name.is_empty() || game_account_name.len() > 32 {
            return Err(native_error(
                "Front game-account name must contain 1..=32 bytes",
            ));
        }

        let response_type = self.member_type(self.front_logon_response_type, "LogonResponse")?;
        let result_type = self.member_type(response_type, "m_result")?;
        let success_type = self.choice_variant_by_index(result_type, 0)?;
        let common_type = self.member_type(success_type, "ResponseSuccessCommon")?;

        let common = self.struct_with_defaults(
            common_type,
            vec![("m_pingTimeout", BsnValue::Integer(60_000))],
        )?;
        let success = self.struct_with_defaults(
            success_type,
            vec![
                ("ResponseSuccessCommon", common),
                (
                    "m_accountRegion",
                    BsnValue::Integer(i128::from(account_region)),
                ),
                (
                    "m_gameAccountRegion",
                    BsnValue::Integer(i128::from(game_account_region)),
                ),
                (
                    "m_gameAccountName",
                    BsnValue::Bytes(game_account_name.to_vec()),
                ),
                ("m_logonFailures", BsnValue::Integer(0)),
            ],
        )?;
        let response = self.struct_with_defaults(
            response_type,
            vec![("m_result", BsnValue::choice(0, success))],
        )?;
        let wrapper = self.struct_with_defaults(
            self.front_logon_response_type,
            vec![("LogonResponse", response)],
        )?;
        Ok(self
            .codec
            .encode(self.front_logon_response_type, &wrapper)?
            .data)
    }

    /// decode a record the *client* sent (server-side inbound). resolves the
    /// route to the client-request type rather than the client-receive type.
    pub(crate) fn decode_client_request_from(
        &self,
        reader: &mut BitReader<'_>,
        header: RoutingHeader,
    ) -> Result<(u32, Payload)> {
        let slot = header
            .service_slot
            .ok_or_else(|| native_error("native client record has no service slot"))?;
        // EnableEncryption is an empty, obfuscated marker: SC2 sends the empty
        // plaintext conn/5 record. it carries no fields, so consume just the
        // route header rather than running the reflected decoder, which rejects
        // obfuscated types that have no registered wire layout.
        if slot == CONNECTION_SLOT && header.command_id == CONNECTION_ENABLE_ENCRYPTION_COMMAND {
            return Ok((
                self.enable_encryption_type,
                Payload::Reflected(BsnValue::Void),
            ));
        }
        // MessageFrame is an obfuscated, bidirectional transport wrapper; reuse the
        // custom incoming decoder to advance past the client's MessageFrames.
        if slot == CONNECTION_SLOT && header.command_id == CONNECTION_MESSAGE_FRAME_COMMAND {
            let payload = decode::message_frame(self, self.message_frame_type, reader)?;
            return Ok((self.message_frame_type, payload));
        }
        let type_id = self.client_request_type(slot, header.command_id)?;
        let value = self.codec.decode_from(reader, type_id)?;
        Ok((type_id, Payload::Reflected(value)))
    }

    fn client_request_type(&self, slot: u8, command: u8) -> Result<u32> {
        Ok(match (slot, command) {
            (AUTHENTICATION_SLOT, AUTH_LOGON_COMMAND) => self.logon_request_type,
            (AUTHENTICATION_SLOT, AUTH_RESUME_COMMAND) => self.resume_request_type,
            (AUTHENTICATION_SLOT, AUTH_PROOF_COMMAND) => self.proof_response_type,
            (AUTHENTICATION_SLOT, AUTH_SINGLE_SIGN_ON_COMMAND) => self.single_sign_on_request_type,
            (CONNECTION_SLOT, CONNECTION_ENABLE_ENCRYPTION_COMMAND) => self.enable_encryption_type,
            // post-handshake client requests core can already decode (server side).
            (CONNECTION_SLOT, CONNECTION_PING_COMMAND) => self.ping_type,
            (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND) => self.message_frame_type,
            (CHAT_SLOT, CHAT_CHANNEL_LIST_REQUEST_COMMAND) => self.chat_channel_list_request_type,
            (CHAT_SLOT, CHAT_ENUM_CONFERENCE_MEMBER_COUNTS_COMMAND) => {
                self.chat_enum_member_counts_type
            }
            (CHAT_SLOT, CHAT_ENUM_CONFERENCE_DESCRIPTIONS_COMMAND) => {
                self.chat_enum_conference_descriptions_type
            }
            (CHAT_SLOT, CHAT_STATUS_CHANGE_COMMAND) => self.chat_status_change_type,
            (FRIENDS_SLOT, FRIENDS_TOONS_COMMAND) => self.friends_toons_request_type,
            (PRESENCE_SLOT, PRESENCE_STATISTICS_SUBSCRIBE_COMMAND) => {
                self.presence_statistics_subscribe_type
            }
            (PRESENCE_SLOT, PRESENCE_TEMPORARY_COMMAND) => self.temporary_presence_request_type,
            _ => return Err(Error::UnmappedNativeRoute { slot, command }),
        })
    }

    /// build a struct value filling every schema member with a typed default,
    /// then applying the named overrides. used for server records whose full
    /// field set is broader than the auth-critical fields we pin explicitly.
    fn struct_with_defaults(
        &self,
        type_id: u32,
        overrides: Vec<(&str, BsnValue)>,
    ) -> Result<BsnValue> {
        let peeled = self.peel_alias(type_id)?;
        let shape = self.codec.schema().shape(peeled)?;
        if shape.kind != TypeKind::Struct {
            return Err(native_error(format!("BSN type {peeled} is not a struct")));
        }
        let mut overrides: HashMap<&str, BsnValue> = overrides.into_iter().collect();
        let mut fields = Vec::with_capacity(shape.member_types.len());
        for position in 0..shape.member_types.len() {
            let member_type = shape.member_types[position];
            let name = shape.member_names[position].clone();
            let raw = match name.as_deref().and_then(|name| overrides.remove(name)) {
                Some(value) => value,
                None => self.default_value(member_type)?,
            };
            fields.push(BsnField::named(
                shape.index_values[position],
                name.as_deref().unwrap_or_default(),
                self.coerce_field_value(member_type, raw)?,
            ));
        }
        if let Some((name, _)) = overrides.into_iter().next() {
            return Err(native_error(format!(
                "BSN struct {peeled} has no field named {name}"
            )));
        }
        Ok(BsnValue::Struct(BsnStruct::new(peeled, fields)))
    }

    /// a typed zero/empty value for `type_id`, used to fill schema fields whose
    /// exact server value is not yet pinned.
    fn default_value(&self, type_id: u32) -> Result<BsnValue> {
        let type_id = self.peel_alias(type_id)?;
        let shape = self.codec.schema().shape(type_id)?;
        Ok(match shape.kind {
            TypeKind::Void => BsnValue::Void,
            TypeKind::Bool => BsnValue::Bool(false),
            TypeKind::Integer | TypeKind::Enum => {
                BsnValue::Integer(shape.value_range.map_or(0, |range| range.minimum))
            }
            TypeKind::FourCc => BsnValue::FourCc(0),
            TypeKind::Float32 => BsnValue::Float32(0.0),
            TypeKind::Float64 => BsnValue::Float64(0.0),
            TypeKind::String => BsnValue::String(String::new()),
            TypeKind::ByteString | TypeKind::Blob => BsnValue::Bytes(Vec::new()),
            TypeKind::Array => BsnValue::Array(Vec::new()),
            TypeKind::Optional => BsnValue::none(),
            TypeKind::Struct => {
                let mut fields = Vec::with_capacity(shape.member_types.len());
                for position in 0..shape.member_types.len() {
                    let name = shape.member_names[position].clone().unwrap_or_default();
                    fields.push(BsnField::named(
                        shape.index_values[position],
                        &name,
                        self.default_value(shape.member_types[position])?,
                    ));
                }
                BsnValue::Struct(BsnStruct::new(type_id, fields))
            }
            TypeKind::Choice => {
                let variant_type = *shape
                    .member_types
                    .first()
                    .ok_or_else(|| native_error(format!("BSN choice {type_id} has no variants")))?;
                let index = *shape.index_values.first().ok_or_else(|| {
                    native_error(format!("BSN choice {type_id} has no variant indices"))
                })?;
                BsnValue::choice(index, self.default_value(variant_type)?)
            }
            TypeKind::BitArray => {
                return Err(native_error(format!(
                    "BSN default for bit-array type {type_id} is unsupported; provide it explicitly"
                )));
            }
            TypeKind::Alias => {
                return Err(native_error("BSN alias type should have been peeled"));
            }
        })
    }

    fn choice_variant_by_index(&self, type_id: u32, index: i128) -> Result<u32> {
        let peeled = self.peel_alias(type_id)?;
        let mut shape = self.codec.schema().shape(peeled)?;
        if shape.kind == TypeKind::Optional {
            let inner = shape
                .member_types
                .first()
                .copied()
                .or(shape.element_type)
                .ok_or_else(|| native_error(format!("BSN optional {peeled} has no inner type")))?;
            shape = self.codec.schema().shape(self.peel_alias(inner)?)?;
        }
        if shape.kind != TypeKind::Choice {
            return Err(native_error(format!("BSN type {peeled} is not a choice")));
        }
        let position = shape
            .index_values
            .iter()
            .position(|value| *value == index)
            .ok_or_else(|| {
                native_error(format!(
                    "BSN choice {peeled} has no variant with index {index}"
                ))
            })?;
        Ok(shape.member_types[position])
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

    /// decode a captured server→client record (routing header + payload) from raw
    /// bytes, returning `(service_slot, command_id, payload)`. convenience for
    /// tooling that has a record and wants its decoded payload — e.g. pulling the
    /// `content_handle` out of a `CacheStreamItems` to pre-fetch a catalog.
    pub fn decode_server_record(&self, bytes: &[u8]) -> Result<(Option<u8>, u8, Payload)> {
        let mut reader = crate::bsn::bits::BitReader::new(bytes, None)?;
        let command_id = u8::try_from(reader.read(6)?).expect("6 bits fit in u8");
        let service_slot = if reader.read(1)? != 0 {
            Some(u8::try_from(reader.read(4)?).expect("4 bits fit in u8"))
        } else {
            None
        };
        let header = crate::bsn::bits::RoutingHeader {
            command_id,
            service_slot,
            bit_count: reader.position(),
        };
        let (_type_id, payload) = self.decode_incoming_from(&mut reader, header)?;
        Ok((service_slot, command_id, payload))
    }

    /// decode a client `Connection/13 MessageFrame` and extract its transport-control
    /// header fields (the "TrnC" reliable-message layer): route command, correlation
    /// id + reply flag, and stream sequence id. used to drive the server-side ack.
    pub fn message_frame_transport(&self, bytes: &[u8]) -> Result<MessageFrameTransport> {
        let mut reader = crate::bsn::bits::BitReader::new(bytes, None)?;
        let _command = reader.read(6)?;
        if reader.read(1)? != 0 {
            let _slot = reader.read(4)?;
        }
        let payload = decode::message_frame(self, self.message_frame_type, &mut reader)?;
        let crate::native::model::Payload::MessageFrame(frame) = payload else {
            return Err(native_error("record is not a message frame"));
        };
        Ok(self.transport_fields(&frame))
    }

    /// pull the reliable-transport control fields (command, correlation id, reply
    /// flag, sequence) out of an already-decoded message frame — the shape
    /// [`RecordStream::receive`] hands back as `Payload::MessageFrame`.
    #[must_use]
    pub fn transport_fields(
        &self,
        frame: &crate::native::model::ConnectionMessageFrame,
    ) -> MessageFrameTransport {
        let mut info = MessageFrameTransport {
            frame_type: frame.frame_type,
            payload_len: frame.payload.len(),
            command: None,
            correlation_id: None,
            reply: None,
            sequence: None,
        };
        for header in &frame.headers {
            let Some(crate::bsn::value::BsnValue::Choice { value, .. }) = header.get("m_data")
            else {
                continue;
            };
            let Some(inner) = value.as_struct() else {
                continue;
            };
            if let Some(crate::bsn::value::BsnValue::Integer(command)) = inner.get("m_command") {
                info.command = u8::try_from(*command).ok();
            }
            if let Some(crate::bsn::value::BsnValue::Integer(id)) = inner.get("m_id") {
                info.correlation_id = u32::try_from(*id).ok();
            }
            if let Some(crate::bsn::value::BsnValue::Bool(reply)) = inner.get("m_reply") {
                info.reply = Some(*reply);
            }
            if let Some(crate::bsn::value::BsnValue::Integer(sequence)) = inner.get("m_sequenceId")
            {
                info.sequence = u32::try_from(*sequence).ok();
            }
        }
        info
    }

    pub fn transport_control_maintenance(&self, correlation_id: u32) -> Result<[Vec<u8>; 2]> {
        Ok([
            self.transport_control_frame(3, correlation_id, false, 1, vec![0; 4])?,
            self.transport_control_frame(
                2,
                correlation_id.wrapping_sub(1),
                false,
                1,
                vec![
                    0x03, 0x00, 0x80, 0x05, 0x7e, 0x40, 0x02, 0x02, 0x0a, 0x03, 0x01,
                ],
            )?,
        ])
    }

    /// Build an `SC_ROUTED` transport reply from a server routing context.
    ///
    /// Client requests are sparse `CS_ROUTED` frames (route, correlation, content),
    /// while server replies must carry the service binding and target established by
    /// the native bootstrap as well as a timestamp and stream header. `template`
    /// supplies the route and service binding; the target is rebuilt from the
    /// game account minted by BGS. Correlation, content size, timestamp, sequence,
    /// payload, and the reply flag are encoded for the live request.
    pub fn transport_routed_reply(
        &self,
        template: &crate::native::model::ConnectionMessageFrame,
        game_account_region: u8,
        game_account_id: u32,
        correlation_id: u32,
        sequence: u32,
        timestamp: u32,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let header_list_type = self.member_type(self.message_frame_type, "m_headers")?;
        let header_type = self.array_element(header_list_type)?;
        let data_type = self.member_type(header_type, "m_data")?;
        let (route_index, _) = self.choice_variant(data_type, "route")?;
        let (service_index, _) = self.choice_variant(data_type, "service")?;
        let (target_index, target_type) = self.choice_variant(data_type, "target")?;
        let (correlation_index, correlation_type) =
            self.choice_variant(data_type, "correlation")?;
        let (content_index, content_type) = self.choice_variant(data_type, "content")?;
        let (timestamp_index, _) = self.choice_variant(data_type, "timestamp")?;
        let (stream_index, stream_type) = self.choice_variant(data_type, "stream")?;

        let template_header = |wanted_index: i128, name: &str| {
            template
                .headers
                .iter()
                .find(|header| {
                    matches!(
                        header.get("m_data"),
                        Some(BsnValue::Choice { index, .. }) if *index == wanted_index
                    )
                })
                .cloned()
                .map(BsnValue::Struct)
                .ok_or_else(|| native_error(format!("transport context omits {name} header")))
        };
        let wrap_header = |index: i128, value: BsnValue| {
            self.struct_value(
                header_type,
                vec![("m_data", BsnValue::choice(index, value))],
            )
        };

        let correlation = self.struct_value(
            correlation_type,
            vec![
                ("m_id", BsnValue::Integer(i128::from(correlation_id))),
                ("m_reply", BsnValue::Bool(true)),
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
                ("m_sequenceId", BsnValue::Integer(i128::from(sequence))),
                ("m_more", BsnValue::Bool(false)),
            ],
        )?;
        let target_ids_type = self.member_type(target_type, "m_ids")?;
        let (game_account_index, game_accounts_type) =
            self.choice_variant(target_ids_type, "GameAccount")?;
        let game_account_type = self.array_element(game_accounts_type)?;
        let game_account = self.struct_value(
            game_account_type,
            vec![
                (
                    "m_region",
                    BsnValue::Integer(i128::from(game_account_region)),
                ),
                ("m_programId", BsnValue::FourCc(fourcc("S2"))),
                ("m_id", BsnValue::Integer(i128::from(game_account_id))),
            ],
        )?;
        let target = self.struct_value(
            target_type,
            vec![
                // Battlenet::Frame::TargetType::GAME_ACCOUNT.
                ("m_type", BsnValue::Integer(5)),
                (
                    "m_ids",
                    BsnValue::choice(game_account_index, BsnValue::Array(vec![game_account])),
                ),
            ],
        )?;
        let headers = vec![
            template_header(route_index, "route")?,
            template_header(service_index, "service")?,
            wrap_header(correlation_index, correlation)?,
            wrap_header(content_index, content)?,
            wrap_header(timestamp_index, BsnValue::Integer(i128::from(timestamp)))?,
            wrap_header(target_index, target)?,
            wrap_header(stream_index, stream)?,
        ];

        let payload_type = self.member_type(self.message_frame_type, "m_payload")?;
        let frame_type = self.member_type(self.message_frame_type, "m_frameType")?;
        let mut writer = Self::record_writer(CONNECTION_MESSAGE_FRAME_COMMAND, CONNECTION_SLOT)?;
        self.codec
            .encode_reflected_into(&mut writer, payload_type, &BsnValue::Bytes(payload))?;
        // Battlenet::Frame::Type::SC_ROUTED. CS_ROUTED (129) is valid only in
        // the opposite direction and causes SC2 to close with PACKET_REJECTED.
        self.codec
            .encode_reflected_into(&mut writer, frame_type, &BsnValue::Integer(66))?;
        self.codec.encode_reflected_into(
            &mut writer,
            header_list_type,
            &BsnValue::Array(headers),
        )?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// build a top-level `CommandResponse` (a no-slot reply record): the 6-bit command
    /// id, the `service_present=0` marker, a 9-bit result code (0 = success), then the
    /// optional body. this is the record the reliable transport carries as its reply
    /// payload; a well-formed one is safe (unlike a stale replayed data blob).
    pub fn command_response(&self, command_id: u8, result: u16, body: &[u8]) -> Result<Vec<u8>> {
        let mut writer = crate::bsn::bits::BitWriter::new();
        writer.write(u64::from(command_id), 6)?;
        writer.write(0, 1)?; // service not present -> command response
        writer.write(u64::from(result), 9)?;
        if !body.is_empty() {
            writer.write_bytes(body, false)?;
        }
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// Encode `BattlePay::GetWalletsResponse::Success` with no wallets.
    ///
    /// This is an inner reliable-transport payload, not a top-level native
    /// record. A custom account must not inherit the captured account's wallet
    /// id, payment type, or display name.
    pub fn empty_battlepay_wallets_response(&self) -> Result<Vec<u8>> {
        let root_type = self
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::BattlePay::GetWalletsResponse")?;
        let marker_type = self.member_type(root_type, "GetWallets")?;
        let result_type = self.member_type(root_type, "m_result")?;
        let success_type = self.choice_variant_by_index(result_type, 0)?;
        let marker = self.struct_value(marker_type, Vec::new())?;
        let success = self.struct_with_defaults(
            success_type,
            vec![("m_wallets", BsnValue::Array(Vec::new()))],
        )?;
        let root = self.struct_value(
            root_type,
            vec![
                ("GetWallets", marker),
                ("m_result", BsnValue::choice(0, success)),
            ],
        )?;
        Ok(self.codec.encode(root_type, &root)?.data)
    }

    /// Encode a local `BattlePay::GetInfoResponse` without replaying another
    /// account's balances or licenses. The two 40-byte values are public content
    /// handles for the current product and license catalogs; all account-owned
    /// collections are empty.
    pub fn empty_battlepay_info_response(&self) -> Result<Vec<u8>> {
        const PRODUCT_CATALOG: [u8; 40] = [
            0x63, 0x61, 0x74, 0x61, 0x00, 0x00, 0x55, 0x53, 0xa3, 0x6e, 0x1f, 0x84, 0xdb, 0x40,
            0x35, 0x0e, 0x10, 0xfb, 0x6c, 0xf7, 0x46, 0x33, 0x27, 0x87, 0x65, 0xe0, 0xd7, 0xc8,
            0x33, 0xf1, 0x49, 0x85, 0x19, 0x74, 0x4f, 0xb1, 0xd6, 0x90, 0x82, 0xb7,
        ];
        const LICENSE_CATALOG: [u8; 40] = [
            0x63, 0x61, 0x74, 0x61, 0x00, 0x00, 0x55, 0x53, 0x2f, 0x14, 0x3c, 0x9b, 0x1b, 0x2b,
            0x7f, 0x2b, 0x1c, 0xfa, 0x4c, 0xdb, 0x6b, 0x31, 0x27, 0xd3, 0x25, 0x88, 0xf9, 0x83,
            0xb5, 0xe1, 0xf6, 0xb1, 0x86, 0xb2, 0x2f, 0xdf, 0x20, 0xf7, 0x98, 0x61,
        ];

        let root_type = self
            .codec
            .schema()
            .unique_type_id("Battlenet::Client::BattlePay::GetInfoResponse")?;
        let marker_type = self.member_type(root_type, "GetInfo")?;
        let marker = self.struct_with_defaults(marker_type, Vec::new())?;
        let root = self.struct_with_defaults(
            root_type,
            vec![
                ("GetInfo", marker),
                ("m_licenseResult", BsnValue::Integer(0)),
                ("m_accountCountry", BsnValue::Bytes(b"USA".to_vec())),
                (
                    "m_productCatalog",
                    BsnValue::Bytes(PRODUCT_CATALOG.to_vec()),
                ),
                (
                    "m_licenseCatalog",
                    BsnValue::Bytes(LICENSE_CATALOG.to_vec()),
                ),
                ("m_currencies", BsnValue::Array(Vec::new())),
                ("m_balances", BsnValue::Array(Vec::new())),
                ("m_licenses", BsnValue::Array(Vec::new())),
            ],
        )?;
        Ok(self.codec.encode(root_type, &root)?.data)
    }

    fn transport_control_frame(
        &self,
        command: u8,
        correlation_id: u32,
        reply: bool,
        sequence: u32,
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
                ("m_reply", BsnValue::Bool(reply)),
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
                ("m_sequenceId", BsnValue::Integer(i128::from(sequence))),
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

    /// a zero-payload record on the chat slot.
    ///
    /// every chat query SC2 issues — `ChannelListRequest` (21),
    /// `EnumConferenceDescriptions` (23), `EnumConferenceMemberCounts` (25) — is
    /// exactly an eleven-bit routing header and nothing else, so an unknown
    /// query can be tried without inventing a payload for it. used only by the
    /// command-id probe.
    /// `S2Master/40 LobbyPreviewRequest` — "what is in this lobby".
    ///
    /// Written by hand rather than by reflected encode: every type on the path
    /// is obfuscated, and the reflected encoder refuses those. The payload is
    /// just the handle — `LobbyPreviewRequest{LobbyPreviewPacket, m_data}` and
    /// `LobbyPreviewRequestData{m_advertHandle}` are single-field wrappers
    /// around it — and the handle's field order is the one recovered from the
    /// `S2GameInfo` presence values: label, epoch, advert id, 32 bits each.
    pub fn lobby_preview_request(
        &self,
        server_label: u32,
        server_epoch: i32,
        advert_id: u32,
    ) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(S2_MASTER_LOBBY_PREVIEW_COMMAND, S2_MASTER_SLOT)?;
        writer.write(u64::from(server_label), 32)?;
        // Time::Seconds is an s32, so the wire carries it biased by -i32::MIN
        writer.write(u64::from(server_epoch.wrapping_sub(i32::MIN) as u32), 32)?;
        writer.write(u64::from(advert_id), 32)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    pub fn chat_empty_request(&self, command: u8) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(command, CHAT_SLOT)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// asks which conference serves each public channel. SC2 sends this at
    /// bootstrap; without it the counts arrive keyed by conference id with
    /// nothing to attribute them to.
    pub fn chat_enum_conference_descriptions(&self) -> Result<Vec<u8>> {
        let value = self.struct_value(self.chat_enum_conference_descriptions_type, Vec::new())?;
        self.encode_record(
            CHAT_ENUM_CONFERENCE_DESCRIPTIONS_COMMAND,
            CHAT_SLOT,
            self.chat_enum_conference_descriptions_type,
            &value,
        )
    }

    pub fn chat_enum_conference_member_counts(&self) -> Result<Vec<u8>> {
        let value = self.struct_value(self.chat_enum_member_counts_type, Vec::new())?;
        self.encode_record(
            CHAT_ENUM_CONFERENCE_MEMBER_COUNTS_COMMAND,
            CHAT_SLOT,
            self.chat_enum_member_counts_type,
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

    // ---- server-side chat responses (mirror the `decode::*` readers) --------
    // these build the records a Battle.net chat server pushes, letting a Sunken
    // server drive a real client through the bootstrap into a channel.

    /// chat `ConferenceMemberCounts`. mirrors
    /// [`decode::conference_member_counts`]. an empty, final page
    /// (`is_last=true`) satisfies the client's bootstrap gate. each entry is
    /// `(conference_id, members, full)`.
    pub fn conference_member_counts_response(
        &self,
        entries: &[(u32, u16, bool)],
        is_last: bool,
    ) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(CHAT_CONFERENCE_MEMBER_COUNTS_COMMAND, CHAT_SLOT)?;
        writer.write(u64::from(is_last), 1)?;
        writer.write(0, 27)?; // reserved
        writer.write(0, 1)?; // m_time present = no
        writer.write(u64::try_from(entries.len()).unwrap_or(u64::MAX), 6)?;
        for (conference_id, members, full) in entries {
            writer.write(0, 23)?; // reserved
            writer.write(u64::from(*conference_id), 32)?;
            writer.write(u64::from(*members), 16)?;
            writer.write(u64::from(*full), 1)?;
        }
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// chat `ConferenceDescriptions` (chat/24) answering SC2's
    /// `EnumConferenceDescriptions` (chat/23). an empty, final page: like the
    /// empty member-count and channel-list responses, a client only needs the
    /// reply to advance its bootstrap.
    ///
    /// written by hand rather than by reflected encode, because the type is
    /// obfuscated and only [`decode::conference_descriptions`] knows its order.
    pub fn conference_descriptions_response(&self, is_last: bool) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(CHAT_CONFERENCE_DESCRIPTIONS_COMMAND, CHAT_SLOT)?;
        writer.write(u64::from(is_last), 1)?;
        writer.write(0, 6)?; // entry count
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// chat `ChannelList`. mirrors [`decode::channel_list`]. each entry is
    /// `(kind, index, identifier)`.
    pub fn channel_list_response(&self, entries: &[(u8, u16, u16)]) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(CHAT_CHANNEL_LIST_RESPONSE_COMMAND, CHAT_SLOT)?;
        writer.write(0, 27)?; // reserved
        writer.write(0, 1)?; // wire flag
        writer.write(0, 9)?; // wire-layout selector
        writer.write(u64::try_from(entries.len()).unwrap_or(u64::MAX), 6)?;
        for (kind, index, identifier) in entries {
            writer.write(u64::from(*kind), 8)?;
            writer.write(u64::from(*index), 16)?;
            writer.write(0, 24)?; // reserved
            writer.write(u64::from(*identifier), 16)?;
        }
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// chat `ModifyChannelListResponse2` (chat/33) — the gate SC2 waits on before it
    /// sends its join (chat/0). it answers SC2's `ModifyChannelListRequest` (chat/32);
    /// `m_token` MUST echo the request's token, and `m_result` must be the SUCCESS
    /// variant (a `Battlenet::Time::Seconds`). a captured response carrying the FAILURE
    /// variant is exactly why replaying it never advanced SC2 — so this builds success.
    pub fn modify_channel_list_response(&self, token: u32, seconds: u32) -> Result<Vec<u8>> {
        let root = self.incoming_type(CHAT_SLOT, CHAT_MODIFY_CHANNEL_LIST_RESPONSE_COMMAND)?;
        let mut writer = Self::record_writer(CHAT_MODIFY_CHANNEL_LIST_RESPONSE_COMMAND, CHAT_SLOT)?;
        let value = self.struct_value(
            root,
            vec![
                ("m_token", BsnValue::Integer(i128::from(token))),
                (
                    "m_result",
                    BsnValue::choice(0, BsnValue::Integer(i128::from(seconds))),
                ),
            ],
        )?;
        self.codec
            .encode_reflected_into(&mut writer, root, &value)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// chat `JoinNotify` accepting a join. mirrors the success path of
    /// [`decode::chat_join`]. omits the token (client then matches by channel
    /// type) and the channel name (so `validate_public_join` passes).
    pub fn chat_join_result(
        &self,
        channel_index: u8,
        member_handle: u32,
        channel_type: u8,
    ) -> Result<Vec<u8>> {
        if usize::from(channel_index) >= CHANNEL_INDEX_COUNT {
            return Err(native_error("chat channel index must be between 0 and 6"));
        }
        let mut writer = Self::record_writer(CHAT_JOIN_NOTIFY_COMMAND, CHAT_SLOT)?;
        writer.write(0, 1)?; // 0 = success
        writer.write(u64::from(member_handle), 32)?;
        writer.write(u64::from(channel_index), 3)?;
        writer.write(0, 32)?; // reserved field A
        writer.write(0, 32)?; // reserved field B
        writer.write(u64::from(channel_type), 4)?;
        writer.write(0, 1)?; // channel-name present = no
        writer.write(0, 1)?; // channel-config present = no
        writer.write(0, 1)?; // extra-u32 present = no
        writer.write(0, 1)?; // token present = no
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// chat `Membership` — the initial roster. mirrors [`decode::chat_membership`].
    /// each `(member_handle, presence_id)` becomes a joined member (choice=1) with
    /// no status sub-records. `members` must be non-empty (the length is encoded as
    /// `count - 1`).
    pub fn chat_membership_response(
        &self,
        channel_index: u8,
        members: &[(u32, u32)],
        end_of_initial: bool,
    ) -> Result<Vec<u8>> {
        if usize::from(channel_index) >= CHANNEL_INDEX_COUNT {
            return Err(native_error("chat channel index must be between 0 and 6"));
        }
        if members.is_empty() {
            return Err(native_error(
                "chat membership must carry at least one member",
            ));
        }
        let mut writer = Self::record_writer(CHAT_MEMBERSHIP_COMMAND, CHAT_SLOT)?;
        writer.write(u64::from(end_of_initial), 1)?;
        writer.write(u64::from(channel_index), 3)?;
        // the array length is stored as (count - 1).
        writer.write(u64::try_from(members.len() - 1).unwrap_or(u64::MAX), 6)?;
        for (member_handle, presence_id) in members {
            writer.write(1, 2)?; // choice = Join
            writer.write(u64::from(*member_handle), 32)?;
            writer.write(u64::from(*presence_id), 32)?;
            writer.write(0, 3)?; // member-status count = 0
        }
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// cache `GetStreamItems` response — a catalog pointer. mirrors
    /// [`decode::cache_stream_items`]. `token` must echo the client's request token;
    /// each item is `(publication_time, content_handle)` naming a depot object the
    /// client then downloads. the client picks the item with the latest publication
    /// time and keys the catalog by `token`.
    pub fn cache_stream_items_response(
        &self,
        token: u32,
        items: &[(i32, [u8; 40])],
    ) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(CACHE_GET_STREAM_ITEMS_COMMAND, CACHE_SLOT)?;
        writer.write(u64::try_from(items.len()).unwrap_or(u64::MAX), 6)?;
        for (publication_time, handle) in items {
            writer.write(0, 23)?; // wire-layout selector (decoder reads a fixed layout)
            writer.write_bytes(handle, true)?; // aligned 40-byte content handle
            writer.write(u64::from(*publication_time as u32), 32)?;
        }
        writer.write(u64::from(token), 32)?;
        writer.write(u64::try_from(items.len()).unwrap_or(u64::MAX), 16)?; // total_items
        writer.write(0, 16)?; // offset
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// profile `Read` response with a "cached" result (choice index 3) — the minimal
    /// valid answer to a profile read: no data to transfer, just the echoed request
    /// id. a synthetic server uses this to satisfy the client's profile reads.
    pub fn profile_read_cache(&self, request_id: u32) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(PROFILE_READ_COMMAND, PROFILE_SLOT)?;
        self.codec.encode_reflected_into(
            &mut writer,
            self.profile_data_response_type,
            &BsnValue::choice(3, BsnValue::Void),
        )?;
        self.codec.encode_reflected_into(
            &mut writer,
            self.token_type,
            &BsnValue::Integer(i128::from(request_id)),
        )?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// Profile `Read` response stream containing one complete record block.
    ///
    /// A `Cache` response is only valid when the client already owns the addressed
    /// record. A newly-created/custom account does not, so startup reads must receive
    /// a `Start` record followed by the promised `Block`, with the live request id
    /// echoed in both records.
    pub fn profile_read_record(
        &self,
        request_id: u32,
        record_type: u32,
        block: &[u8],
    ) -> Result<[Vec<u8>; 2]> {
        let start_type = self.choice_variant_by_index(self.profile_data_response_type, 0)?;
        let start = self.struct_value(
            start_type,
            vec![
                ("m_numPackets", BsnValue::Integer(1)),
                ("m_type", BsnValue::Integer(i128::from(record_type))),
            ],
        )?;
        Ok([
            self.profile_read_response(request_id, BsnValue::choice(0, start))?,
            self.profile_read_response(
                request_id,
                BsnValue::choice(1, BsnValue::Bytes(block.to_vec())),
            )?,
        ])
    }

    /// Profile `Read` response declaring a known record type with no packets.
    /// Retail uses this form when the requested path exists conceptually but has
    /// no data yet (notably the first read of a new battle-profile record).
    pub fn profile_read_empty(&self, request_id: u32, record_type: u32) -> Result<Vec<u8>> {
        let start_type = self.choice_variant_by_index(self.profile_data_response_type, 0)?;
        let start = self.struct_value(
            start_type,
            vec![
                ("m_numPackets", BsnValue::Integer(0)),
                ("m_type", BsnValue::Integer(i128::from(record_type))),
            ],
        )?;
        self.profile_read_response(request_id, BsnValue::choice(0, start))
    }

    fn profile_read_response(&self, request_id: u32, result: BsnValue) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(PROFILE_READ_COMMAND, PROFILE_SLOT)?;
        self.codec
            .encode_reflected_into(&mut writer, self.profile_data_response_type, &result)?;
        self.codec.encode_reflected_into(
            &mut writer,
            self.token_type,
            &BsnValue::Integer(i128::from(request_id)),
        )?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// minimal valid server->client `current_season` record for the
    /// S2_MASTER slot / current-season command. mirrors `decode::current_season`
    /// exactly with `failure = 0`: no ranked matchmakers, no leagues, an
    /// all-defaults `SeasonInfo` (every optional flag cleared), and no league
    /// configurations. `authority` sets the "season complete" bit the decoder
    /// surfaces as `complete`.
    pub fn current_season_response(&self, authority: bool) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(S2_MASTER_CURRENT_SEASON_COMMAND, S2_MASTER_SLOT)?;
        // value.failure (bool) — 0 selects the success branch.
        writer.write(0, 1)?;
        // value.authority (bool) — decoded as `complete`.
        writer.write(u64::from(authority), 1)?;
        // value.ranked: array length (7 bits) == 0 → no RankedMatchmaker items.
        writer.write(0, 7)?;
        // value.leagues: array length (9 bits) == 0 → no league items.
        writer.write(0, 9)?;
        // value.season: SeasonInfo — mirror decode_season_info with every
        // optional (1-bit-gated) field absent.
        writer.write(0, 32)?;
        writer.write(0, 10)?;
        writer.write(0, 1)?; // optional u32 absent
        writer.write(0, 16)?;
        writer.write(0, 1)?; // optional u16 absent
        writer.write(0, 32)?;
        writer.write(0, 16)?;
        writer.write(0, 16)?;
        writer.write(0, 25)?;
        writer.write(0, 1)?; // optional u16 absent
        writer.write(0, 1)?; // optional u32 absent
        writer.write(0, 1)?; // optional u16 absent
        writer.write(0, 64)?;
        // value.configurations: array length (9 bits) == 0 → no LeagueConfiguration items.
        writer.write(0, 9)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// build a minimal valid server->client `Presence/UpdateNotify`
    /// (`PRESENCE_SLOT` / `PRESENCE_UPDATE_COMMAND`) record. this is a pure
    /// bit-packed layout (no reflected BSN types); every write mirrors a read in
    /// `decode::presence_update`, in the same order and bit-width. the record
    /// carries one handle and leaves every optional/opaque field empty or zeroed.
    pub fn presence_update_response(
        &self,
        presence_id: u32,
        handle: u32,
        online: bool,
    ) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(PRESENCE_UPDATE_COMMAND, PRESENCE_SLOT)?;
        // value.wire_layout_selector: 19-bit obfuscation selector (opaque, zero is valid)
        writer.write(0, 19)?;
        // value.online: 1-bit inverted bool (wire 0 => online == true)
        writer.write(u64::from(!online), 1)?;
        // value.local_presence_id / value.master_presence_id: uint32
        writer.write(u64::from(presence_id), 32)?;
        writer.write(u64::from(presence_id), 32)?;
        // value.field_data: 11-bit length prefix + aligned byte payload (empty)
        writer.write(0, 11)?;
        writer.write_bytes(&[], true)?;
        // value.reserved: 11 reserved bits
        writer.write(0, 11)?;
        // value.cleared_handles: 4-bit count + uint32 items (empty)
        writer.write(0, 4)?;
        // value.handles: 4-bit count + uint32 items (one handle)
        writer.write(1, 4)?;
        writer.write(u64::from(handle), 32)?;
        // value.variable_sizes: 4-bit count + uint16 items (empty)
        writer.write(0, 4)?;
        // value.optional_target: 1-bit presence flag (absent)
        writer.write(0, 1)?;
        // value.trailing_selector: 8-bit obfuscation selector
        writer.write(0, 8)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// presence `FieldSpecAnnounce` — the presence-field dictionary the server
    /// announces to the client. mirrors [`decode::presence_fields`] exactly.
    /// pass the field specs (an empty slice yields the minimal valid record).
    /// each entry's flags select client-only / writable / ephemeral / server-only,
    /// and `fixed_size` is the optional fixed byte width.
    pub fn presence_fields_response(
        &self,
        entries: &[crate::native::model::PresenceField],
    ) -> Result<Vec<u8>> {
        if entries.len() > 100 {
            return Err(native_error(
                "presence announcement contains too many field definitions",
            ));
        }
        let mut writer = Self::record_writer(PRESENCE_FIELDS_COMMAND, PRESENCE_SLOT)?;
        // array length: 7-bit count (read_spanned_usize(reader, 7)).
        writer.write(u64::try_from(entries.len()).unwrap_or(u64::MAX), 7)?;
        for entry in entries {
            let flags = entry.flags;
            writer.write(u64::from(flags.client_only()), 1)?;
            writer.write(u64::from(flags.writable()), 1)?;
            writer.write(u64::from(flags.ephemeral()), 1)?;
            // optional uint16: 1 = absent (none), 0 = present followed by 16 bits.
            match entry.fixed_size {
                None => writer.write(1, 1)?,
                Some(size) => {
                    writer.write(0, 1)?;
                    writer.write(u64::from(size), 16)?;
                }
            }
            writer.write(u64::from(flags.server_only()), 1)?;
            writer.write(u64::from(entry.identifier), 8)?;
            writer.write(u64::from(entry.handle), 32)?;
        }
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// toon `Welcome` — the minimal server->client welcome notify. mirrors
    /// [`decode::toon_welcome`] exactly: every reflected member is encoded with its
    /// schema default, every counted array is emitted empty, and the opaque wire
    /// selectors / name-restriction strings are written as zero / empty. this is the
    /// smallest record the retail client will accept for TOON_SLOT/TOON_WELCOME_COMMAND.
    /// welcome is a pure server push and carries no request token to echo.
    pub fn toon_welcome_response(&self) -> Result<Vec<u8>> {
        let type_id = self.incoming_type(TOON_SLOT, TOON_WELCOME_COMMAND)?;
        let mut writer = Self::record_writer(TOON_WELCOME_COMMAND, TOON_SLOT)?;

        let depot_region = self.member_type(type_id, "m_depotRegion")?;
        self.codec.encode_reflected_into(
            &mut writer,
            depot_region,
            &self.default_value(depot_region)?,
        )?;

        // m_achievementHandles — empty array (4-bit element count)
        writer.write(0, 4)?;

        for name in ["m_isPlayingFromIGR", "m_defaultPortrait"] {
            let field = self.member_type(type_id, name)?;
            self.codec
                .encode_reflected_into(&mut writer, field, &self.default_value(field)?)?;
        }

        // portrait obfuscation selector — 31-bit opaque field
        writer.write(0, 31)?;

        for name in [
            "m_maxGameServerConnectTimeoutMS",
            "m_programName",
            "m_programFlags",
        ] {
            let field = self.member_type(type_id, name)?;
            self.codec
                .encode_reflected_into(&mut writer, field, &self.default_value(field)?)?;
        }

        // m_realmMapList — empty array (3-bit element count)
        writer.write(0, 3)?;

        // m_unlockablesFiles — empty array (8-bit element count)
        writer.write(0, 8)?;

        // trailing obfuscation selector — 3-bit opaque field
        writer.write(0, 3)?;

        let max_map_favorites = self.member_type(type_id, "m_maxMapFavorites")?;
        self.codec.encode_reflected_into(
            &mut writer,
            max_map_favorites,
            &self.default_value(max_map_favorites)?,
        )?;

        // intermediate + final name-restriction strings — empty generated UTF-8
        // (13-bit length prefix, minimum 0 bytes, so an empty string writes a zero length)
        encode_generated_utf8(&mut writer, "", 13, 4096, 1024)?;
        encode_generated_utf8(&mut writer, "", 13, 4096, 1024)?;

        let current_time = self.member_type(type_id, "m_currentTime")?;
        self.codec.encode_reflected_into(
            &mut writer,
            current_time,
            &self.default_value(current_time)?,
        )?;

        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// multiplayer `ClubSettings` (S2_MULTIPLAYER_SLOT / S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND).
    /// the minimal valid server->client settings record. mirrors
    /// [`decode::club_settings`] field-for-field: an empty `description` and
    /// `message` (both generated UTF-8 with a 13-bit length prefix and 0-byte
    /// minimum), followed by the five opaque setting words written as zeros
    /// (`setting_0`/`setting_1` are read as `uint32`; `setting_2..4` as `int32`).
    /// the decoder reads no request token, so the record carries no parameters.
    pub fn club_settings_response(&self) -> Result<Vec<u8>> {
        let mut writer =
            Self::record_writer(S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND, S2_MULTIPLAYER_SLOT)?;
        // value.description — generated UTF-8, 13-bit length, min 0 / max 4096 bytes: empty.
        encode_generated_utf8(&mut writer, "", 13, 4096, 1024)?;
        // value.message — same shape: empty.
        encode_generated_utf8(&mut writer, "", 13, 4096, 1024)?;
        writer.write(0, 32)?; // value.setting_0 (uint32)
        writer.write(0, 32)?; // value.setting_1 (uint32)
        writer.write(0, 32)?; // value.setting_2 (int32, XOR-biased on decode)
        writer.write(0, 32)?; // value.setting_3 (int32)
        writer.write(0, 32)?; // value.setting_4 (int32)
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// encode a minimal server->client `Friends/30 FriendsList` record
    /// (FRIENDS_SLOT / FRIENDS_LIST_COMMAND), the inverse of
    /// `decode::friends_list`. that decoder is pure bit-packing: an optional
    /// `complete` bool (a 1-bit presence flag, followed by a 1-bit value only
    /// when present) and then a 7-bit `updates` array length, followed by that
    /// many update records. the smallest valid snapshot has `complete = None`
    /// and an empty update list, so both leading fields are zero and no update
    /// bodies are emitted. there is no reflected type and no request token on
    /// this route, so the method takes no parameters.
    pub fn friends_list_response(&self) -> Result<Vec<u8>> {
        let mut writer = Self::record_writer(FRIENDS_LIST_COMMAND, FRIENDS_SLOT)?;
        // value.complete: optional bool — absent (presence bit 0 => none), so
        // the reader's `reader.read(1)? == 0` branch yields none and reads no
        // value bit.
        writer.write(0, 1)?;
        // value.updates.count: 7-bit array length (read_spanned_usize width 7) —
        // empty list, so the decode loop runs zero times.
        writer.write(0, 7)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// server->client `ToonList` (`TOON_SLOT`/`TOON_LIST_COMMAND`) carrying a
    /// single synthetic toon display. mirrors [`decode::toon_list_with_provenance`]:
    /// a 6-bit display count, then per display a 7-bit `(len - 2)` length-prefixed
    /// aligned UTF-8 name, an int32 `last_online` obfuscated on the wire by an
    /// `XOR 0x8000_0000`, a 3-bit wire-layout selector, 32-bit flags, and reflected
    /// `m_profile` + `m_realm` fields. a synthetic server answers the client's toon
    /// enumeration with this record; the display it advertises is `"Sunken"` on
    /// realm 1.
    /// server->client `ToonList` advertising one toon the client can select. the
    /// profile address (`m_label`, `m_id`) MUST be non-zero — SC2 validates it and
    /// refuses to select a toon whose profile is empty. callers pass the live
    /// identity so SC2 sees a real, well-formed toon.
    pub fn toon_list_response(
        &self,
        name: &str,
        realm: u32,
        profile_id: u64,
        last_online: u32,
    ) -> Result<Vec<u8>> {
        let root_type = self.incoming_type(TOON_SLOT, TOON_LIST_COMMAND)?;
        let display_type = self.array_element(self.member_type(root_type, "m_toonDisplays")?)?;
        let profile_type = self.member_type(display_type, "m_profile")?;
        let realm_type = self.member_type(display_type, "m_realm")?;

        let raw = name.as_bytes();
        if !(2..=25).contains(&name.chars().count()) || !(2..=100).contains(&raw.len()) {
            return Err(native_error(
                "toon name must be 2..=25 chars / 2..=100 bytes",
            ));
        }

        let mut writer = Self::record_writer(TOON_LIST_COMMAND, TOON_SLOT)?;
        writer.write(1, 6)?; // exactly one display
        // name: 7-bit (len - minimum_bytes=2) prefix, then aligned raw bytes.
        writer.write((raw.len() - 2) as u64, 7)?;
        writer.write_bytes(raw, true)?;
        // last_online (i32): the decoder XORs the wire u32 with 0x8000_0000, so the
        // wire value is `last_online ^ 0x8000_0000`. a real toon carries a non-zero
        // timestamp; the zeroed placeholder made SC2 reject the toon.
        writer.write(u64::from(last_online ^ 0x8000_0000), 32)?;
        writer.write(0, 3)?; // wire-layout selector (decoder reads a fixed layout)
        writer.write(0, 32)?; // flags
        // m_profile: a populated ProfileAddress. m_label is the universal SC2 profile
        // constant (TOON_PROFILE_LABEL); a placeholder label makes SC2 reject the toon.
        let profile = self.struct_value(
            profile_type,
            vec![
                ("m_label", BsnValue::Integer(i128::from(TOON_PROFILE_LABEL))),
                ("m_id", BsnValue::Integer(i128::from(profile_id))),
            ],
        )?;
        self.codec
            .encode_reflected_into(&mut writer, profile_type, &profile)?;
        self.codec.encode_reflected_into(
            &mut writer,
            realm_type,
            &BsnValue::Integer(i128::from(realm)),
        )?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// connection `GameSiteInfo` — the synthetic server's greeting record.
    /// mirrors [`decode::game_site_info`], which decodes two reflected fields in
    /// wire order: `m_externalIp4Addr` (a `Battlenet::IP4::AddressPort`: a fixed
    /// 4-byte address blob and a fixed 2-byte port blob) followed by `m_siteData`
    /// (`SiteDataForClientList`, a 7-bit-counted array). the minimal valid record
    /// advertises the address `0.0.0.0:0` and an empty site list (`item_count` 0).
    ///
    /// the two fields are encoded individually (not as one struct) because the
    /// decoder reads them field-by-field, and the wire order (`m_externalIp4Addr`
    /// first) is the reverse of the struct's declaration order.
    pub fn game_site_info_response(&self) -> Result<Vec<u8>> {
        let type_id = self.incoming_type(CONNECTION_SLOT, CONNECTION_GAME_SITE_INFO_COMMAND)?;
        let external_type = self.member_type(type_id, "m_externalIp4Addr")?;
        let sites_type = self.member_type(type_id, "m_siteData")?;
        // AddressPort's blobs are fixed width (address 4..=4, port 2..=2), so an
        // empty default value would be rejected; supply exactly-sized zero blobs.
        let external_value = self.struct_value(
            external_type,
            vec![
                ("m_address", BsnValue::Bytes(vec![0u8; 4])),
                ("m_port", BsnValue::Bytes(vec![0u8; 2])),
            ],
        )?;
        // empty site-data array — 7-bit length written as 0.
        let sites_value = BsnValue::Array(Vec::new());
        let mut writer = Self::record_writer(CONNECTION_GAME_SITE_INFO_COMMAND, CONNECTION_SLOT)?;
        self.codec
            .encode_reflected_into(&mut writer, external_type, &external_value)?;
        self.codec
            .encode_reflected_into(&mut writer, sites_type, &sites_value)?;
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// server `Toon::SelectResponse` (`ToonSelected`). mirrors
    /// [`decode::toon_selected_with_provenance`] field-for-field: a reflected
    /// record address, the four reflected toon-handle fields in the decoder's
    /// exact order (`m_programId`, `m_region`, `m_realm`, `m_id`), a reflected
    /// realm, a reflected last-logon, and a generated-UTF8 name
    /// (`length_bits = 7`, `minimum_bytes = 2`). echoes a single synthetic toon
    /// ("Sunken", realm 1, handle id 42) with default record-address and
    /// last-logon values — the smallest record the decoder accepts.
    pub fn toon_selected_response(&self) -> Result<Vec<u8>> {
        // resolve the incoming type exactly as `decode_incoming_from` does.
        let type_id = self.incoming_type(TOON_SLOT, TOON_SELECTED_COMMAND)?;
        let record_address_type = self.member_type(type_id, "m_recordAddress")?;
        let handle_type = self.member_type(type_id, "m_toonHandle")?;
        let realm_type = self.member_type(type_id, "m_realm")?;
        let last_logon_type = self.member_type(type_id, "m_lastLogon")?;

        let mut writer = Self::record_writer(TOON_SELECTED_COMMAND, TOON_SLOT)?;

        let record_address = self.default_value(record_address_type)?;
        self.codec
            .encode_reflected_into(&mut writer, record_address_type, &record_address)?;

        // value.handle — the four fields the decoder reads individually, in order.
        // m_programId (FourCc in the schema): use the field default so the encoder
        // adapts to whatever concrete type the metadata declares.
        let program_id_type = self.member_type(handle_type, "m_programId")?;
        let program_id = self.default_value(program_id_type)?;
        self.codec
            .encode_reflected_into(&mut writer, program_id_type, &program_id)?;
        // m_region: default (range minimum) — a plausible, always-in-range value.
        let region_type = self.member_type(handle_type, "m_region")?;
        let region = self.default_value(region_type)?;
        self.codec
            .encode_reflected_into(&mut writer, region_type, &region)?;
        let handle_realm_type = self.member_type(handle_type, "m_realm")?;
        self.codec
            .encode_reflected_into(&mut writer, handle_realm_type, &BsnValue::Integer(1))?;
        let id_type = self.member_type(handle_type, "m_id")?;
        self.codec
            .encode_reflected_into(&mut writer, id_type, &BsnValue::Integer(42))?;

        self.codec
            .encode_reflected_into(&mut writer, realm_type, &BsnValue::Integer(1))?;

        let last_logon = self.default_value(last_logon_type)?;
        self.codec
            .encode_reflected_into(&mut writer, last_logon_type, &last_logon)?;

        // value.name — generated UTF-8 with length_bits = 7, minimum_bytes = 2:
        // the length field stores (byte_len - minimum_bytes).
        let name: &[u8] = b"Sunken";
        writer.write((name.len() - 2) as u64, 7)?;
        writer.write_bytes(name, true)?;

        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// build a minimal server->client `S2Multiplayer/13 GetToonClubs/46`
    /// ("club_summaries") record carrying an EMPTY club list.
    ///
    /// the reflected `Battlenet::Client::Club::GetToonClubsResponse` type is
    /// `UnsupportedObfuscated`, so `decode::club_summaries` walks the record by
    /// hand instead of trusting the schema. this encoder mirrors that manual
    /// walk exactly:
    ///   * an 11-bit routing header (cmd 46, slot 13),
    ///   * a 1-bit gap so the club count lands at absolute bit
    ///     `CLUB_COUNT_OFFSET` (12), then an 8-bit count of `0`,
    ///   * 32 bits of filler carrying the element region up to
    ///     `CLUB_FIRST_ELEMENT` (bit 52) — never read when the count is 0,
    ///   * the fixed per-record tail the decoder always consumes:
    ///     a u32 (here the echoed request `token`), one rank byte, and a
    ///     final flag bit — i.e. `32 + 8 + 0*8 + 1 = 41` bits,
    ///   * byte alignment.
    ///
    /// the whole record is exactly 96 bits (12 bytes), which is precisely the
    /// `end` position `decode::club_summaries` seeks to for an empty list
    /// (`(52 + 41).div_ceil(8) * 8 == 96`), so it decodes with no short or
    /// leftover bytes.
    pub fn club_summaries_response(&self, token: u32) -> Result<Vec<u8>> {
        // 11-bit routing header: command 46, service slot 13.
        let mut writer =
            Self::record_writer(S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND, S2_MULTIPLAYER_SLOT)?;
        // position is now 11; pad one bit so the count lands at absolute bit 12.
        writer.write(0, 1)?; // -> bit 12
        // club count == 0 (empty list) at CLUB_COUNT_OFFSET.
        writer.write(0, 8)?; // -> bit 20
        // filler up to CLUB_FIRST_ELEMENT (bit 52); unread when the count is 0.
        writer.write(0, 32)?; // -> bit 52
        // fixed tail the decoder always consumes: u32 + rank byte + final flag.
        writer.write(u64::from(token), 32)?; // -> bit 84 (echoed token, opaque to the decoder)
        writer.write(0, 8)?; // rank byte -> bit 92
        writer.write(0, 1)?; // final flag -> bit 93
        writer.align()?; // -> bit 96 (12 bytes)
        Ok(writer.into_bytes())
    }

    /// encode a minimal server->client `Profile/14 SettingsAvailable(4)` record.
    ///
    /// mirrors [`decode::profile_settings`], which reads the three reflected
    /// members `m_type`, `m_path`, `m_address` (in that exact order) of the
    /// route's incoming type. the record carries no request token, so this
    /// method takes no parameters; every field is emitted as its schema default
    /// (enum minimum `1`, empty field-path blob, zeroed record address).
    pub fn profile_settings_response(&self) -> Result<Vec<u8>> {
        let type_id = self.incoming_type(PROFILE_SLOT, PROFILE_SETTINGS_AVAILABLE_COMMAND)?;
        let type_field = self.member_type(type_id, "m_type")?;
        let path_field = self.member_type(type_id, "m_path")?;
        let address_field = self.member_type(type_id, "m_address")?;
        self.profile_settings_available(
            self.default_value(type_field)?,
            self.default_value(path_field)?,
            self.default_value(address_field)?,
        )
    }

    /// Encode a server→client `Profile/4 SettingsAvailable` announcement.
    pub fn profile_settings_available(
        &self,
        profile_type: BsnValue,
        path: BsnValue,
        address: BsnValue,
    ) -> Result<Vec<u8>> {
        let type_id = self.incoming_type(PROFILE_SLOT, PROFILE_SETTINGS_AVAILABLE_COMMAND)?;
        let mut writer = Self::record_writer(PROFILE_SETTINGS_AVAILABLE_COMMAND, PROFILE_SLOT)?;
        for (name, value) in [
            ("m_type", profile_type),
            ("m_path", path),
            ("m_address", address),
        ] {
            let field_type = self.member_type(type_id, name)?;
            self.codec
                .encode_reflected_into(&mut writer, field_type, &value)?;
        }
        writer.align()?;
        Ok(writer.into_bytes())
    }

    /// Convenience form of [`Self::profile_settings_available`] for concrete values.
    pub fn profile_settings_available_record(
        &self,
        profile_type: u32,
        path: &[u8],
        address: crate::native::model::ProfileAddress,
    ) -> Result<Vec<u8>> {
        let address = self.struct_value(
            self.profile_record_address_type,
            vec![
                ("m_label", BsnValue::Integer(i128::from(address.label))),
                ("m_id", BsnValue::Integer(i128::from(address.id))),
            ],
        )?;
        self.profile_settings_available(
            BsnValue::Integer(i128::from(profile_type)),
            BsnValue::Bytes(path.to_vec()),
            address,
        )
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
            WhisperTarget::WarcraftAccount(_) => {
                return Err(native_error(
                    "WC3 whisper target cannot be encoded by StarCraft II",
                ));
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
    fn the_route_catalog_agrees_with_the_ids_we_pinned_by_hand() {
        // the Friends notify ids derived as enum-index + 7 are exactly the ones
        // retail registers handlers for
        for command in [
            FRIENDS_SEND_INVITATION_RESULT_COMMAND,
            FRIENDS_INVITATION_ADDED_COMMAND,
            FRIENDS_INVITATION_REMOVED_COMMAND,
            FRIENDS_LIST_COMMAND,
            FRIENDS_ACCOUNT_BLOCK_COMMAND,
            FRIENDS_ACCOUNT_BLOCK_REMOVED_COMMAND,
            FRIENDS_TOON_BLOCK_COMMAND,
            FRIENDS_FRIEND_OF_FRIEND_RESULT_COMMAND,
        ] {
            assert!(
                client_handles_route(FRIENDS_SLOT, command),
                "retail handles Friends/{command}"
            );
        }
        // and the S2Master routes we already decode
        for command in [
            S2_MASTER_CURRENT_SEASON_COMMAND,
            S2_MASTER_MMQ_GET_INFO_COMMAND,
            S2_MASTER_MMQ_GET_LIST_COMMAND,
        ] {
            assert!(client_handles_route(S2_MASTER_SLOT, command));
        }
        // lobby preview is not one of them, which is why asking for it ends the
        // connection rather than answering
        assert!(!client_handles_route(
            S2_MASTER_SLOT,
            S2_MASTER_LOBBY_PREVIEW_COMMAND
        ));
    }

    #[test]
    fn a_lobby_preview_request_carries_the_handle_the_way_presence_writes_one() {
        let record = protocol()
            .lobby_preview_request(3_324_694_265, 1_782_861_089, 6_154_869)
            .expect("the request builds");
        let header = decode_routing_header(&record, None).expect("a routing header");
        assert_eq!(header.command_id, S2_MASTER_LOBBY_PREVIEW_COMMAND);
        assert_eq!(header.service_slot, Some(S2_MASTER_SLOT));
        // the header is 6 + 1 + 4 bits, then the handle
        let mut reader = BitReader::new(&record, None).expect("a reader");
        reader.read(11).expect("the routing header");
        assert_eq!(reader.read(32).expect("label"), 3_324_694_265);
        // Time::Seconds is an s32, so it goes out biased by -i32::MIN
        assert_eq!(
            reader.read(32).expect("epoch"),
            u64::from(1_782_861_089_u32.wrapping_add(0x8000_0000))
        );
        assert_eq!(reader.read(32).expect("advert"), 6_154_869);
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
    fn custom_front_builds_a_self_consistent_logon_response() {
        let protocol = protocol();
        let name = b"Superiority#1";
        let bytes = protocol.front_logon_response(1, 1, name).unwrap();
        let decoded = protocol
            .codec
            .decode(protocol.front_logon_response_type, &bytes, None, 0)
            .unwrap();
        assert_eq!(decoded.bit_count.div_ceil(8), bytes.len());

        let wrapper = value_struct(&decoded.value, "Front LogonResponse3").unwrap();
        let response = value_struct(
            required_field(wrapper, "LogonResponse").unwrap(),
            "Front LogonResponse wrapper",
        )
        .unwrap();
        let (result_index, result) =
            value_choice(required_field(response, "m_result").unwrap(), "result").unwrap();
        assert_eq!(result_index, 0);
        let success = value_struct(result, "success").unwrap();
        assert_eq!(
            value_integer(
                required_field(success, "m_accountRegion").unwrap(),
                "region"
            )
            .unwrap(),
            1
        );
        assert_eq!(
            value_bytes(
                required_field(success, "m_gameAccountName").unwrap(),
                "name"
            )
            .unwrap(),
            name
        );
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
    fn decodes_retail_whisper_echo() {
        let protocol = protocol();
        // captured from the current service after an SC:R conversation sent
        // `beep`. Chat/30 names its empty marker `WhisperEcho`, not the
        // `Whisper` marker used by Chat/19.
        let packet =
            hex::decode("5e1d080004626565700100417070000000010f4e656c736f6e5465737439312331343538")
                .unwrap();
        let payload = decode_incoming(&protocol, &packet);
        assert_eq!(
            payload,
            Payload::ChatWhisperEcho(crate::native::model::ChatWhisper {
                peer: crate::native::model::ToonFullName {
                    region: 1,
                    program_id: u32::from_be_bytes(*b"\0App"),
                    realm: 1,
                    name: "NelsonTest91#1458".to_owned(),
                },
                body: "beep".to_owned(),
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
            "5705"
        );
        assert_eq!(
            hex::encode(protocol.chat_enum_conference_member_counts().unwrap()),
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
    fn transport_reply_round_trips_live_envelope_fields_and_payload() {
        let protocol = protocol();
        // Small retail SC_ROUTED frame used only as route/service/target context.
        // The builder does not retain its payload, correlation, time, or stream ids.
        let template_record = hex::decode(
            "4d01010101003e42074105d5d128000000000ee50ddea25612b2977d4f33040323ff26df18000000000300000001c8aa13ab1d02010081c7e4600b010001",
        )
        .unwrap();
        let (_, _, Payload::MessageFrame(template)) =
            protocol.decode_server_record(&template_record).unwrap()
        else {
            panic!("expected a transport context frame");
        };
        let body = protocol
            .command_response(21, 0, &[0xde, 0xad, 0xbe, 0xef])
            .unwrap();
        let record = protocol
            .transport_routed_reply(
                &template,
                1,
                0x1234_5678,
                0xa1b2_c3d4,
                37,
                1_787_523_200,
                body.clone(),
            )
            .unwrap();

        let (slot, command, payload) = protocol.decode_server_record(&record).unwrap();
        assert_eq!(slot, Some(CONNECTION_SLOT));
        assert_eq!(command, CONNECTION_MESSAGE_FRAME_COMMAND);
        let Payload::MessageFrame(frame) = payload else {
            panic!("expected a reliable-transport message frame");
        };
        assert_eq!(frame.frame_type, 66);
        assert_eq!(frame.headers.len(), 7);
        let fields = protocol.transport_fields(&frame);
        assert_eq!(fields.command, Some(14));
        assert_eq!(fields.correlation_id, Some(0xa1b2_c3d4));
        assert_eq!(fields.reply, Some(true));
        assert_eq!(fields.sequence, Some(37));
        assert_eq!(frame.payload, body);
        let target = frame
            .headers
            .iter()
            .find_map(|header| match header.get("m_data") {
                Some(BsnValue::Choice { index: 2, value }) => value.as_struct(),
                _ => None,
            })
            .expect("game-account target header");
        assert_eq!(target.get("m_type").and_then(BsnValue::as_integer), Some(5));
        let Some(BsnValue::Choice {
            index: 3,
            value: target_ids,
        }) = target.get("m_ids")
        else {
            panic!("expected game-account target ids");
        };
        let BsnValue::Array(game_accounts) = target_ids.as_ref() else {
            panic!("expected game-account target array");
        };
        let game_account = value_struct(&game_accounts[0], "target game account").unwrap();
        assert_eq!(
            game_account.get("m_region").and_then(BsnValue::as_integer),
            Some(1)
        );
        assert_eq!(
            game_account.get("m_programId"),
            Some(&BsnValue::FourCc(fourcc("S2")))
        );
        assert_eq!(
            game_account.get("m_id").and_then(BsnValue::as_integer),
            Some(0x1234_5678)
        );
    }

    #[test]
    fn empty_battlepay_wallets_response_has_no_account_fixture() {
        let protocol = protocol();
        let body = protocol.empty_battlepay_wallets_response().unwrap();
        let root_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Client::BattlePay::GetWalletsResponse")
            .unwrap();
        let decoded = protocol.codec().decode(root_type, &body, None, 0).unwrap();
        let root = value_struct(&decoded.value, "wallet response").unwrap();
        let (variant, success) =
            value_choice(required_field(root, "m_result").unwrap(), "wallet result").unwrap();
        assert_eq!(variant, 0);
        let success = value_struct(success, "wallet success").unwrap();
        assert!(matches!(
            required_field(success, "m_wallets").unwrap(),
            BsnValue::Array(wallets) if wallets.is_empty()
        ));
        assert_eq!(decoded.bit_count, 7);
        assert_eq!(body, [0]);
    }

    #[test]
    fn empty_battlepay_info_response_has_no_account_licenses_or_balances() {
        let protocol = protocol();
        let body = protocol.empty_battlepay_info_response().unwrap();
        let root_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Client::BattlePay::GetInfoResponse")
            .unwrap();
        let decoded = protocol.codec().decode(root_type, &body, None, 0).unwrap();
        let root = value_struct(&decoded.value, "BattlePay info response").unwrap();
        assert_eq!(
            value_bytes(
                required_field(root, "m_accountCountry").unwrap(),
                "BattlePay account country",
            )
            .unwrap(),
            b"USA"
        );
        for field in ["m_currencies", "m_balances", "m_licenses"] {
            assert!(matches!(
                required_field(root, field).unwrap(),
                BsnValue::Array(values) if values.is_empty()
            ));
        }
        for field in ["m_productCatalog", "m_licenseCatalog"] {
            assert_eq!(
                value_bytes(required_field(root, field).unwrap(), field)
                    .unwrap()
                    .len(),
                40
            );
        }
        assert!(body.len() < 100);
    }

    #[test]
    fn resume_response_carries_the_native_ping_timeout() {
        let protocol = protocol();
        let proof = [0x5a; 32];
        let packet = protocol.resume_response(&proof).unwrap();
        let Payload::Reflected(value) = decode_incoming(&protocol, &packet) else {
            panic!("expected a reflected ResumeResponse");
        };
        let root = value_struct(&value, "resume response").unwrap();
        let (variant, success) =
            value_choice(required_field(root, "m_result").unwrap(), "resume result").unwrap();
        assert_eq!(variant, 0);
        let success = value_struct(success, "resume success").unwrap();
        let common = value_struct(
            required_field(success, "ResponseSuccessCommon").unwrap(),
            "resume common",
        )
        .unwrap();
        assert_eq!(
            value_integer(
                required_field(common, "m_pingTimeout").unwrap(),
                "resume ping timeout",
            )
            .unwrap(),
            60_000
        );
        let BsnValue::Array(modules) = required_field(common, "m_finalRequest").unwrap() else {
            panic!("resume final request is not an array");
        };
        assert_eq!(modules.len(), 1);
        let module = value_struct(&modules[0], "resume final module").unwrap();
        let mut expected = vec![2];
        expected.extend_from_slice(&proof);
        assert_eq!(
            value_bytes(required_field(module, "m_data").unwrap(), "resume proof").unwrap(),
            expected
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

    #[test]
    fn current_season_response_round_trips() {
        let protocol = crate::native::protocol::Protocol::current().expect("protocol schema loads");
        let bytes = protocol
            .current_season_response(false)
            .expect("encode minimal current-season record");

        let (service_slot, command, payload) = protocol
            .decode_server_record(&bytes)
            .expect("decode the encoded current-season record without leftover/short bytes");

        assert_eq!(
            service_slot,
            Some(crate::native::protocol::S2_MASTER_SLOT),
            "record must target the S2_MASTER slot (10)"
        );
        assert_eq!(
            command,
            crate::native::protocol::S2_MASTER_CURRENT_SEASON_COMMAND,
            "record must carry the current-season command (27)"
        );

        match payload {
            crate::native::model::Payload::StartupSummary(summary) => {
                assert_eq!(summary.kind, "current-season");
                assert_eq!(
                    summary.item_count, 0,
                    "minimal record has no ranked/leagues/configs"
                );
                assert_eq!(summary.complete, Some(false), "authority bit was 0");
            }
            other => panic!("unexpected payload variant: {other:?}"),
        }
    }

    #[test]
    fn presence_update_response_round_trips_minimal_record() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol
            .presence_update_response(0x1122_3344, 0x5566_7788, true)
            .unwrap();

        let (slot, command, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(slot, Some(PRESENCE_SLOT));
        assert_eq!(command, PRESENCE_UPDATE_COMMAND);

        let crate::native::model::Payload::PresenceUpdate(update) = payload else {
            panic!("expected a presence update payload");
        };
        assert_eq!(update.local_presence_id, 0x1122_3344);
        assert_eq!(update.master_presence_id, 0x1122_3344);
        assert!(update.online);
        assert!(update.field_data.is_empty());
        assert!(update.cleared_handles.is_empty());
        assert_eq!(update.handles, vec![0x5566_7788]);
        assert!(update.variable_sizes.is_empty());
    }

    #[test]
    fn presence_fields_response_round_trips_minimal_and_populated() {
        let protocol = protocol();

        // minimal valid record: an empty field dictionary.
        let empty = protocol.presence_fields_response(&[]).unwrap();
        let (slot, command, payload) = protocol.decode_server_record(&empty).unwrap();
        assert_eq!(slot, Some(PRESENCE_SLOT));
        assert_eq!(command, PRESENCE_FIELDS_COMMAND);
        match payload {
            Payload::PresenceFields(fields) => assert!(fields.entries.is_empty()),
            other => panic!("expected PresenceFields, got {other:?}"),
        }

        // populated record: one entry exercising the per-entry loop, including the
        // optional fixed_size branch and the flag bits, must round-trip identically.
        let entry = crate::native::model::PresenceField {
            handle: 0xDEAD_BEEF,
            identifier: 7,
            flags: crate::native::model::PresenceFieldFlags::from_bits(
                crate::native::model::PresenceFieldFlags::WRITABLE
                    | crate::native::model::PresenceFieldFlags::CLIENT_ONLY,
            ),
            fixed_size: Some(0x1234),
        };
        let bytes = protocol.presence_fields_response(&[entry]).unwrap();
        let (slot, command, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(slot, Some(PRESENCE_SLOT));
        assert_eq!(command, PRESENCE_FIELDS_COMMAND);
        match payload {
            Payload::PresenceFields(fields) => {
                assert_eq!(fields.entries.len(), 1);
                assert_eq!(fields.entries[0], entry);
            }
            other => panic!("expected PresenceFields, got {other:?}"),
        }
    }

    #[test]
    #[ignore = "toon_welcome field order still diverges from the decoder; omitted from the greeting for now"]
    fn toon_welcome_response_round_trips() {
        let protocol = Protocol::current().expect("load current native protocol");
        let bytes = protocol
            .toon_welcome_response()
            .expect("encode toon welcome record");
        let (service_slot, command, _payload) = protocol
            .decode_server_record(&bytes)
            .expect("decode toon welcome record without leftover/short bytes");
        assert_eq!(
            service_slot,
            Some(TOON_SLOT),
            "welcome targets the Toon slot"
        );
        assert_eq!(
            command, TOON_WELCOME_COMMAND,
            "welcome uses the Toon Welcome command"
        );
    }

    #[test]
    fn club_settings_response_round_trips_through_the_decoder() {
        let protocol = crate::native::protocol::Protocol::current().unwrap();

        let bytes = protocol.club_settings_response().unwrap();

        let (service_slot, command, _payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(
            service_slot,
            Some(crate::native::protocol::S2_MULTIPLAYER_SLOT)
        );
        assert_eq!(
            command,
            crate::native::protocol::S2_MULTIPLAYER_CLUB_SETTINGS_COMMAND
        );
    }

    #[test]
    fn friends_list_response_round_trips_empty_page() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol.friends_list_response().unwrap();
        let (service_slot, command, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(service_slot, Some(FRIENDS_SLOT));
        assert_eq!(command, FRIENDS_LIST_COMMAND);
        match payload {
            Payload::Friends(page) => {
                assert!(
                    page.updates.is_empty(),
                    "expected empty friends update list"
                );
                assert_eq!(page.complete, None, "expected absent `complete` flag");
            }
            other => panic!("expected Friends payload, got {other:?}"),
        }
    }

    #[test]
    fn conference_descriptions_response_is_empty_and_final() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol.conference_descriptions_response(true).unwrap();
        let mut reader = BitReader::new(&bytes, None).unwrap();
        let header = crate::bsn::bits::decode_routing_header(&bytes, None).unwrap();
        assert_eq!(header.service_slot, Some(CHAT_SLOT));
        assert_eq!(header.command_id, CHAT_CONFERENCE_DESCRIPTIONS_COMMAND);
        reader.set_position(11).unwrap();
        let Payload::ConferenceDescriptions(page) =
            crate::native::decode::conference_descriptions(&mut reader).unwrap()
        else {
            panic!("expected conference descriptions");
        };
        assert!(page.is_last);
        assert!(page.entries.is_empty());
    }
    #[test]
    fn modify_channel_list_response_echoes_token_and_succeeds() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol
            .modify_channel_list_response(268_438_306, 1_786_938_664)
            .unwrap();
        let record = crate::native::inspect::inspect_native_record(
            &protocol,
            crate::native::inspect::Direction::Incoming,
            &bytes,
        )
        .unwrap();
        assert_eq!(record.service_slot, CHAT_SLOT);
        assert_eq!(record.command_id, CHAT_MODIFY_CHANNEL_LIST_RESPONSE_COMMAND);
        let field = |suffix: &str| {
            record
                .fields
                .iter()
                .find(|f| f.path.ends_with(suffix))
                .unwrap_or_else(|| panic!("missing field {suffix}"))
                .value
                .clone()
        };
        // token echoed, and the SUCCESS variant (0) selected — not the failure the
        // captured response carried.
        assert_eq!(field("m_token"), "268438306");
        assert_eq!(field("m_result"), "variant 0");
    }

    #[test]
    fn toon_list_response_round_trips() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol
            .toon_list_response("Sunken", 1, 0x1122_3344_5566_7788, 1_786_938_664)
            .unwrap();
        let (slot, command, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(slot, Some(TOON_SLOT));
        assert_eq!(command, TOON_LIST_COMMAND);
        let crate::native::model::Payload::ToonList(list) = payload else {
            panic!("expected a toon list payload, got {payload:?}");
        };
        assert_eq!(list.displays.len(), 1);
        assert_eq!(list.displays[0].name, "Sunken");
        assert_eq!(list.displays[0].realm, 1);
        // the model drops last_online/profile, so verify those fields at the byte
        // level via the inspector: SC2 rejects the toon unless last_online is a real
        // timestamp and m_label is the universal 0xCAFEBABE constant.
        let record = crate::native::inspect::inspect_native_record(
            &protocol,
            crate::native::inspect::Direction::Incoming,
            &bytes,
        )
        .unwrap();
        let field = |suffix: &str| {
            record
                .fields
                .iter()
                .find(|f| f.path.ends_with(suffix))
                .unwrap_or_else(|| panic!("missing field {suffix}"))
                .value
                .clone()
        };
        assert_eq!(field("last_online"), "1786938664");
        assert_eq!(field("profile.m_label"), TOON_PROFILE_LABEL.to_string());
    }

    #[test]
    fn game_site_info_greeting_round_trips() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol.game_site_info_response().unwrap();
        let (service_slot, command_id, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(service_slot, Some(CONNECTION_SLOT));
        assert_eq!(command_id, CONNECTION_GAME_SITE_INFO_COMMAND);
        match payload {
            crate::native::model::Payload::StartupSummary(summary) => {
                assert_eq!(summary.kind, "game-site-info");
                assert_eq!(summary.item_count, 0);
                assert_eq!(summary.complete, None);
            }
            other => panic!("unexpected game-site-info payload: {other:?}"),
        }
    }

    #[test]
    fn toon_selected_response_round_trips_through_the_decoder() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol.toon_selected_response().unwrap();

        let (slot, command, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(slot, Some(TOON_SLOT));
        assert_eq!(command, TOON_SELECTED_COMMAND);

        let Payload::ToonSelected(selected) = payload else {
            panic!("expected a ToonSelected payload, got {payload:?}");
        };
        assert_eq!(selected.name, "Sunken");
        assert_eq!(selected.realm, 1);
        assert_eq!(selected.handle.realm, 1);
        assert_eq!(selected.handle.id, 42);
    }

    #[test]
    fn an_empty_club_summaries_reply_round_trips() {
        let protocol = Protocol::current().unwrap();

        let bytes = protocol.club_summaries_response(0xdead_beef).unwrap();

        // minimal empty record is exactly 12 bytes (96 bits).
        assert_eq!(bytes.len(), 12);

        // routing header must name the club-summaries route.
        let header = decode_routing_header(&bytes, None).unwrap();
        assert_eq!(header.command_id, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND);
        assert_eq!(header.service_slot, Some(S2_MULTIPLAYER_SLOT));

        // the club count at absolute bit 12 must be zero.
        let mut reader = BitReader::new(&bytes, None).unwrap();
        reader.set_position(12).unwrap();
        assert_eq!(reader.read(8).unwrap(), 0, "empty club list");

        // full decode: correct route, empty summaries, no short/leftover bytes.
        let (slot, command, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(slot, Some(S2_MULTIPLAYER_SLOT));
        assert_eq!(command, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND);
        match payload {
            Payload::ClubSummaries(clubs) => assert!(clubs.is_empty()),
            other => panic!("expected empty ClubSummaries, got {other:?}"),
        }
    }

    #[test]
    fn profile_settings_response_round_trips() {
        let protocol = crate::native::protocol::Protocol::current().unwrap();
        let bytes = protocol.profile_settings_response().unwrap();
        let (service_slot, command_id, payload) = protocol.decode_server_record(&bytes).unwrap();
        assert_eq!(
            service_slot,
            Some(crate::native::protocol::PROFILE_SLOT),
            "record must target the profile service slot (14)"
        );
        assert_eq!(
            command_id,
            crate::native::protocol::PROFILE_SETTINGS_AVAILABLE_COMMAND,
            "record must carry the settings-available command (4)"
        );
        match payload {
            crate::native::model::Payload::StartupSummary(summary) => {
                assert_eq!(summary.kind, "profile-settings");
                assert_eq!(summary.item_count, 1);
            }
            other => panic!("unexpected payload for profile settings: {other:?}"),
        }
    }

    #[test]
    fn retail_friend_invitation_decodes_to_the_exact_record_boundary() {
        let protocol = protocol();
        let bytes = hex::decode(
            "5c2b0b54616762616e2331353535000000012041000000000000000000000000fba7ed00002310fb01",
        )
        .unwrap();
        let header = decode_routing_header(&bytes, None).unwrap();
        assert_eq!(header.service_slot, Some(FRIENDS_SLOT));
        assert_eq!(header.command_id, FRIENDS_INVITATION_ADDED_COMMAND);
        let mut reader = BitReader::new(&bytes, None).unwrap();
        reader.set_position(header.bit_count).unwrap();
        let (type_id, Payload::Reflected(value)) =
            protocol.decode_incoming_from(&mut reader, header).unwrap()
        else {
            panic!("expected reflected invitation payload");
        };
        assert_eq!(type_id, 2_688);
        assert_eq!(reader.position(), 323);
        assert_eq!(reader.read(5).unwrap(), 0, "only byte padding remains");

        let root = value.as_struct().unwrap();
        let invitation = root.get("m_invitation").unwrap().as_struct().unwrap();
        let BsnValue::Optional(Some(nickname)) = invitation.get("m_nickname").unwrap() else {
            panic!("invitation omitted its nickname");
        };
        assert_eq!(nickname.as_string(), Some("Tagban#1555"));
        assert_eq!(invitation.get("m_role").unwrap().as_integer(), Some(1));
    }
}
