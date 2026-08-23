#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct FrameAccountProgram {
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeader {
    #[bsn(name = "m_data")]
    pub data: super::frame::FrameHeaderData,
}

#[derive(Clone, Debug)]
pub enum FrameHeaderData {
    Content(super::frame::FrameHeaderDataContent),
    Route(super::frame::FrameHeaderDataRoute),
    Target(super::frame::FrameHeaderDataTarget),
    Correlation(super::frame::FrameHeaderDataCorrelation),
    Client(super::frame::FrameHeaderDataClient),
    Service(super::frame::FrameHeaderDataService),
    Error(super::frame::FrameHeaderDataError),
    Replicate(super::frame::FrameHeaderDataReplicate),
    Timestamp(i32),
    Stream(super::frame::FrameHeaderDataStream),
    TraceRoute(super::frame::FrameHeaderDataTraceRoute),
}
impl superiority_core::bsn::FromBsn for FrameHeaderData {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for FrameHeaderData, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Content(<super::frame::FrameHeaderDataContent as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Route(<super::frame::FrameHeaderDataRoute as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Target(<super::frame::FrameHeaderDataTarget as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Correlation(<super::frame::FrameHeaderDataCorrelation as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            4i128 => Ok(Self::Client(<super::frame::FrameHeaderDataClient as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            5i128 => Ok(Self::Service(<super::frame::FrameHeaderDataService as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            6i128 => Ok(Self::Error(<super::frame::FrameHeaderDataError as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            7i128 => Ok(Self::Replicate(<super::frame::FrameHeaderDataReplicate as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            8i128 => Ok(Self::Timestamp(<i32 as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            9i128 => Ok(Self::Stream(<super::frame::FrameHeaderDataStream as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            10i128 => Ok(Self::TraceRoute(<super::frame::FrameHeaderDataTraceRoute as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a FrameHeaderData variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataClient {
    #[bsn(name = "m_account")]
    pub account: u32,
    #[bsn(name = "m_gameAccount")]
    pub game_account: super::gameaccount::GameAccountHandle,
    #[bsn(name = "m_toonHandle")]
    pub toon_handle: super::toon::ToonHandle,
    #[bsn(name = "m_client")]
    pub client: u32,
    #[bsn(name = "m_obfuscationSeed")]
    pub obfuscation_seed: u32,
    #[bsn(name = "m_addressPort")]
    pub address_port: super::ip4::IP4AddressPort,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataContent {
    #[bsn(name = "m_size")]
    pub size: u32,
    #[bsn(name = "m_encoding")]
    pub encoding: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataCorrelation {
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_reply")]
    pub reply: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataError {
    #[bsn(name = "m_result")]
    pub result: u16,
    #[bsn(name = "m_message")]
    pub message: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataReplicate {
    #[bsn(name = "m_command")]
    pub command: super::frame::FrameReplicationCommandEnum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataRoute {
    #[bsn(name = "m_name")]
    pub name: FourCc,
    #[bsn(name = "m_hash")]
    pub hash: u32,
    #[bsn(name = "m_command")]
    pub command: u8,
    #[bsn(name = "m_node")]
    pub node: Option<super::frame::FrameNode>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataService {
    #[bsn(name = "m_label")]
    pub label: u32,
    #[bsn(name = "m_type")]
    pub type_: u32,
    #[bsn(name = "m_epoch")]
    pub epoch: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataStream {
    #[bsn(name = "m_sequenceId")]
    pub sequence_id: u16,
    #[bsn(name = "m_more")]
    pub more: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataTarget {
    #[bsn(name = "m_type")]
    pub type_: super::frame::FrameTargetTypeEnum,
    #[bsn(name = "m_ids")]
    pub ids: super::frame::FrameHeaderDataTargetIds,
}

#[derive(Clone, Debug)]
pub enum FrameHeaderDataTargetIds {
    ClientId(Vec<u32>),
    AccountId(Vec<u32>),
    ProgramId(Vec<FourCc>),
    GameAccount(Vec<super::gameaccount::GameAccountHandle>),
    AccountProgram(Vec<super::frame::FrameAccountProgram>),
    MatchmakerQueue(Vec<super::matchmaker::MatchMakerHandle>),
    ToonHandle(Vec<super::toon::ToonHandle>),
}
impl superiority_core::bsn::FromBsn for FrameHeaderDataTargetIds {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for FrameHeaderDataTargetIds, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::ClientId(
                <Vec<u32> as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            1i128 => Ok(Self::AccountId(
                <Vec<u32> as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            2i128 => Ok(Self::ProgramId(
                <Vec<FourCc> as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            3i128 => Ok(Self::GameAccount(<Vec<
                super::gameaccount::GameAccountHandle,
            > as superiority_core::bsn::FromBsn>::from_bsn(
                inner
            )?)),
            4i128 => Ok(Self::AccountProgram(<Vec<
                super::frame::FrameAccountProgram,
            > as superiority_core::bsn::FromBsn>::from_bsn(
                inner
            )?)),
            5i128 => Ok(Self::MatchmakerQueue(<Vec<
                super::matchmaker::MatchMakerHandle,
            > as superiority_core::bsn::FromBsn>::from_bsn(
                inner
            )?)),
            6i128 => Ok(Self::ToonHandle(
                <Vec<super::toon::ToonHandle> as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a FrameHeaderDataTargetIds variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameHeaderDataTraceRoute {
    #[bsn(name = "m_services")]
    pub services: Vec<super::frame::FrameNode>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FrameNode {
    #[bsn(name = "m_label")]
    pub label: u32,
    #[bsn(name = "m_epoch")]
    pub epoch: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameReplicationCommandEnum {
    REPLICATECOPYONLINE,
    REPLICATECOPYOFFLINE,
}
impl superiority_core::bsn::FromBsn for FrameReplicationCommandEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            1i128 => Ok(Self::REPLICATECOPYONLINE),
            2i128 => Ok(Self::REPLICATECOPYOFFLINE),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid FrameReplicationCommandEnum"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameTargetTypeEnum {
    CLIENTID,
    ACCOUNTID,
    PROGRAMID,
    GAMEACCOUNTINGAME,
    GAMEACCOUNT,
    ACCOUNTPROGRAM,
    GAMEACCOUNTINSUNKEN,
    ACCOUNTPROGRAMINSUNKEN,
    MATCHMAKERQUEUE,
    TOONHANDLEINSUNKEN,
    TOONHANDLE,
}
impl superiority_core::bsn::FromBsn for FrameTargetTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            1i128 => Ok(Self::CLIENTID),
            2i128 => Ok(Self::ACCOUNTID),
            3i128 => Ok(Self::PROGRAMID),
            4i128 => Ok(Self::GAMEACCOUNTINGAME),
            5i128 => Ok(Self::GAMEACCOUNT),
            6i128 => Ok(Self::ACCOUNTPROGRAM),
            7i128 => Ok(Self::GAMEACCOUNTINSUNKEN),
            8i128 => Ok(Self::ACCOUNTPROGRAMINSUNKEN),
            9i128 => Ok(Self::MATCHMAKERQUEUE),
            10i128 => Ok(Self::TOONHANDLEINSUNKEN),
            11i128 => Ok(Self::TOONHANDLE),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid FrameTargetTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameTypeEnum {
    CSROUTED,
    SSROUTED,
    SSBROADCAST,
    SSERROR,
    SCERROR,
    SCROUTED,
    SCBROADCAST,
}
impl superiority_core::bsn::FromBsn for FrameTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            129i128 => Ok(Self::CSROUTED),
            33i128 => Ok(Self::SSROUTED),
            34i128 => Ok(Self::SSBROADCAST),
            35i128 => Ok(Self::SSERROR),
            65i128 => Ok(Self::SCERROR),
            66i128 => Ok(Self::SCROUTED),
            67i128 => Ok(Self::SCBROADCAST),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid FrameTypeEnum"
            ))),
        }
    }
}
