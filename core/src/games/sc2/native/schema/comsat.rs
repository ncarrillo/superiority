#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientComSatTalkerId {
    #[bsn(name = "m_id")]
    pub id: super::comsat::ClientComSatTalkerIdId,
}

#[derive(Clone, Debug)]
pub enum ClientComSatTalkerIdId {
    Invalid(()),
    DatagramConnectionEndPoint(super::comsat::ClientComSatTalkerIdIdDatagramConnectionEndPoint),
    Stream(super::comsat::ClientComSatTalkerIdIdStream),
}
impl superiority_core::bsn::FromBsn for ClientComSatTalkerIdId {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientComSatTalkerIdId, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Invalid(<() as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::DatagramConnectionEndPoint(<super::comsat::ClientComSatTalkerIdIdDatagramConnectionEndPoint as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Stream(<super::comsat::ClientComSatTalkerIdIdStream as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClientComSatTalkerIdId variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientComSatTalkerIdIdDatagramConnectionEndPoint {
    #[bsn(name = "m_endPoint")]
    pub end_point: super::datagramconnection::ClientDatagramConnectionEndPoint,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientComSatTalkerIdIdStream {
    #[bsn(name = "m_id")]
    pub id: u8,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientComSatTalkerInfo {
    #[bsn(name = "m_enabled")]
    pub enabled: bool,
    #[bsn(name = "m_id")]
    pub id: super::comsat::ClientComSatTalkerId,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ComSatSessionIdUnion {
    #[bsn(name = "m_label")]
    pub label: u32,
    #[bsn(name = "m_instance")]
    pub instance: u32,
}
