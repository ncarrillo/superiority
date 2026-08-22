#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ChannelId {
    #[prost(uint32, optional, tag = "1")]
    pub r#type: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "2")]
    pub host: ::core::option::Option<super::super::ProcessId>,
    #[prost(fixed32, optional, tag = "3")]
    pub id: ::core::option::Option<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RemoveMemberRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub member_id: super::super::EntityId,
    #[prost(uint32, optional, tag = "3")]
    pub reason: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendMessageRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub message: Message,
    #[prost(uint64, optional, tag = "3", default = "0")]
    pub required_privileges: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateChannelStateRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub state_change: ChannelState,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateMemberStateRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "2")]
    pub state_change: ::prost::alloc::vec::Vec<Member>,
    #[prost(uint32, repeated, tag = "3")]
    pub removed_role: ::prost::alloc::vec::Vec<u32>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct DissolveRequest {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(uint32, optional, tag = "2")]
    pub reason: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct JoinNotification {
    #[prost(message, optional, tag = "1")]
    pub self_: ::core::option::Option<Member>,
    #[prost(message, repeated, tag = "2")]
    pub member: ::prost::alloc::vec::Vec<Member>,
    #[prost(message, required, tag = "3")]
    pub channel_state: ChannelState,
    #[prost(message, optional, tag = "4")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "5")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
    #[prost(message, optional, tag = "6")]
    pub presence_subscriber_id: ::core::option::Option<
        super::super::account::v1::AccountId,
    >,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MemberAddedNotification {
    #[prost(message, required, tag = "1")]
    pub member: Member,
    #[prost(message, optional, tag = "2")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "3")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct LeaveNotification {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[deprecated]
    #[prost(message, required, tag = "2")]
    pub member_id: super::super::EntityId,
    #[prost(uint32, optional, tag = "3")]
    pub reason: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "4")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "5")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct MemberRemovedNotification {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub member_id: super::super::EntityId,
    #[prost(uint32, optional, tag = "3")]
    pub reason: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "4")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "5")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendMessageNotification {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub message: Message,
    #[prost(uint64, optional, tag = "3", default = "0")]
    pub required_privileges: ::core::option::Option<u64>,
    #[prost(string, optional, tag = "4")]
    pub battle_tag: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(message, optional, tag = "5")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "6")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateChannelStateNotification {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, required, tag = "2")]
    pub state_change: ChannelState,
    #[prost(message, optional, tag = "3")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "4")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
    #[prost(message, optional, tag = "5")]
    pub presence_subscriber_id: ::core::option::Option<
        super::super::account::v1::AccountId,
    >,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateMemberStateNotification {
    #[prost(message, optional, tag = "1")]
    pub agent_id: ::core::option::Option<super::super::EntityId>,
    #[prost(message, repeated, tag = "2")]
    pub state_change: ::prost::alloc::vec::Vec<Member>,
    #[prost(message, optional, tag = "4")]
    pub channel_id: ::core::option::Option<ChannelId>,
    #[prost(message, optional, tag = "5")]
    pub subscriber_id: ::core::option::Option<SubscriberId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Message {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListChannelsOptions {
    #[prost(uint32, optional, tag = "1", default = "0")]
    pub start_index: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "2", default = "16")]
    pub max_results: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(fixed32, optional, tag = "4")]
    pub program: ::core::option::Option<u32>,
    #[prost(fixed32, optional, tag = "5")]
    pub locale: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "6")]
    pub capacity_full: ::core::option::Option<u32>,
    #[prost(message, required, tag = "7")]
    pub attribute_filter: super::super::AttributeFilter,
    #[prost(string, optional, tag = "8")]
    pub channel_type: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChannelDescription {
    #[prost(message, required, tag = "1")]
    pub channel_id: super::super::EntityId,
    #[prost(uint32, optional, tag = "2")]
    pub current_members: ::core::option::Option<u32>,
    #[prost(message, optional, tag = "3")]
    pub state: ::core::option::Option<ChannelState>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChannelInfo {
    #[prost(message, required, tag = "1")]
    pub description: ChannelDescription,
    #[prost(message, repeated, tag = "2")]
    pub member: ::prost::alloc::vec::Vec<Member>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChannelState {
    #[prost(uint32, optional, tag = "1")]
    pub max_members: ::core::option::Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub min_members: ::core::option::Option<u32>,
    #[prost(message, repeated, tag = "3")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(message, repeated, tag = "4")]
    pub invitation: ::prost::alloc::vec::Vec<super::super::Invitation>,
    #[prost(uint32, optional, tag = "6")]
    pub reason: ::core::option::Option<u32>,
    #[prost(
        enumeration = "channel_state::PrivacyLevel",
        optional,
        tag = "7",
        default = "Open"
    )]
    pub privacy_level: ::core::option::Option<i32>,
    #[prost(string, optional, tag = "8")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "10", default = "default")]
    pub channel_type: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(fixed32, optional, tag = "11")]
    pub program: ::core::option::Option<u32>,
    #[prost(bool, optional, tag = "13", default = "true")]
    pub subscribe_to_presence: ::core::option::Option<bool>,
}
pub mod channel_state {
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
    pub enum PrivacyLevel {
        Open = 1,
        OpenInvitationAndFriend = 2,
        OpenInvitation = 3,
        Closed = 4,
    }
    impl PrivacyLevel {
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::Open => "PRIVACY_LEVEL_OPEN",
                Self::OpenInvitationAndFriend => {
                    "PRIVACY_LEVEL_OPEN_INVITATION_AND_FRIEND"
                }
                Self::OpenInvitation => "PRIVACY_LEVEL_OPEN_INVITATION",
                Self::Closed => "PRIVACY_LEVEL_CLOSED",
            }
        }
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "PRIVACY_LEVEL_OPEN" => Some(Self::Open),
                "PRIVACY_LEVEL_OPEN_INVITATION_AND_FRIEND" => {
                    Some(Self::OpenInvitationAndFriend)
                }
                "PRIVACY_LEVEL_OPEN_INVITATION" => Some(Self::OpenInvitation),
                "PRIVACY_LEVEL_CLOSED" => Some(Self::Closed),
                _ => None,
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct MemberAccountInfo {
    #[prost(string, optional, tag = "3")]
    pub battle_tag: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MemberState {
    #[prost(message, repeated, tag = "1")]
    pub attribute: ::prost::alloc::vec::Vec<super::super::Attribute>,
    #[prost(uint32, repeated, tag = "2")]
    pub role: ::prost::alloc::vec::Vec<u32>,
    #[prost(uint64, optional, tag = "3", default = "0")]
    pub privileges: ::core::option::Option<u64>,
    #[prost(message, optional, tag = "4")]
    pub info: ::core::option::Option<MemberAccountInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Member {
    #[prost(message, required, tag = "1")]
    pub identity: super::super::Identity,
    #[prost(message, required, tag = "2")]
    pub state: MemberState,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct SubscriberId {
    #[deprecated]
    #[prost(message, optional, tag = "1")]
    pub account: ::core::option::Option<super::super::account::v1::AccountId>,
    #[prost(message, optional, tag = "2")]
    pub game_account: ::core::option::Option<
        super::super::account::v1::GameAccountHandle,
    >,
    #[prost(message, optional, tag = "3")]
    pub process: ::core::option::Option<super::super::ProcessId>,
}
