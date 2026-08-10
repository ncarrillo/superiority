#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RegisterUtilitiesRequest {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(fixed32, required, tag = "3")]
    pub program: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RegisterUtilitiesResponse {
    #[prost(string, optional, tag = "1")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct UnregisterUtilitiesRequest {}
