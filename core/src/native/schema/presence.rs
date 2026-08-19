#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPresenceStatisticsSubscribe {
    #[bsn(name = "m_on")]
    pub on: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPresenceStatisticsUpdate {
    #[bsn(name = "m_statistics")]
    pub statistics: Vec<super::statistics::StatisticsClientValue>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPresenceTemporaryPresence {
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPresenceTemporaryPresenceRequest {
    #[bsn(name = "TemporaryPresence")]
    pub temporary_presence: super::presence::ClientPresenceTemporaryPresence,
    #[bsn(name = "m_toonList")]
    pub toon_list: Vec<super::toon::ToonHandle>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPresenceTemporaryPresenceResponse {
    #[bsn(name = "TemporaryPresence")]
    pub temporary_presence: super::presence::ClientPresenceTemporaryPresence,
    #[bsn(name = "m_result")]
    pub result: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceCustomMessage {
    #[bsn(name = "m_msg")]
    pub msg: String,
    #[bsn(name = "m_time")]
    pub time: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceFieldSpec {
    #[bsn(name = "m_id")]
    pub id: super::presence::PresenceTypeEnumEnum,
    #[bsn(name = "m_writable")]
    pub writable: bool,
    #[bsn(name = "m_ephemeral")]
    pub ephemeral: bool,
    #[bsn(name = "m_serverOnly")]
    pub server_only: bool,
    #[bsn(name = "m_clientOnly")]
    pub client_only: bool,
    #[bsn(name = "m_size")]
    pub size: super::presence::PresenceFieldSpecSize,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceFieldSpecAnnounceEntry {
    #[bsn(name = "m_handle")]
    pub handle: u32,
    #[bsn(name = "m_spec")]
    pub spec: super::presence::PresenceFieldSpec,
}

#[derive(Clone, Debug)]
pub enum PresenceFieldSpecSize {
    Fixed(u16),
    Variable(()),
}
impl sc2_core::bsn::FromBsn for PresenceFieldSpecSize {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => return Err(sc2_core::Error::BsnWire(format!("expected a choice for PresenceFieldSpecSize, found {other:?}"))),
        };
        match index {
            0i128 => Ok(Self::Fixed(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Variable(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a PresenceFieldSpecSize variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceSharedPacketsFieldSpecAnnounce {
    #[bsn(name = "m_list")]
    pub list: Vec<super::presence::PresenceFieldSpecAnnounceEntry>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceSharedPacketsLevel0Info {
    #[bsn(name = "m_target")]
    pub target: u32,
    #[bsn(name = "m_isLastPacket")]
    pub is_last_packet: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceSharedPacketsUpdateBase {
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceSharedPacketsUpdateNotify {
    #[bsn(name = "UpdateBase")]
    pub update_base: super::presence::PresenceSharedPacketsUpdateBase,
    #[bsn(name = "m_idLocal")]
    pub id_local: u32,
    #[bsn(name = "m_idMaster")]
    pub id_master: u32,
    #[bsn(name = "m_online")]
    pub online: u8,
    #[bsn(name = "m_serverOnly")]
    pub server_only: bool,
    #[bsn(name = "m_update")]
    pub update: super::presence::PresenceUpdate,
    #[bsn(name = "m_level0")]
    pub level0: Option<super::presence::PresenceSharedPacketsLevel0Info>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresenceTypeEnumEnum {
    UNKNOWN,
    U8,
    S8,
    U16,
    S16,
    U32,
    S32,
    U64,
    S64,
    FLOAT32,
    FLOAT64,
    BOOL,
    FOURCC,
    STRINGLITERAL,
    STRINGTABLEENTRY,
    IMAGETABLEENTRY,
    OPAQUEDATA,
    TOONFULLNAME,
    ACCOUNTNAME,
    PROFILEADDRESS,
    S2GAMEINFO,
    ACCOUNTINFO,
    TOONHANDLE,
    GAMEACCOUNTHANDLE,
    ACHIEVEMENT,
    ACCOUNTNICKNAME,
    MAX,
}
impl sc2_core::bsn::FromBsn for PresenceTypeEnumEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNKNOWN),
            1i128 => Ok(Self::U8),
            2i128 => Ok(Self::S8),
            3i128 => Ok(Self::U16),
            4i128 => Ok(Self::S16),
            5i128 => Ok(Self::U32),
            6i128 => Ok(Self::S32),
            7i128 => Ok(Self::U64),
            8i128 => Ok(Self::S64),
            9i128 => Ok(Self::FLOAT32),
            10i128 => Ok(Self::FLOAT64),
            11i128 => Ok(Self::BOOL),
            12i128 => Ok(Self::FOURCC),
            13i128 => Ok(Self::STRINGLITERAL),
            14i128 => Ok(Self::STRINGTABLEENTRY),
            15i128 => Ok(Self::IMAGETABLEENTRY),
            16i128 => Ok(Self::OPAQUEDATA),
            17i128 => Ok(Self::TOONFULLNAME),
            18i128 => Ok(Self::ACCOUNTNAME),
            19i128 => Ok(Self::PROFILEADDRESS),
            20i128 => Ok(Self::S2GAMEINFO),
            21i128 => Ok(Self::ACCOUNTINFO),
            22i128 => Ok(Self::TOONHANDLE),
            23i128 => Ok(Self::GAMEACCOUNTHANDLE),
            24i128 => Ok(Self::ACHIEVEMENT),
            25i128 => Ok(Self::ACCOUNTNICKNAME),
            255i128 => Ok(Self::MAX),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a valid PresenceTypeEnumEnum"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct PresenceUpdate {
    #[bsn(name = "m_handlesCleared")]
    pub handles_cleared: Vec<u32>,
    #[bsn(name = "m_handles")]
    pub handles: Vec<u32>,
    #[bsn(name = "m_varSizes")]
    pub var_sizes: Vec<u16>,
    #[bsn(name = "m_fieldData")]
    pub field_data: Bytes,
}

