#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ConferenceClubName {
    #[bsn(name = "m_club")]
    pub club: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ConferenceConferenceConfiguration {
    #[bsn(name = "m_maxMembers")]
    pub max_members: u16,
    #[bsn(name = "m_allowedPrograms")]
    pub allowed_programs: Vec<FourCc>,
    #[bsn(name = "m_allowedRealms")]
    pub allowed_realms: Vec<u32>,
    #[bsn(name = "m_flags")]
    pub flags: u32,
    #[bsn(name = "m_targetProportion")]
    pub target_proportion: f32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ConferenceFullConferenceDescription {
    #[bsn(name = "m_parentCategory")]
    pub parent_category: u8,
    #[bsn(name = "m_name")]
    pub name: super::conference::ConferenceShardName,
    #[bsn(name = "m_sortOrder")]
    pub sort_order: u16,
    #[bsn(name = "m_configuration")]
    pub configuration: super::conference::ConferenceConferenceConfiguration,
    #[bsn(name = "m_id")]
    pub id: u32,
}

#[derive(Clone, Debug)]
pub enum ConferenceLocatorKey {
    Private(String),
    Public(super::conference::ConferencePublicPartialName),
    Club(super::conference::ConferenceClubName),
}
impl sc2_core::bsn::FromBsn for ConferenceLocatorKey {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ConferenceLocatorKey, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Private(<String as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Public(<super::conference::ConferencePublicPartialName as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Club(<super::conference::ConferenceClubName as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ConferenceLocatorKey variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ConferencePublicPartialName {
    #[bsn(name = "m_locale")]
    pub locale: FourCc,
    #[bsn(name = "m_name")]
    pub name: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ConferenceShardName {
    #[bsn(name = "m_key")]
    pub key: super::conference::ConferenceLocatorKey,
    #[bsn(name = "m_index")]
    pub index: u16,
}
