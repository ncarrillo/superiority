#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ResolveAccountRequest {
    #[prost(message, optional, tag = "1")]
    pub r#ref: ::core::option::Option<AccountReference>,
    #[prost(bool, optional, tag = "12")]
    pub fetch_id: ::core::option::Option<bool>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ResolveAccountResponse {
    #[prost(message, optional, tag = "12")]
    pub id: ::core::option::Option<AccountId>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountFlagUpdateRequest {
    #[prost(message, optional, tag = "1")]
    pub game_account: ::core::option::Option<GameAccountHandle>,
    #[prost(uint64, optional, tag = "2")]
    pub flag: ::core::option::Option<u64>,
    #[prost(bool, optional, tag = "3")]
    pub active: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscriptionUpdateRequest {
    #[prost(message, repeated, tag = "2")]
    pub r#ref: ::prost::alloc::vec::Vec<SubscriberReference>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscriptionUpdateResponse {
    #[prost(message, repeated, tag = "1")]
    pub r#ref: ::prost::alloc::vec::Vec<SubscriberReference>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccountStateRequest {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
    #[prost(uint32, optional, tag = "2")]
    pub program: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub region: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "10")]
    pub options: ::core::option::Option<AccountFieldOptions>,
    #[prost(message, optional, tag = "11")]
    pub tags: ::core::option::Option<AccountFieldTags>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccountStateResponse {
    #[prost(message, optional, tag = "1")]
    pub state: ::core::option::Option<AccountState>,
    #[prost(message, optional, tag = "2")]
    pub tags: ::core::option::Option<AccountFieldTags>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSignedAccountStateRequest {
    #[prost(message, optional, tag = "1")]
    pub account: ::core::option::Option<AccountId>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSignedAccountStateResponse {
    #[prost(string, optional, tag = "1")]
    pub token: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetGameAccountStateRequest {
    #[deprecated]
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "2")]
    pub game_account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "10")]
    pub options: ::core::option::Option<GameAccountFieldOptions>,
    #[prost(message, optional, tag = "11")]
    pub tags: ::core::option::Option<GameAccountFieldTags>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetGameAccountStateResponse {
    #[prost(message, optional, tag = "1")]
    pub state: ::core::option::Option<GameAccountState>,
    #[prost(message, optional, tag = "2")]
    pub tags: ::core::option::Option<GameAccountFieldTags>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetLicensesRequest {
    #[prost(message, optional, tag = "1")]
    pub target_id: ::core::option::Option<super::super::EntityId>,
    #[prost(bool, optional, tag = "2")]
    pub fetch_account_licenses: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub fetch_game_account_licenses: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub fetch_dynamic_account_licenses: ::core::option::Option<bool>,
    #[prost(fixed32, optional, tag = "5")]
    pub program: ::core::option::Option<u32>,
    #[prost(bool, optional, tag = "6", default = "false")]
    pub exclude_unknown_program: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLicensesResponse {
    #[prost(message, repeated, tag = "1")]
    pub licenses: ::prost::alloc::vec::Vec<AccountLicense>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetGameSessionInfoRequest {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetGameSessionInfoResponse {
    #[prost(message, optional, tag = "2")]
    pub session_info: ::core::option::Option<GameSessionInfo>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetGameTimeRemainingInfoRequest {
    #[prost(message, optional, tag = "1")]
    pub game_account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "2")]
    pub account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(string, optional, tag = "3")]
    pub benefactor_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetGameTimeRemainingInfoResponse {
    #[prost(message, optional, tag = "1")]
    pub game_time_remaining_info: ::core::option::Option<GameTimeRemainingInfo>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetCaisInfoRequest {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetCaisInfoResponse {
    #[prost(message, optional, tag = "1")]
    pub cais_info: ::core::option::Option<Cais>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetAuthorizedDataRequest {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
    #[prost(string, repeated, tag = "2")]
    pub tag: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "3")]
    pub privileged_network: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAuthorizedDataResponse {
    #[prost(message, repeated, tag = "1")]
    pub data: ::prost::alloc::vec::Vec<AuthorizedData>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountStateNotification {
    #[prost(message, optional, tag = "1")]
    pub account_state: ::core::option::Option<AccountState>,
    #[deprecated]
    #[prost(uint64, optional, tag = "2")]
    pub subscriber_id: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "3")]
    pub account_tags: ::core::option::Option<AccountFieldTags>,
    #[prost(bool, optional, tag = "4")]
    pub subscription_completed: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameAccountStateNotification {
    #[prost(message, optional, tag = "1")]
    pub game_account_state: ::core::option::Option<GameAccountState>,
    #[deprecated]
    #[prost(uint64, optional, tag = "2")]
    pub subscriber_id: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "3")]
    pub game_account_tags: ::core::option::Option<GameAccountFieldTags>,
    #[prost(bool, optional, tag = "4")]
    pub subscription_completed: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameAccountNotification {
    #[prost(message, repeated, tag = "1")]
    pub game_accounts: ::prost::alloc::vec::Vec<GameAccountList>,
    #[prost(uint64, optional, tag = "2")]
    pub subscriber_id: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "3")]
    pub account_tags: ::core::option::Option<AccountFieldTags>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountSessionNotification {
    #[prost(message, optional, tag = "1")]
    pub game_account: ::core::option::Option<GameAccountHandle>,
    #[prost(message, optional, tag = "2")]
    pub session_info: ::core::option::Option<GameSessionUpdateInfo>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AccountId {
    #[prost(fixed32, required, tag = "1")]
    pub id: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AccountLicense {
    #[prost(uint32, required, tag = "1")]
    pub id: u32,
    #[prost(uint64, optional, tag = "2")]
    pub expires: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountHandle {
    #[prost(fixed32, required, tag = "1")]
    pub id: u32,
    #[prost(fixed32, required, tag = "2")]
    pub program: u32,
    #[prost(uint32, required, tag = "3")]
    pub region: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AccountReference {
    #[prost(fixed32, optional, tag = "1")]
    pub id: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "2")]
    pub email: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(message, optional, tag = "3")]
    pub handle: ::core::option::Option<GameAccountHandle>,
    #[prost(string, optional, tag = "4")]
    pub battle_tag: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "10", default = "0")]
    pub region: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct Identity {
    #[prost(message, optional, tag = "1")]
    pub account: ::core::option::Option<AccountId>,
    #[prost(message, optional, tag = "2")]
    pub game_account: ::core::option::Option<GameAccountHandle>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProgramTag {
    #[prost(fixed32, optional, tag = "1")]
    pub program: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "2")]
    pub tag: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RegionTag {
    #[prost(fixed32, optional, tag = "1")]
    pub region: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "2")]
    pub tag: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountFieldTags {
    #[prost(fixed32, optional, tag = "2")]
    pub account_level_info_tag: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "3")]
    pub privacy_info_tag: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "4")]
    pub parental_control_info_tag: ::core::option::Option<u32>,
    #[prost(message, repeated, tag = "7")]
    pub game_level_info_tags: ::prost::alloc::vec::Vec<ProgramTag>,
    #[prost(message, repeated, tag = "9")]
    pub game_status_tags: ::prost::alloc::vec::Vec<ProgramTag>,
    #[prost(message, repeated, tag = "11")]
    pub game_account_tags: ::prost::alloc::vec::Vec<RegionTag>,
    #[prost(fixed32, optional, tag = "12")]
    pub security_status_tag: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountFieldTags {
    #[prost(fixed32, optional, tag = "2")]
    pub game_level_info_tag: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "3")]
    pub game_time_info_tag: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "4")]
    pub game_status_tag: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "5")]
    pub raf_info_tag: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AccountFieldOptions {
    #[prost(bool, optional, tag = "1")]
    pub all_fields: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub field_account_level_info: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub field_privacy_info: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub field_parental_control_info: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "6")]
    pub field_game_level_info: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub field_game_status: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "8")]
    pub field_game_accounts: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "9")]
    pub field_security_status: ::core::option::Option<bool>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountFieldOptions {
    #[prost(bool, optional, tag = "1")]
    pub all_fields: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub field_game_level_info: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub field_game_time_info: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub field_game_status: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub field_raf_info: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscriberReference {
    #[prost(uint64, optional, tag = "1", default = "0")]
    pub object_id: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "2")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "3")]
    pub account_options: ::core::option::Option<AccountFieldOptions>,
    #[prost(message, optional, tag = "4")]
    pub account_tags: ::core::option::Option<AccountFieldTags>,
    #[prost(message, optional, tag = "5")]
    pub game_account_options: ::core::option::Option<GameAccountFieldOptions>,
    #[prost(message, optional, tag = "6")]
    pub game_account_tags: ::core::option::Option<GameAccountFieldTags>,
    #[prost(uint64, optional, tag = "7", default = "0")]
    pub subscriber_id: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountLevelInfo {
    #[prost(message, repeated, tag = "3")]
    pub licenses: ::prost::alloc::vec::Vec<AccountLicense>,
    #[prost(fixed32, optional, tag = "4")]
    pub default_currency: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "5")]
    pub country: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "6")]
    pub preferred_region: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "7")]
    pub full_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "8")]
    pub battle_tag: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "9")]
    pub muted: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "10")]
    pub manual_review: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "11")]
    pub account_paid_any: ::core::option::Option<bool>,
    #[prost(enumeration = "IdentityVerificationStatus", optional, tag = "12")]
    pub identity_check_status: ::core::option::Option<i32>,
    #[prost(string, optional, tag = "13")]
    pub email: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "14")]
    pub headless_account: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "15")]
    pub test_account: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "17")]
    pub is_sms_protected: ::core::option::Option<bool>,
    #[prost(uint32, optional, tag = "18")]
    pub ratings_board_minimum_age: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct PrivacyInfo {
    #[prost(bool, optional, tag = "3")]
    pub is_using_rid: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub is_visible_for_view_friends: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub is_hidden_from_friend_finder: ::core::option::Option<bool>,
    #[prost(
        enumeration = "privacy_info::GameInfoPrivacy",
        optional,
        tag = "6",
        default = "PrivacyFriends"
    )]
    pub game_info_privacy: ::core::option::Option<i32>,
    #[prost(bool, optional, tag = "7")]
    pub only_allow_friend_whispers: ::core::option::Option<bool>,
}
pub mod privacy_info {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum GameInfoPrivacy {
        PrivacyMe = 0,
        PrivacyFriends = 1,
        PrivacyEveryone = 2,
    }
    impl GameInfoPrivacy {
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::PrivacyMe => "PRIVACY_ME",
                Self::PrivacyFriends => "PRIVACY_FRIENDS",
                Self::PrivacyEveryone => "PRIVACY_EVERYONE",
            }
        }
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "PRIVACY_ME" => Some(Self::PrivacyMe),
                "PRIVACY_FRIENDS" => Some(Self::PrivacyFriends),
                "PRIVACY_EVERYONE" => Some(Self::PrivacyEveryone),
                _ => None,
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ParentalControlInfo {
    #[prost(string, optional, tag = "3")]
    pub timezone: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "4")]
    pub minutes_per_day: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "5")]
    pub minutes_per_week: ::core::option::Option<u32>,
    #[prost(bool, optional, tag = "6")]
    pub can_receive_voice: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub can_send_voice: ::core::option::Option<bool>,
    #[prost(bool, repeated, packed = "false", tag = "8")]
    pub play_schedule: ::prost::alloc::vec::Vec<bool>,
    #[prost(bool, optional, tag = "9")]
    pub can_join_group: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "10")]
    pub can_use_profile: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameLevelInfo {
    #[prost(bool, optional, tag = "4")]
    pub is_trial: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub is_lifetime: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "6")]
    pub is_restricted: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub is_beta: ::core::option::Option<bool>,
    #[prost(string, optional, tag = "8")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(fixed32, optional, tag = "9")]
    pub program: ::core::option::Option<u32>,
    #[prost(message, repeated, tag = "10")]
    pub licenses: ::prost::alloc::vec::Vec<AccountLicense>,
    #[prost(uint32, optional, tag = "11")]
    pub realm_permissions: ::core::option::Option<u32>,
    #[prost(uint64, optional, tag = "12")]
    pub last_logout_time_ms: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameTimeInfo {
    #[prost(bool, optional, tag = "3")]
    pub is_unlimited_play_time: ::core::option::Option<bool>,
    #[prost(uint64, optional, tag = "5")]
    pub play_time_expires: ::core::option::Option<u64>,
    #[prost(bool, optional, tag = "6")]
    pub is_subscription: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub is_recurring_subscription: ::core::option::Option<bool>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameTimeRemainingInfo {
    #[prost(uint32, optional, tag = "1")]
    pub minutes_remaining: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub parental_daily_minutes_remaining: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub parental_weekly_minutes_remaining: ::core::option::Option<u32>,
    #[deprecated]
    #[prost(uint32, optional, tag = "4")]
    pub seconds_remaining_until_kick: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameStatus {
    #[prost(bool, optional, tag = "4")]
    pub is_suspended: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub is_banned: ::core::option::Option<bool>,
    #[prost(uint64, optional, tag = "6")]
    pub suspension_expires: ::core::option::Option<u64>,
    #[prost(fixed32, optional, tag = "7")]
    pub program: ::core::option::Option<u32>,
    #[prost(bool, optional, tag = "8")]
    pub is_locked: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "9")]
    pub is_bam_unlockable: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RafInfo {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub raf_info: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameSessionInfo {
    #[deprecated]
    #[prost(uint32, optional, tag = "3")]
    pub start_time: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "4")]
    pub location: ::core::option::Option<GameSessionLocation>,
    #[prost(bool, optional, tag = "5")]
    pub has_benefactor: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "6")]
    pub is_using_igr: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub parental_controls_active: ::core::option::Option<bool>,
    #[prost(uint64, optional, tag = "8")]
    pub start_time_sec: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "9")]
    pub igr_id: ::core::option::Option<IgrId>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameSessionUpdateInfo {
    #[prost(message, optional, tag = "8")]
    pub cais: ::core::option::Option<Cais>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameSessionLocation {
    #[prost(string, optional, tag = "1")]
    pub ip_address: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "2")]
    pub country: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub city: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct Cais {
    #[prost(uint32, optional, tag = "1")]
    pub played_minutes: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub rested_minutes: ::core::option::Option<u32>,
    #[prost(uint64, optional, tag = "3")]
    pub last_heard_time: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameAccountList {
    #[prost(uint32, optional, tag = "3")]
    pub region: ::core::option::Option<u32>,
    #[prost(message, repeated, tag = "4")]
    pub handle: ::prost::alloc::vec::Vec<GameAccountHandle>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SecurityStatus {
    #[prost(bool, optional, tag = "1")]
    pub sms_protect_enabled: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub email_verified: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub authenticator_enabled: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub sqa_enabled: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub authenticator_required: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountState {
    #[prost(message, optional, tag = "1")]
    pub account_level_info: ::core::option::Option<AccountLevelInfo>,
    #[prost(message, optional, tag = "2")]
    pub privacy_info: ::core::option::Option<PrivacyInfo>,
    #[prost(message, optional, tag = "3")]
    pub parental_control_info: ::core::option::Option<ParentalControlInfo>,
    #[prost(message, repeated, tag = "5")]
    pub game_level_info: ::prost::alloc::vec::Vec<GameLevelInfo>,
    #[prost(message, repeated, tag = "6")]
    pub game_status: ::prost::alloc::vec::Vec<GameStatus>,
    #[prost(message, repeated, tag = "7")]
    pub game_accounts: ::prost::alloc::vec::Vec<GameAccountList>,
    #[prost(message, optional, tag = "8")]
    pub security_status: ::core::option::Option<SecurityStatus>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountStateTagged {
    #[prost(message, optional, tag = "1")]
    pub account_state: ::core::option::Option<AccountState>,
    #[prost(message, optional, tag = "2")]
    pub account_tags: ::core::option::Option<AccountFieldTags>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameAccountState {
    #[prost(message, optional, tag = "1")]
    pub game_level_info: ::core::option::Option<GameLevelInfo>,
    #[prost(message, optional, tag = "2")]
    pub game_time_info: ::core::option::Option<GameTimeInfo>,
    #[prost(message, optional, tag = "3")]
    pub game_status: ::core::option::Option<GameStatus>,
    #[prost(message, optional, tag = "4")]
    pub raf_info: ::core::option::Option<RafInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GameAccountStateTagged {
    #[prost(message, optional, tag = "1")]
    pub game_account_state: ::core::option::Option<GameAccountState>,
    #[prost(message, optional, tag = "2")]
    pub game_account_tags: ::core::option::Option<GameAccountFieldTags>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AuthorizedData {
    #[prost(string, optional, tag = "1")]
    pub data: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, repeated, packed = "false", tag = "2")]
    pub license: ::prost::alloc::vec::Vec<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct IgrId {
    #[prost(oneof = "igr_id::Type", tags = "1, 2, 3")]
    pub r#type: ::core::option::Option<igr_id::Type>,
}
pub mod igr_id {
    #[derive(Clone, PartialEq, Eq, Hash, ::prost::Oneof)]
    pub enum Type {
        #[prost(message, tag = "1")]
        GameAccount(super::GameAccountHandle),
        #[prost(fixed32, tag = "2")]
        ExternalId(u32),
        #[prost(string, tag = "3")]
        Uuid(::prost::alloc::string::String),
    }
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct IgrAddress {
    #[prost(string, optional, tag = "1")]
    pub client_address: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "2")]
    pub region: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum IdentityVerificationStatus {
    IdentNoData = 0,
    IdentPending = 1,
    IdentOver18 = 2,
    IdentUnder18 = 3,
    IdentFailed = 4,
    IdentSuccess = 5,
    IdentSuccMnl = 6,
    IdentUnknown = 7,
}
impl IdentityVerificationStatus {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::IdentNoData => "IDENT_NO_DATA",
            Self::IdentPending => "IDENT_PENDING",
            Self::IdentOver18 => "IDENT_OVER_18",
            Self::IdentUnder18 => "IDENT_UNDER_18",
            Self::IdentFailed => "IDENT_FAILED",
            Self::IdentSuccess => "IDENT_SUCCESS",
            Self::IdentSuccMnl => "IDENT_SUCC_MNL",
            Self::IdentUnknown => "IDENT_UNKNOWN",
        }
    }
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "IDENT_NO_DATA" => Some(Self::IdentNoData),
            "IDENT_PENDING" => Some(Self::IdentPending),
            "IDENT_OVER_18" => Some(Self::IdentOver18),
            "IDENT_UNDER_18" => Some(Self::IdentUnder18),
            "IDENT_FAILED" => Some(Self::IdentFailed),
            "IDENT_SUCCESS" => Some(Self::IdentSuccess),
            "IDENT_SUCC_MNL" => Some(Self::IdentSuccMnl),
            "IDENT_UNKNOWN" => Some(Self::IdentUnknown),
            _ => None,
        }
    }
}
