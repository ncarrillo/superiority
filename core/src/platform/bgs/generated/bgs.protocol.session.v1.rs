#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct CreateSessionRequest {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(fixed32, optional, tag = "2")]
    pub platform: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "3")]
    pub locale: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "4")]
    pub client_address: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(int32, optional, tag = "5")]
    pub application_version: ::core::option::Option<i32>,
    #[prost(string, optional, tag = "6")]
    pub user_agent: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub session_key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(message, optional, tag = "8")]
    pub options: ::core::option::Option<SessionOptions>,
    #[prost(bool, optional, tag = "9", default = "false")]
    pub requires_mark_alive: ::core::option::Option<bool>,
    #[prost(string, optional, tag = "10")]
    pub mac_address: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct CreateSessionResponse {
    #[prost(string, optional, tag = "1")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct UpdateSessionRequest {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(message, optional, tag = "2")]
    pub options: ::core::option::Option<SessionOptions>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct DestroySessionRequest {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(string, optional, tag = "2")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSessionCapacityRequest {}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSessionCapacityResponse {
    #[prost(uint32, optional, tag = "1")]
    pub sessions_available: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub time_frame_seconds: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSessionStateByBenefactorRequest {
    #[prost(message, optional, tag = "1")]
    pub benefactor_handle: ::core::option::Option<
        super::super::account::v1::GameAccountHandle,
    >,
    #[prost(bool, optional, tag = "2", default = "false")]
    pub include_billing_disabled: ::core::option::Option<bool>,
    #[prost(string, optional, tag = "3")]
    pub benefactor_uuid: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSessionStateByBenefactorResponse {
    #[deprecated]
    #[prost(message, optional, tag = "1")]
    pub benefactor_handle: ::core::option::Option<
        super::super::account::v1::GameAccountHandle,
    >,
    #[prost(message, repeated, tag = "2")]
    pub session: ::prost::alloc::vec::Vec<SessionState>,
    #[prost(uint32, optional, tag = "3")]
    pub minutes_remaining: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarkSessionsAliveRequest {
    #[prost(message, repeated, tag = "1")]
    pub session: ::prost::alloc::vec::Vec<SessionIdentifier>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarkSessionsAliveResponse {
    #[prost(message, repeated, tag = "1")]
    pub failed_session: ::prost::alloc::vec::Vec<SessionIdentifier>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSessionStateRequest {
    #[prost(message, optional, tag = "1")]
    pub handle: ::core::option::Option<super::super::account::v1::GameAccountHandle>,
    #[prost(bool, optional, tag = "2", default = "false")]
    pub include_billing_disabled: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSessionStateResponse {
    #[deprecated]
    #[prost(message, optional, tag = "1")]
    pub handle: ::core::option::Option<super::super::account::v1::GameAccountHandle>,
    #[prost(message, optional, tag = "2")]
    pub session: ::core::option::Option<SessionState>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSignedSessionStateRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::account::v1::GameAccountHandle>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetSignedSessionStateResponse {
    #[prost(string, optional, tag = "1")]
    pub token: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RefreshSessionKeyRequest {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub session_key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RefreshSessionKeyResponse {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub session_key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SessionCreatedNotification {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(uint32, optional, tag = "2")]
    pub reason: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub session_key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(string, optional, tag = "5")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SessionDestroyedNotification {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(uint32, optional, tag = "2")]
    pub reason: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SessionUpdatedNotification {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(uint32, optional, tag = "2")]
    pub reason: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SessionOptions {
    #[prost(bool, optional, tag = "1", default = "true")]
    pub billing: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "2", default = "true")]
    pub presence: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SessionState {
    #[prost(message, optional, tag = "1")]
    pub handle: ::core::option::Option<super::super::account::v1::GameAccountHandle>,
    #[prost(string, optional, tag = "2")]
    pub client_address: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, optional, tag = "3")]
    pub last_tick_time: ::core::option::Option<u64>,
    #[prost(uint64, optional, tag = "4")]
    pub create_time: ::core::option::Option<u64>,
    #[prost(bool, optional, tag = "5")]
    pub parental_controls_active: ::core::option::Option<bool>,
    #[prost(message, optional, tag = "6")]
    pub location: ::core::option::Option<super::super::account::v1::GameSessionLocation>,
    #[prost(bool, optional, tag = "7")]
    pub using_igr_address: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "8")]
    pub has_benefactor: ::core::option::Option<bool>,
    #[prost(message, optional, tag = "9")]
    pub igr_id: ::core::option::Option<super::super::account::v1::IgrId>,
    #[prost(bool, optional, tag = "10")]
    pub deductible: ::core::option::Option<bool>,
    #[prost(uint64, optional, tag = "11")]
    pub expire_time_ms: ::core::option::Option<u64>,
    #[prost(string, optional, tag = "12")]
    pub mac_address: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SessionIdentifier {
    #[prost(message, optional, tag = "1")]
    pub game_account: ::core::option::Option<
        super::super::account::v1::GameAccountHandle,
    >,
    #[prost(string, optional, tag = "2")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, optional, tag = "3")]
    pub account_id: ::core::option::Option<u64>,
}
