#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientDatagramConnectionEndPoint {
    #[bsn(name = "m_id")]
    pub id: super::datagramconnection::ClientDatagramConnectionEndPointId,
}

#[derive(Clone, Debug)]
pub enum ClientDatagramConnectionEndPointId {
    PlayerTarget(super::datagramconnection::ClientDatagramConnectionEndPointIdPlayerTarget),
}
impl sc2_core::bsn::FromBsn for ClientDatagramConnectionEndPointId {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientDatagramConnectionEndPointId, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::PlayerTarget(<super::datagramconnection::ClientDatagramConnectionEndPointIdPlayerTarget as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientDatagramConnectionEndPointId variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientDatagramConnectionEndPointIdPlayerTarget {
    #[bsn(name = "m_playerTarget")]
    pub player_target: super::defines::ClientDefinesPlayerTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatagramConnectionArbitrationNotifyEnum {
    NONE,
    ATTEMPTBEGIN,
    ATTEMPTTIMEOUT,
    PACKETRECEIVED,
    FAILED,
    STUNUPDATE,
    ADDRESSCHANGED,
}
impl sc2_core::bsn::FromBsn for DatagramConnectionArbitrationNotifyEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::NONE),
            1i128 => Ok(Self::ATTEMPTBEGIN),
            2i128 => Ok(Self::ATTEMPTTIMEOUT),
            3i128 => Ok(Self::PACKETRECEIVED),
            4i128 => Ok(Self::FAILED),
            5i128 => Ok(Self::STUNUPDATE),
            6i128 => Ok(Self::ADDRESSCHANGED),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid DatagramConnectionArbitrationNotifyEnum"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatagramConnectionNATTypeEnum {
    UNKNOWN,
    OPEN,
    NAT,
    SYMMETRICNAT,
}
impl sc2_core::bsn::FromBsn for DatagramConnectionNATTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNKNOWN),
            1i128 => Ok(Self::OPEN),
            2i128 => Ok(Self::NAT),
            3i128 => Ok(Self::SYMMETRICNAT),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid DatagramConnectionNATTypeEnum"
            ))),
        }
    }
}
