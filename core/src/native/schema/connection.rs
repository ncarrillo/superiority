#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionBoom {
    #[bsn(name = "m_error")]
    pub error: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionEnableEncryption {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionGameSiteInfo {
    #[bsn(name = "m_siteData")]
    pub site_data: Vec<super::s2game::S2GameSiteDataForClient>,
    #[bsn(name = "m_externalIp4Addr")]
    pub external_ip4_addr: super::ip4::IP4AddressPort,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionMessageFrame {
    #[bsn(name = "m_frameType")]
    pub frame_type: super::frame::FrameTypeEnum,
    #[bsn(name = "m_headers")]
    pub headers: Vec<super::frame::FrameHeader>,
    #[bsn(name = "m_payload")]
    pub payload: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionPing {
    #[bsn(name = "m_timeData")]
    pub time_data: Option<i64>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionPong {
    #[bsn(name = "m_timeData")]
    pub time_data: Option<i64>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionRegulatorUpdate {
    #[bsn(name = "m_info")]
    pub info: super::regulator::RegulatorInfo,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionServerVersion {
    #[bsn(name = "m_version")]
    pub version: u32,
}
