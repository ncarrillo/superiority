#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientRequest {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(message, optional, tag = "2")]
    pub host: ::core::option::Option<super::super::ProcessId>,
    #[prost(message, optional, tag = "3")]
    pub account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "4")]
    pub game_account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(fixed32, optional, tag = "5")]
    pub program: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "6")]
    pub client_info: ::core::option::Option<ClientInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientResponse {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerRequest {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(fixed32, required, tag = "2")]
    pub program: u32,
    #[prost(message, optional, tag = "3")]
    pub host: ::core::option::Option<super::super::ProcessId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerResponse {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct PresenceChannelCreatedRequest {
    #[prost(message, required, tag = "1")]
    pub id: super::super::EntityId,
    #[prost(message, optional, tag = "3")]
    pub game_account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "4")]
    pub account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "5")]
    pub host: ::core::option::Option<super::super::ProcessId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPlayerVariablesRequest {
    #[prost(message, repeated, tag = "1")]
    pub player_variables: ::prost::alloc::vec::Vec<PlayerVariables>,
    #[prost(message, optional, tag = "2")]
    pub host: ::core::option::Option<super::super::ProcessId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPlayerVariablesResponse {
    #[prost(message, repeated, tag = "1")]
    pub player_variables: ::prost::alloc::vec::Vec<PlayerVariables>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountOnlineNotification {
    #[prost(message, required, tag = "1")]
    pub game_account_id: super::super::EntityId,
    #[prost(message, optional, tag = "2")]
    pub host: ::core::option::Option<super::super::ProcessId>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GameAccountOfflineNotification {
    #[prost(message, required, tag = "1")]
    pub game_account_id: super::super::EntityId,
    #[prost(message, optional, tag = "2")]
    pub host: ::core::option::Option<super::super::ProcessId>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetAchievementsFileRequest {
    #[prost(message, optional, tag = "1")]
    pub host: ::core::option::Option<super::super::ProcessId>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetAchievementsFileResponse {
    #[prost(message, optional, tag = "1")]
    pub content_handle: ::core::option::Option<super::super::ContentHandle>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct GetAllValuesForAttributeRequest {
    #[prost(string, optional, tag = "1")]
    pub attribute_key: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(message, optional, tag = "2")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(fixed32, optional, tag = "5")]
    pub program: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAllValuesForAttributeResponse {
    #[prost(message, repeated, tag = "1")]
    pub attribute_value: ::prost::alloc::vec::Vec<super::super::Variant>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegisterUtilitiesRequest {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(fixed32, optional, tag = "2")]
    pub program: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RegisterUtilitiesResponse {
    #[prost(string, optional, tag = "1")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct UnregisterUtilitiesRequest {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlayerVariables {
    #[prost(message, required, tag = "1")]
    pub identity: super::super::Identity,
    #[prost(double, optional, tag = "2")]
    pub rating: ::core::option::Option<f64>,
    #[prost(message, repeated, tag = "3")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ClientInfo {
    #[prost(string, optional, tag = "1")]
    pub client_address: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "2")]
    pub privileged_network: ::core::option::Option<bool>,
}
