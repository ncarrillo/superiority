#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionBoom {
    #[bsn(name = "m_error")]
    pub error: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientConnectionClosingReasonEnum {
    PACKETTOOLARGE,
    PACKETCORRUPT,
    PACKETINVALID,
    PACKETINCORRECT,
    HEADERCORRUPT,
    HEADERIGNORED,
    HEADERINCORRECT,
    PACKETREJECTED,
    CHANNELUNHANDLED,
    COMMANDUNHANDLED,
    COMMANDBADPERMISSIONS,
    DIRECTCALL,
    TIMEOUT,
}
impl superiority_core::bsn::FromBsn for ClientConnectionClosingReasonEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            1i128 => Ok(Self::PACKETTOOLARGE),
            2i128 => Ok(Self::PACKETCORRUPT),
            3i128 => Ok(Self::PACKETINVALID),
            4i128 => Ok(Self::PACKETINCORRECT),
            5i128 => Ok(Self::HEADERCORRUPT),
            6i128 => Ok(Self::HEADERIGNORED),
            7i128 => Ok(Self::HEADERINCORRECT),
            8i128 => Ok(Self::PACKETREJECTED),
            9i128 => Ok(Self::CHANNELUNHANDLED),
            10i128 => Ok(Self::COMMANDUNHANDLED),
            11i128 => Ok(Self::COMMANDBADPERMISSIONS),
            12i128 => Ok(Self::DIRECTCALL),
            13i128 => Ok(Self::TIMEOUT),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClientConnectionClosingReasonEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientConnectionConnectionClosing {
    #[bsn(name = "m_header")]
    pub header: Option<super::header::Header>,
    #[bsn(name = "m_closingReason")]
    pub closing_reason: super::connection::ClientConnectionClosingReasonEnum,
    #[bsn(name = "m_badData")]
    pub bad_data: Bytes,
    #[bsn(name = "m_packets")]
    pub packets: Vec<super::packetinfo::PacketInfo>,
    #[bsn(name = "m_now")]
    pub now: u32,
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
pub struct ClientConnectionLogoutRequest {}

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
