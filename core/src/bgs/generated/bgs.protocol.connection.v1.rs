#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectRequest {
    #[prost(message, optional, tag = "1")]
    pub client_id: ::core::option::Option<super::super::ProcessId>,
    #[prost(message, optional, tag = "2")]
    pub bind_request: ::core::option::Option<BindRequest>,
    #[prost(bool, optional, tag = "3", default = "true")]
    pub use_bindless_rpc: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectionMeteringContentHandles {
    #[prost(message, repeated, tag = "1")]
    pub content_handle: ::prost::alloc::vec::Vec<super::super::ContentHandle>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectResponse {
    #[prost(message, required, tag = "1")]
    pub server_id: super::super::ProcessId,
    #[prost(message, optional, tag = "2")]
    pub client_id: ::core::option::Option<super::super::ProcessId>,
    #[prost(uint32, optional, tag = "3")]
    pub bind_result: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "4")]
    pub bind_response: ::core::option::Option<BindResponse>,
    #[prost(message, optional, tag = "5")]
    pub content_handle_array: ::core::option::Option<ConnectionMeteringContentHandles>,
    #[prost(uint64, optional, tag = "6")]
    pub server_time: ::core::option::Option<u64>,
    #[prost(bool, optional, tag = "7", default = "false")]
    pub use_bindless_rpc: ::core::option::Option<bool>,
    #[prost(message, optional, tag = "8")]
    pub binary_content_handle_array: ::core::option::Option<
        ConnectionMeteringContentHandles,
    >,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct BoundService {
    #[prost(fixed32, required, tag = "1")]
    pub hash: u32,
    #[prost(uint32, required, tag = "2")]
    pub id: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BindRequest {
    #[deprecated]
    #[prost(fixed32, repeated, tag = "1")]
    pub deprecated_imported_service_hash: ::prost::alloc::vec::Vec<u32>,
    #[deprecated]
    #[prost(message, repeated, tag = "2")]
    pub deprecated_exported_service: ::prost::alloc::vec::Vec<BoundService>,
    #[prost(message, repeated, tag = "3")]
    pub exported_service: ::prost::alloc::vec::Vec<BoundService>,
    #[prost(message, repeated, tag = "4")]
    pub imported_service: ::prost::alloc::vec::Vec<BoundService>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct BindResponse {
    #[deprecated]
    #[prost(uint32, repeated, tag = "1")]
    pub imported_service_id: ::prost::alloc::vec::Vec<u32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct EchoRequest {
    #[prost(fixed64, optional, tag = "1")]
    pub time: ::core::option::Option<u64>,
    #[prost(bool, optional, tag = "2", default = "false")]
    pub network_only: ::core::option::Option<bool>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub payload: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(message, optional, tag = "4")]
    pub forward: ::core::option::Option<super::super::ProcessId>,
    #[prost(string, optional, tag = "5")]
    pub forward_client_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct EchoResponse {
    #[prost(fixed64, optional, tag = "1")]
    pub time: ::core::option::Option<u64>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub payload: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct DisconnectRequest {
    #[prost(uint32, required, tag = "1")]
    pub error_code: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct DisconnectNotification {
    #[prost(uint32, required, tag = "1")]
    pub error_code: u32,
    #[prost(string, optional, tag = "2")]
    pub reason: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct EncryptRequest {}
