#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Variant {
    #[prost(bool, optional, tag = "2")]
    pub bool_value: ::core::option::Option<bool>,
    #[prost(int64, optional, tag = "3")]
    pub int_value: ::core::option::Option<i64>,
    #[prost(double, optional, tag = "4")]
    pub float_value: ::core::option::Option<f64>,
    #[prost(string, optional, tag = "5")]
    pub string_value: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "6")]
    pub blob_value: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub message_value: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(string, optional, tag = "8")]
    pub fourcc_value: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, optional, tag = "9")]
    pub uint_value: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "10")]
    pub entity_id_value: ::core::option::Option<EntityId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Attribute {
    #[prost(string, required, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, required, tag = "2")]
    pub value: Variant,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AttributeFilter {
    #[prost(enumeration = "attribute_filter::Operation", required, tag = "1")]
    pub op: i32,
    #[prost(message, repeated, tag = "2")]
    pub attribute: ::prost::alloc::vec::Vec<Attribute>,
}
pub mod attribute_filter {
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
    pub enum Operation {
        MatchNone = 0,
        MatchAny = 1,
        MatchAll = 2,
        MatchAllMostSpecific = 3,
    }
    impl Operation {
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::MatchNone => "MATCH_NONE",
                Self::MatchAny => "MATCH_ANY",
                Self::MatchAll => "MATCH_ALL",
                Self::MatchAllMostSpecific => "MATCH_ALL_MOST_SPECIFIC",
            }
        }
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "MATCH_NONE" => Some(Self::MatchNone),
                "MATCH_ANY" => Some(Self::MatchAny),
                "MATCH_ALL" => Some(Self::MatchAll),
                "MATCH_ALL_MOST_SPECIFIC" => Some(Self::MatchAllMostSpecific),
                _ => None,
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ContentHandle {
    #[prost(fixed32, required, tag = "1")]
    pub region: u32,
    #[prost(fixed32, required, tag = "2")]
    pub usage: u32,
    #[prost(bytes = "vec", required, tag = "3")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, optional, tag = "4")]
    pub proto_url: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct EntityId {
    #[prost(fixed64, required, tag = "1")]
    pub high: u64,
    #[prost(fixed64, required, tag = "2")]
    pub low: u64,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct Identity {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<EntityId>,
    #[prost(message, optional, tag = "2")]
    pub game_account_id: ::core::option::Option<EntityId>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct BgsFieldOptions {
    #[prost(enumeration = "LogOption", optional, tag = "1")]
    pub log: ::core::option::Option<i32>,
    #[prost(bool, optional, tag = "2")]
    pub shard_key: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub fanout_key: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub client_instance_key: ::core::option::Option<bool>,
    #[prost(string, optional, tag = "5")]
    pub realized_enum: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FieldRestriction {
    #[prost(oneof = "field_restriction::Type", tags = "1, 2, 3, 4, 5, 6, 7, 8")]
    pub r#type: ::core::option::Option<field_restriction::Type>,
}
pub mod field_restriction {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Type {
        #[prost(message, tag = "1")]
        Signed(super::SignedFieldRestriction),
        #[prost(message, tag = "2")]
        Unsigned(super::UnsignedFieldRestriction),
        #[prost(message, tag = "3")]
        Float(super::FloatFieldRestriction),
        #[prost(message, tag = "4")]
        String(super::StringFieldRestriction),
        #[prost(message, tag = "5")]
        Repeated(super::RepeatedFieldRestriction),
        #[prost(message, tag = "6")]
        Message(super::MessageFieldRestriction),
        #[prost(message, tag = "7")]
        EntityId(super::EntityIdRestriction),
        #[prost(message, tag = "8")]
        Bytes(super::StringFieldRestriction),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RepeatedFieldRestriction {
    #[prost(message, optional, tag = "1")]
    pub size: ::core::option::Option<UnsignedIntRange>,
    #[prost(bool, optional, tag = "2")]
    pub unique: ::core::option::Option<bool>,
    #[prost(oneof = "repeated_field_restriction::Type", tags = "3, 4, 5, 6, 7, 8")]
    pub r#type: ::core::option::Option<repeated_field_restriction::Type>,
}
pub mod repeated_field_restriction {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Type {
        #[prost(message, tag = "3")]
        Signed(super::SignedFieldRestriction),
        #[prost(message, tag = "4")]
        Unsigned(super::UnsignedFieldRestriction),
        #[prost(message, tag = "5")]
        Float(super::FloatFieldRestriction),
        #[prost(message, tag = "6")]
        String(super::StringFieldRestriction),
        #[prost(message, tag = "7")]
        EntityId(super::EntityIdRestriction),
        #[prost(message, tag = "8")]
        Bytes(super::StringFieldRestriction),
    }
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SignedFieldRestriction {
    #[prost(message, optional, tag = "1")]
    pub limits: ::core::option::Option<SignedIntRange>,
    #[prost(sint64, repeated, packed = "false", tag = "2")]
    pub exclude: ::prost::alloc::vec::Vec<i64>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct UnsignedFieldRestriction {
    #[prost(message, optional, tag = "1")]
    pub limits: ::core::option::Option<UnsignedIntRange>,
    #[prost(uint64, repeated, packed = "false", tag = "2")]
    pub exclude: ::prost::alloc::vec::Vec<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FloatFieldRestriction {
    #[prost(message, optional, tag = "1")]
    pub limits: ::core::option::Option<FloatRange>,
    #[prost(float, repeated, packed = "false", tag = "2")]
    pub exclude: ::prost::alloc::vec::Vec<f32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct StringFieldRestriction {
    #[prost(message, optional, tag = "1")]
    pub size: ::core::option::Option<UnsignedIntRange>,
    #[prost(string, repeated, tag = "2")]
    pub exclude: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct EntityIdRestriction {
    #[prost(bool, optional, tag = "1")]
    pub needed: ::core::option::Option<bool>,
    #[prost(enumeration = "entity_id_restriction::Kind", optional, tag = "2")]
    pub kind: ::core::option::Option<i32>,
}
pub mod entity_id_restriction {
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
    pub enum Kind {
        Any = 0,
        Account = 1,
        GameAccount = 2,
        AccountOrGameAccount = 3,
        Service = 4,
        Channel = 5,
    }
    impl Kind {
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::Any => "ANY",
                Self::Account => "ACCOUNT",
                Self::GameAccount => "GAME_ACCOUNT",
                Self::AccountOrGameAccount => "ACCOUNT_OR_GAME_ACCOUNT",
                Self::Service => "SERVICE",
                Self::Channel => "CHANNEL",
            }
        }
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "ANY" => Some(Self::Any),
                "ACCOUNT" => Some(Self::Account),
                "GAME_ACCOUNT" => Some(Self::GameAccount),
                "ACCOUNT_OR_GAME_ACCOUNT" => Some(Self::AccountOrGameAccount),
                "SERVICE" => Some(Self::Service),
                "CHANNEL" => Some(Self::Channel),
                _ => None,
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct MessageFieldRestriction {
    #[prost(bool, optional, tag = "1")]
    pub needed: ::core::option::Option<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum LogOption {
    Hidden = 1,
    Hex = 2,
}
impl LogOption {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Hidden => "HIDDEN",
            Self::Hex => "HEX",
        }
    }
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "HIDDEN" => Some(Self::Hidden),
            "HEX" => Some(Self::Hex),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct BgsMessageOptions {
    #[prost(bool, optional, tag = "1")]
    pub custom_select_shard: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub custom_validator: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct BgsMethodOptions {
    #[prost(uint32, optional, tag = "1")]
    pub id: ::core::option::Option<u32>,
    #[prost(enumeration = "ClientIdentityRoutingType", optional, tag = "2")]
    pub client_identity_routing: ::core::option::Option<i32>,
    #[prost(bool, optional, tag = "3")]
    pub enable_fanout: ::core::option::Option<bool>,
    #[prost(string, optional, tag = "4")]
    pub legacy_fanout_replacement: ::core::option::Option<
        ::prost::alloc::string::String,
    >,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct UnsignedIntRange {
    #[prost(uint64, optional, tag = "1")]
    pub min: ::core::option::Option<u64>,
    #[prost(uint64, optional, tag = "2")]
    pub max: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SignedIntRange {
    #[prost(int64, optional, tag = "1")]
    pub min: ::core::option::Option<i64>,
    #[prost(int64, optional, tag = "2")]
    pub max: ::core::option::Option<i64>,
}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct FloatRange {
    #[prost(float, optional, tag = "1")]
    pub min: ::core::option::Option<f32>,
    #[prost(float, optional, tag = "2")]
    pub max: ::core::option::Option<f32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ClientIdentityRoutingType {
    ClientIdentityRoutingDisabled = 0,
    ClientIdentityRoutingBattleNetAccount = 1,
    ClientIdentityRoutingGameAccount = 2,
    ClientIdentityRoutingInstanceId = 3,
}
impl ClientIdentityRoutingType {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::ClientIdentityRoutingDisabled => "CLIENT_IDENTITY_ROUTING_DISABLED",
            Self::ClientIdentityRoutingBattleNetAccount => {
                "CLIENT_IDENTITY_ROUTING_BATTLE_NET_ACCOUNT"
            }
            Self::ClientIdentityRoutingGameAccount => {
                "CLIENT_IDENTITY_ROUTING_GAME_ACCOUNT"
            }
            Self::ClientIdentityRoutingInstanceId => {
                "CLIENT_IDENTITY_ROUTING_INSTANCE_ID"
            }
        }
    }
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "CLIENT_IDENTITY_ROUTING_DISABLED" => {
                Some(Self::ClientIdentityRoutingDisabled)
            }
            "CLIENT_IDENTITY_ROUTING_BATTLE_NET_ACCOUNT" => {
                Some(Self::ClientIdentityRoutingBattleNetAccount)
            }
            "CLIENT_IDENTITY_ROUTING_GAME_ACCOUNT" => {
                Some(Self::ClientIdentityRoutingGameAccount)
            }
            "CLIENT_IDENTITY_ROUTING_INSTANCE_ID" => {
                Some(Self::ClientIdentityRoutingInstanceId)
            }
            _ => None,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct BgsServiceOptions {
    #[prost(string, optional, tag = "1")]
    pub descriptor_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "4")]
    pub version: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "5")]
    pub shard_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, optional, tag = "6")]
    pub resolve_client_instance: ::core::option::Option<bool>,
    #[prost(enumeration = "bgs_service_options::ServiceType", optional, tag = "7")]
    pub r#type: ::core::option::Option<i32>,
}
pub mod bgs_service_options {
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
    pub enum ServiceType {
        Rpc = 0,
        Event = 1,
        EventBroadcast = 2,
    }
    impl ServiceType {
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::Rpc => "SERVICE_TYPE_RPC",
                Self::Event => "SERVICE_TYPE_EVENT",
                Self::EventBroadcast => "SERVICE_TYPE_EVENT_BROADCAST",
            }
        }
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "SERVICE_TYPE_RPC" => Some(Self::Rpc),
                "SERVICE_TYPE_EVENT" => Some(Self::Event),
                "SERVICE_TYPE_EVENT_BROADCAST" => Some(Self::EventBroadcast),
                _ => None,
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SdkServiceOptions {
    #[prost(bool, optional, tag = "1")]
    pub inbound: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub outbound: ::core::option::Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub use_client_id: ::core::option::Option<bool>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct Invitation {
    #[prost(fixed64, required, tag = "1")]
    pub id: u64,
    #[prost(message, required, tag = "2")]
    pub inviter_identity: Identity,
    #[prost(message, required, tag = "3")]
    pub invitee_identity: Identity,
    #[prost(string, optional, tag = "4")]
    pub inviter_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "5")]
    pub invitee_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "6")]
    pub invitation_message: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, optional, tag = "7")]
    pub creation_time: ::core::option::Option<u64>,
    #[prost(uint64, optional, tag = "8")]
    pub expiration_time: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct InvitationParams {
    #[deprecated]
    #[prost(string, optional, tag = "1")]
    pub invitation_message: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, optional, tag = "2")]
    pub expiration_time: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum InvitationRemovedReason {
    Accepted = 0,
    Declined = 1,
    Revoked = 2,
    Ignored = 3,
    Expired = 4,
    Canceled = 5,
}
impl InvitationRemovedReason {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Accepted => "INVITATION_REMOVED_REASON_ACCEPTED",
            Self::Declined => "INVITATION_REMOVED_REASON_DECLINED",
            Self::Revoked => "INVITATION_REMOVED_REASON_REVOKED",
            Self::Ignored => "INVITATION_REMOVED_REASON_IGNORED",
            Self::Expired => "INVITATION_REMOVED_REASON_EXPIRED",
            Self::Canceled => "INVITATION_REMOVED_REASON_CANCELED",
        }
    }
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "INVITATION_REMOVED_REASON_ACCEPTED" => Some(Self::Accepted),
            "INVITATION_REMOVED_REASON_DECLINED" => Some(Self::Declined),
            "INVITATION_REMOVED_REASON_REVOKED" => Some(Self::Revoked),
            "INVITATION_REMOVED_REASON_IGNORED" => Some(Self::Ignored),
            "INVITATION_REMOVED_REASON_EXPIRED" => Some(Self::Expired),
            "INVITATION_REMOVED_REASON_CANCELED" => Some(Self::Canceled),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SuggestionRemovedReason {
    Approved = 0,
    Declined = 1,
    Expired = 2,
    Canceled = 3,
}
impl SuggestionRemovedReason {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Approved => "SUGGESTION_REMOVED_REASON_APPROVED",
            Self::Declined => "SUGGESTION_REMOVED_REASON_DECLINED",
            Self::Expired => "SUGGESTION_REMOVED_REASON_EXPIRED",
            Self::Canceled => "SUGGESTION_REMOVED_REASON_CANCELED",
        }
    }
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SUGGESTION_REMOVED_REASON_APPROVED" => Some(Self::Approved),
            "SUGGESTION_REMOVED_REASON_DECLINED" => Some(Self::Declined),
            "SUGGESTION_REMOVED_REASON_EXPIRED" => Some(Self::Expired),
            "SUGGESTION_REMOVED_REASON_CANCELED" => Some(Self::Canceled),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct NoResponse {}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct Address {
    #[prost(string, required, tag = "1")]
    pub address: ::prost::alloc::string::String,
    #[prost(uint32, optional, tag = "2")]
    pub port: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProcessId {
    #[prost(uint32, required, tag = "1")]
    pub label: u32,
    #[prost(uint32, required, tag = "2")]
    pub epoch: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ObjectAddress {
    #[prost(message, required, tag = "1")]
    pub host: ProcessId,
    #[prost(uint64, optional, tag = "2", default = "0")]
    pub object_id: ::core::option::Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct NoData {}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ErrorInfo {
    #[prost(message, required, tag = "1")]
    pub object_address: ObjectAddress,
    #[prost(uint32, required, tag = "2")]
    pub status: u32,
    #[prost(uint32, required, tag = "3")]
    pub service_hash: u32,
    #[prost(uint32, required, tag = "4")]
    pub method_id: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct FanoutTarget {
    #[prost(string, optional, tag = "1")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub key: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
    #[prost(uint64, optional, tag = "3")]
    pub object_id: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Header {
    #[prost(uint32, required, tag = "1")]
    pub service_id: u32,
    #[prost(uint32, optional, tag = "2")]
    pub method_id: ::core::option::Option<u32>,
    #[prost(uint32, required, tag = "3")]
    pub token: u32,
    #[prost(uint64, optional, tag = "4", default = "0")]
    pub object_id: ::core::option::Option<u64>,
    #[prost(uint32, optional, tag = "5", default = "0")]
    pub size: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "6", default = "0")]
    pub status: ::core::option::Option<u32>,
    #[prost(message, repeated, tag = "7")]
    pub error: ::prost::alloc::vec::Vec<ErrorInfo>,
    #[prost(uint64, optional, tag = "8")]
    pub timeout: ::core::option::Option<u64>,
    #[prost(bool, optional, tag = "9")]
    pub is_response: ::core::option::Option<bool>,
    #[prost(message, repeated, tag = "10")]
    pub forward_targets: ::prost::alloc::vec::Vec<ProcessId>,
    #[prost(fixed32, optional, tag = "11")]
    pub service_hash: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "13")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(message, repeated, tag = "14")]
    pub fanout_target: ::prost::alloc::vec::Vec<FanoutTarget>,
    #[prost(string, repeated, tag = "15")]
    pub client_id_fanout_target: ::prost::alloc::vec::Vec<
        ::prost::alloc::string::String,
    >,
    #[prost(bytes = "vec", optional, tag = "16")]
    pub client_record: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct KafkaHeader {
    #[prost(fixed32, optional, tag = "1")]
    pub service_hash: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub method_id: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub token: ::core::option::Option<u32>,
    #[prost(uint64, optional, tag = "4", default = "0")]
    pub object_id: ::core::option::Option<u64>,
    #[prost(uint32, optional, tag = "5", default = "0")]
    pub size: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "6", default = "0")]
    pub status: ::core::option::Option<u32>,
    #[prost(uint64, optional, tag = "7")]
    pub timeout: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "8")]
    pub forward_target: ::core::option::Option<ProcessId>,
    #[prost(string, optional, tag = "9")]
    pub return_topic: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "11")]
    pub client_id: ::core::option::Option<::prost::alloc::string::String>,
}
