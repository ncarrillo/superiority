#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct Target {
    #[prost(message, optional, tag = "1")]
    pub identity: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(string, optional, tag = "2")]
    pub r#type: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Subscription {
    #[prost(message, repeated, tag = "1")]
    pub target: ::prost::alloc::vec::Vec<Target>,
    #[prost(message, optional, tag = "2")]
    pub subscriber: ::core::option::Option<super::super::account::v1::Identity>,
    #[prost(bool, optional, tag = "3")]
    pub delivery_required: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Notification {
    #[prost(message, optional, tag = "1")]
    pub sender_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub target_id: super::super::EntityId,
    #[prost(string, required, tag = "3")]
    pub r#type: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "4")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(message, optional, tag = "5")]
    pub sender_account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, optional, tag = "6")]
    pub target_account_id: ::core::option::Option<super::super::EntityId>,
    #[prost(string, optional, tag = "7")]
    pub sender_battle_tag: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "8")]
    pub target_battle_tag: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(message, optional, tag = "10")]
    pub forwarding_identity: ::core::option::Option<super::super::account::v1::Identity>,
}
