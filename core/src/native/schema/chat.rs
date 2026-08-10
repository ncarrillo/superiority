#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatChannelType2Enum {
    UNKNOWN,
    NAMED,
    CONVO,
    PARTY,
    PUBLIC,
    CLUB,
}
impl sc2_core::bsn::FromBsn for ChatChannelType2Enum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNKNOWN),
            1i128 => Ok(Self::NAMED),
            2i128 => Ok(Self::CONVO),
            3i128 => Ok(Self::PARTY),
            4i128 => Ok(Self::PUBLIC),
            5i128 => Ok(Self::CLUB),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ChatChannelType2Enum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatFullName {
    #[bsn(name = "m_shard")]
    pub shard: Option<u32>,
}

#[derive(Clone, Debug)]
pub enum ChatJoinNotifyResult2 {
    Success(super::chat::ChatJoinNotifyResult2Success),
    Failed(super::chat::ChatJoinNotifyResult2Failed),
}
impl sc2_core::bsn::FromBsn for ChatJoinNotifyResult2 {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ChatJoinNotifyResult2, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(
                <super::chat::ChatJoinNotifyResult2Success as sc2_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            1i128 => Ok(Self::Failed(
                <super::chat::ChatJoinNotifyResult2Failed as sc2_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a ChatJoinNotifyResult2 variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatJoinNotifyResult2Failed {
    #[bsn(name = "m_reason")]
    pub reason: u16,
    #[bsn(name = "m_channelType")]
    pub channel_type: Option<super::chat::ChatChannelType2Enum>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatJoinNotifyResult2Success {
    #[bsn(name = "m_memberHandle")]
    pub member_handle: u32,
    #[bsn(name = "m_channelIndex")]
    pub channel_index: u8,
    #[bsn(name = "m_voiceSessionId")]
    pub voice_session_id: super::comsat::ComSatSessionIdUnion,
    #[bsn(name = "m_channelType")]
    pub channel_type: super::chat::ChatChannelType2Enum,
    #[bsn(name = "m_name")]
    pub name: Option<super::conference::ConferenceShardName>,
    #[bsn(name = "m_config")]
    pub config: Option<super::conference::ConferenceConferenceConfiguration>,
    #[bsn(name = "m_inviterMemberHandle")]
    pub inviter_member_handle: Option<u32>,
}

#[derive(Clone, Debug)]
pub enum ChatMemberStatusSingle {
    Other(super::chat::ChatMemberStatusSingleOther),
    Party(super::chat::ChatMemberStatusSingleParty),
    TalkerNetworkId(super::chat::ChatMemberStatusSingleTalkerNetworkId),
    TalkerInfo(super::chat::ChatMemberStatusSingleTalkerInfo),
    VoiceEnabled(bool),
    Display(super::chat::ChatMemberStatusSingleDisplay),
    Active(bool),
    Sentinel(()),
}
impl sc2_core::bsn::FromBsn for ChatMemberStatusSingle {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ChatMemberStatusSingle, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Other(<super::chat::ChatMemberStatusSingleOther as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Party(<super::chat::ChatMemberStatusSingleParty as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::TalkerNetworkId(<super::chat::ChatMemberStatusSingleTalkerNetworkId as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::TalkerInfo(<super::chat::ChatMemberStatusSingleTalkerInfo as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            4i128 => Ok(Self::VoiceEnabled(<bool as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            5i128 => Ok(Self::Display(<super::chat::ChatMemberStatusSingleDisplay as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            6i128 => Ok(Self::Active(<bool as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            7i128 => Ok(Self::Sentinel(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ChatMemberStatusSingle variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMemberStatusSingleDisplay {
    #[bsn(name = "m_toonName")]
    pub toon_name: super::toon::ToonFullName,
}

#[derive(Clone, Debug)]
pub enum ChatMemberStatusSingleOther {
    ClubData(super::chat::ChatMemberStatusSingleOtherClubData),
    Licenses(Vec<u32>),
}
impl sc2_core::bsn::FromBsn for ChatMemberStatusSingleOther {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ChatMemberStatusSingleOther, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::ClubData(<super::chat::ChatMemberStatusSingleOtherClubData as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Licenses(<Vec<u32> as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ChatMemberStatusSingleOther variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMemberStatusSingleOtherClubData {
    #[bsn(name = "m_rank")]
    pub rank: super::club::ClubMemberRankEnum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMemberStatusSingleParty {
    #[bsn(name = "m_partyStatus")]
    pub party_status: super::chat::ChatPartyMemberStatusEnum,
    #[bsn(name = "m_expansionLevel")]
    pub expansion_level: Option<super::starcraft2::Starcraft2ExpansionLevelEnum>,
    #[bsn(name = "m_captain")]
    pub captain: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMemberStatusSingleTalkerInfo {
    #[bsn(name = "m_talkerInfo")]
    pub talker_info: super::comsat::ClientComSatTalkerInfo,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMemberStatusSingleTalkerNetworkId {
    #[bsn(name = "m_talkerNetworkId")]
    pub talker_network_id: u8,
}

#[derive(Clone, Debug)]
pub enum ChatMembershipChange {
    LeaveChannel(super::chat::ChatMembershipChangeLeaveChannel),
    JoinChannel(super::chat::ChatMembershipChangeJoinChannel),
    UpdateStatus(super::chat::ChatMembershipChangeUpdateStatus),
}
impl sc2_core::bsn::FromBsn for ChatMembershipChange {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ChatMembershipChange, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::LeaveChannel(<super::chat::ChatMembershipChangeLeaveChannel as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::JoinChannel(<super::chat::ChatMembershipChangeJoinChannel as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::UpdateStatus(<super::chat::ChatMembershipChangeUpdateStatus as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ChatMembershipChange variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMembershipChangeJoinChannel {
    #[bsn(name = "m_memberHandle")]
    pub member_handle: u32,
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_memberStatus")]
    pub member_status: Vec<super::chat::ChatMemberStatusSingle>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMembershipChangeLeaveChannel {
    #[bsn(name = "m_memberHandle")]
    pub member_handle: u32,
    #[bsn(name = "m_reason")]
    pub reason: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ChatMembershipChangeUpdateStatus {
    #[bsn(name = "m_memberHandle")]
    pub member_handle: u32,
    #[bsn(name = "m_memberStatus")]
    pub member_status: super::chat::ChatMemberStatusSingle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatPartyMemberStatusEnum {
    INVALID,
    ONLINE,
    OFFLINE,
    INVITED,
}
impl sc2_core::bsn::FromBsn for ChatPartyMemberStatusEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::INVALID),
            1i128 => Ok(Self::ONLINE),
            2i128 => Ok(Self::OFFLINE),
            3i128 => Ok(Self::INVITED),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ChatPartyMemberStatusEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatChannelList {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatChannelListRequest {
    #[bsn(name = "ChannelList")]
    pub channel_list: super::chat::ClientChatChannelList,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatChannelListResponse {
    #[bsn(name = "ChannelList")]
    pub channel_list: super::chat::ClientChatChannelList,
    #[bsn(name = "m_channels")]
    pub channels: Vec<super::chat::ChatFullName>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatConferenceDescriptions {
    #[bsn(name = "m_list")]
    pub list: Vec<super::conference::ConferenceFullConferenceDescription>,
    #[bsn(name = "m_isLast")]
    pub is_last: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatEnumConferenceDescriptions {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatInviteNotify {
    #[bsn(name = "m_channelIndex")]
    pub channel_index: u8,
    #[bsn(name = "m_inviterPresence")]
    pub inviter_presence: u32,
    #[bsn(name = "m_channelType")]
    pub channel_type: super::chat::ChatChannelType2Enum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatJoinNotify2 {
    #[bsn(name = "m_token")]
    pub token: Option<u32>,
    #[bsn(name = "m_result")]
    pub result: super::chat::ChatJoinNotifyResult2,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatJoinRequest2 {
    #[bsn(name = "m_token")]
    pub token: u32,
    #[bsn(name = "m_key")]
    pub key: super::conference::ConferenceLocatorKey,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatMembershipChangeNotify {
    #[bsn(name = "m_endOfInitial")]
    pub end_of_initial: bool,
    #[bsn(name = "m_channelIndex")]
    pub channel_index: u8,
    #[bsn(name = "m_changes")]
    pub changes: Vec<super::chat::ChatMembershipChange>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatMessage {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatMessageRecv {
    #[bsn(name = "Message")]
    pub message: super::chat::ClientChatMessage,
    #[bsn(name = "m_channelIndex")]
    pub channel_index: u8,
    #[bsn(name = "m_memberHandle")]
    pub member_handle: u32,
    #[bsn(name = "m_body")]
    pub body: String,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatMessageSend {
    #[bsn(name = "Message")]
    pub message: super::chat::ClientChatMessage,
    #[bsn(name = "m_channelIndex")]
    pub channel_index: u8,
    #[bsn(name = "m_body")]
    pub body: String,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatWhisper {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatWhisperEcho {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatWhisperEchoRecv {
    #[bsn(name = "WhisperEcho")]
    pub whisper_echo: super::chat::ClientChatWhisperEcho,
    #[bsn(name = "m_sender")]
    pub sender: super::toon::ToonFullName,
    #[bsn(name = "m_body")]
    pub body: String,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientChatWhisperRecv {
    #[bsn(name = "Whisper")]
    pub whisper: super::chat::ClientChatWhisper,
    #[bsn(name = "m_sender")]
    pub sender: super::toon::ToonFullName,
    #[bsn(name = "m_body")]
    pub body: String,
}
