#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub entity_id: super::super::EntityId,
    #[prost(uint64, required, tag = "3")]
    pub object_id: u64,
    #[prost(fixed32, repeated, packed = "false", tag = "4")]
    pub program: ::prost::alloc::vec::Vec<u32>,
    #[prost(message, repeated, tag = "6")]
    pub key: ::prost::alloc::vec::Vec<FieldKey>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SubscribeNotificationRequest {
    #[prost(message, required, tag = "1")]
    pub entity_id: super::super::EntityId,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct UnsubscribeRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub entity_id: super::super::EntityId,
    #[prost(uint64, optional, tag = "3")]
    pub object_id: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRequest {
    #[prost(message, required, tag = "1")]
    pub entity_id: super::super::EntityId,
    #[prost(message, repeated, tag = "2")]
    pub field_operation: ::prost::alloc::vec::Vec<FieldOperation>,
    #[prost(bool, optional, tag = "3")]
    pub no_create: ::core::option::Option<bool>,
    #[prost(message, optional, tag = "4")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryRequest {
    #[prost(message, required, tag = "1")]
    pub entity_id: super::super::EntityId,
    #[prost(message, repeated, tag = "2")]
    pub key: ::prost::alloc::vec::Vec<FieldKey>,
    #[prost(message, optional, tag = "3")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryResponse {
    #[prost(message, repeated, tag = "2")]
    pub field: ::prost::alloc::vec::Vec<Field>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct OwnershipRequest {
    #[prost(message, required, tag = "1")]
    pub entity_id: super::super::EntityId,
    #[prost(bool, optional, tag = "2")]
    pub release_ownership: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchSubscribeRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "2")]
    pub entity_id: ::prost::alloc::vec::Vec<super::super::EntityId>,
    #[prost(fixed32, repeated, packed = "false", tag = "3")]
    pub program: ::prost::alloc::vec::Vec<u32>,
    #[prost(message, repeated, tag = "4")]
    pub key: ::prost::alloc::vec::Vec<FieldKey>,
    #[prost(uint64, optional, tag = "5")]
    pub object_id: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SubscribeResult {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
    #[prost(uint32, optional, tag = "2")]
    pub result: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchSubscribeResponse {
    #[prost(message, repeated, tag = "1")]
    pub subscribe_failed: ::prost::alloc::vec::Vec<SubscribeResult>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BatchUnsubscribeRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "2")]
    pub entity_id: ::prost::alloc::vec::Vec<super::super::EntityId>,
    #[prost(uint64, optional, tag = "3")]
    pub object_id: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RichPresenceLocalizationKey {
    #[prost(fixed32, required, tag = "1")]
    pub program: u32,
    #[prost(fixed32, required, tag = "2")]
    pub stream: u32,
    #[prost(uint32, required, tag = "3")]
    pub localization_id: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct FieldKey {
    #[prost(uint32, required, tag = "1")]
    pub program: u32,
    #[prost(uint32, required, tag = "2")]
    pub group: u32,
    #[prost(uint32, required, tag = "3")]
    pub field: u32,
    #[prost(uint64, optional, tag = "4", default = "0")]
    pub unique_id: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Field {
    #[prost(message, required, tag = "1")]
    pub key: FieldKey,
    #[prost(message, required, tag = "2")]
    pub value: super::super::Variant,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FieldOperation {
    #[prost(message, required, tag = "1")]
    pub field: Field,
    #[prost(
        enumeration = "field_operation::OperationType",
        optional,
        tag = "2",
        default = "Set"
    )]
    pub operation: ::core::option::Option<i32>,
}
pub mod field_operation {
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
    pub enum OperationType {
        Set = 0,
        Clear = 1,
    }
    impl OperationType {
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::Set => "SET",
                Self::Clear => "CLEAR",
            }
        }
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "SET" => Some(Self::Set),
                "CLEAR" => Some(Self::Clear),
                _ => None,
            }
        }
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PresenceState {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "2")]
    pub field_operation: ::prost::alloc::vec::Vec<FieldOperation>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChannelState {
    #[prost(message, optional, tag = "1")]
    pub entity_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "2")]
    pub field_operation: ::prost::alloc::vec::Vec<FieldOperation>,
    #[prost(bool, optional, tag = "3", default = "false")]
    pub healing: ::core::option::Option<bool>,
}
