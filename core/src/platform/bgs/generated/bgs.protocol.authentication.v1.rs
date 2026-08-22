#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ModuleLoadRequest {
    #[prost(message, required, tag = "1")]
    pub module_handle: super::super::ContentHandle,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub message: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ModuleNotification {
    #[prost(int32, optional, tag = "2")]
    pub module_id: ::core::option::Option<i32>,
    #[prost(uint32, optional, tag = "3")]
    pub result: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ModuleMessageRequest {
    #[prost(int32, required, tag = "1")]
    pub module_id: i32,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub message: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct LogonRequest {
    #[prost(string, optional, tag = "1")]
    pub program: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "2")]
    pub platform: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "3")]
    pub locale: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "4")]
    pub email: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "5")]
    pub version: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(int32, optional, tag = "6")]
    pub application_version: ::core::option::Option<i32>,
    #[prost(bool, optional, tag = "7")]
    pub public_computer: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "10", default = "false")]
    pub allow_logon_queue_notifications: ::core::option::Option<bool>,
    #[prost(bytes = "vec", optional, tag = "12")]
    pub cached_web_credentials: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(string, optional, tag = "14")]
    pub user_agent: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "15")]
    pub device_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogonResult {
    #[prost(uint32, required, tag = "1")]
    pub error_code: u32,
    #[prost(message, optional, tag = "2")]
    pub account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "3")]
    pub game_account_id: ::prost::alloc::vec::Vec<super::super::EntityId>,
    #[prost(string, optional, tag = "4")]
    pub email: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, repeated, packed = "false", tag = "5")]
    pub available_region: ::prost::alloc::vec::Vec<u32>,
    #[prost(uint32, optional, tag = "6")]
    pub connected_region: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "7")]
    pub battle_tag: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "8")]
    pub geoip_country: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "9")]
    pub session_key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(bool, optional, tag = "10")]
    pub restricted_mode: ::core::option::Option<bool>,
    #[prost(string, optional, tag = "11")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateSsoTokenRequest {
    #[prost(fixed32, optional, tag = "1")]
    pub program: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateSsoTokenResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub sso_id: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub sso_secret: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct LogonUpdateRequest {
    #[prost(uint32, required, tag = "1")]
    pub error_code: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct LogonQueueUpdateRequest {
    #[prost(uint32, required, tag = "1")]
    pub position: u32,
    #[prost(uint64, required, tag = "2")]
    pub estimated_time: u64,
    #[prost(uint64, required, tag = "3")]
    pub eta_deviation_in_sec: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountSettingsNotification {
    #[prost(message, repeated, tag = "1")]
    pub licenses: ::prost::alloc::vec::Vec<super::super::account::v1::AccountLicense>,
    #[prost(bool, optional, tag = "2")]
    pub is_using_rid: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub is_playing_from_igr: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub can_receive_voice: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub can_send_voice: ::core::option::Option<bool>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ServerStateChangeRequest {
    #[prost(uint32, required, tag = "1")]
    pub state: u32,
    #[prost(uint64, required, tag = "2")]
    pub event_time: u64,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct VersionInfo {
    #[prost(uint32, optional, tag = "1")]
    pub number: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "2")]
    pub patch: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "3")]
    pub is_optional: ::core::option::Option<bool>,
    #[prost(uint64, optional, tag = "4")]
    pub kick_time: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct VersionInfoNotification {
    #[prost(message, optional, tag = "1")]
    pub version_info: ::core::option::Option<VersionInfo>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct MemModuleLoadRequest {
    #[prost(message, required, tag = "1")]
    pub handle: super::super::ContentHandle,
    #[prost(bytes = "vec", required, tag = "2")]
    pub key: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", required, tag = "3")]
    pub input: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct MemModuleLoadResponse {
    #[prost(bytes = "vec", required, tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SelectGameAccountRequest {
    #[prost(message, required, tag = "1")]
    pub game_account_id: super::super::EntityId,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountSelectedRequest {
    #[prost(uint32, required, tag = "1")]
    pub result: u32,
    #[prost(message, optional, tag = "2")]
    pub game_account_id: ::core::option::Option<super::super::EntityId>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateWebCredentialsRequest {
    #[prost(fixed32, optional, tag = "1")]
    pub program: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateWebCredentialsResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub web_credentials: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct VerifyWebCredentialsRequest {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub web_credentials: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SsoData {
    #[prost(string, optional, tag = "1")]
    pub account_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "16")]
    pub region: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "2")]
    pub program: ::core::option::Option<u32>,
    #[prost(uint64, optional, tag = "3")]
    pub time_created: ::core::option::Option<u64>,
    #[prost(string, optional, tag = "4")]
    pub ip_address: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateTokenRequest {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::super::account::v1::AccountId>,
    #[prost(fixed32, optional, tag = "2")]
    pub program: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub platform_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "4")]
    pub client_ip: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "6", default = "true")]
    pub single_use: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "7", default = "false")]
    pub generate_token_id: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateTokenResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub authentication_token: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub authentication_token_id: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AuthenticateTokenRequest {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub authentication_token: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(fixed32, optional, tag = "2")]
    pub program: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub platform_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "4")]
    pub locale: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "5")]
    pub client_ip: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "6")]
    pub user_agent: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, optional, tag = "7")]
    pub version: ::core::option::Option<u64>,
    #[prost(bytes = "vec", optional, tag = "8")]
    pub authentication_token_id: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct AuthenticateTokenResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub authentication_token: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(message, optional, tag = "2")]
    pub sso_data: ::core::option::Option<SsoData>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateTrustedWebCredentialsRequest {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::super::account::v1::AccountId>,
    #[prost(fixed32, optional, tag = "2")]
    pub program: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub platform_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "4")]
    pub client_ip: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub session_key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GenerateTrustedWebCredentialsResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub web_credentials: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
