#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ContentHandleRequest {
    #[prost(fixed32, required, tag = "1")]
    pub program: u32,
    #[prost(fixed32, required, tag = "2")]
    pub stream: u32,
    #[prost(fixed32, optional, tag = "3", default = "1701729619")]
    pub version: ::core::option::Option<u32>,
}
