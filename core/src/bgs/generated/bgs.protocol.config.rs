#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RpcMethodConfig {
    #[deprecated]
    #[prost(string, optional, tag = "1")]
    pub service_name: ::core::option::Option<::prost::alloc::string::String>,
    #[deprecated]
    #[prost(string, optional, tag = "2")]
    pub method_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "3", default = "1")]
    pub fixed_call_cost: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "4", default = "0")]
    pub fixed_packet_size: ::core::option::Option<u32>,
    #[prost(float, optional, tag = "5", default = "0")]
    pub variable_multiplier: ::core::option::Option<f32>,
    #[prost(float, optional, tag = "6", default = "1")]
    pub multiplier: ::core::option::Option<f32>,
    #[prost(uint32, optional, tag = "7")]
    pub rate_limit_count: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "8")]
    pub rate_limit_seconds: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "9")]
    pub max_packet_size: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "10")]
    pub max_encoded_size: ::core::option::Option<u32>,
    #[prost(float, optional, tag = "11")]
    pub timeout: ::core::option::Option<f32>,
    #[prost(uint32, optional, tag = "12")]
    pub cap_balance: ::core::option::Option<u32>,
    #[prost(float, optional, tag = "13", default = "0")]
    pub income_per_second: ::core::option::Option<f32>,
    #[prost(uint32, optional, tag = "14")]
    pub service_hash: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "15")]
    pub method_id: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RpcMeterConfig {
    #[prost(message, repeated, tag = "1")]
    pub method: ::prost::alloc::vec::Vec<RpcMethodConfig>,
    #[prost(uint32, optional, tag = "2", default = "1")]
    pub income_per_second: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub initial_balance: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    pub cap_balance: ::core::option::Option<u32>,
    #[prost(float, optional, tag = "5", default = "0")]
    pub startup_period: ::core::option::Option<f32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtocolAlias {
    #[prost(string, required, tag = "1")]
    pub server_service_name: ::prost::alloc::string::String,
    #[prost(string, required, tag = "2")]
    pub client_service_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServiceAliases {
    #[prost(message, repeated, tag = "1")]
    pub protocol_alias: ::prost::alloc::vec::Vec<ProtocolAlias>,
}
