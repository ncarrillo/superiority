#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct S2GameReplayLobbyContextData {
    #[bsn(name = "m_type")]
    pub type_: super::s2game::S2GameReplayLobbyTypeEnum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S2GameReplayLobbyTypeEnum {
    WATCH,
    RECOVER,
}
impl sc2_core::bsn::FromBsn for S2GameReplayLobbyTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::WATCH),
            1i128 => Ok(Self::RECOVER),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid S2GameReplayLobbyTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2GameSiteDataForClient {
    #[bsn(name = "m_name")]
    pub name: Bytes,
    #[bsn(name = "m_addressPort")]
    pub address_port: Option<super::ip4::IP4AddressPort>,
}
