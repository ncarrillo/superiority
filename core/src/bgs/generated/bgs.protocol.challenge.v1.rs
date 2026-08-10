#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ChallengeExternalRequest {
    #[prost(string, optional, tag = "1")]
    pub request_token: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "2")]
    pub payload_type: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub payload: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ChallengeExternalResult {
    #[prost(string, optional, tag = "1")]
    pub request_token: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "2", default = "true")]
    pub passed: ::core::option::Option<bool>,
}
