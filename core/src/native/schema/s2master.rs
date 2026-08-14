#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MasterCurrentSeason {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MasterCurrentSeasonResponse {
    #[bsn(name = "CurrentSeason")]
    pub current_season: super::s2master::ClientS2MasterCurrentSeason,
    #[bsn(name = "m_result")]
    pub result: super::s2master::ClientS2MasterCurrentSeasonResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientS2MasterCurrentSeasonResponseResult {
    Success(super::s2master::ClientS2MasterCurrentSeasonResponseResultSuccess),
    Failure(u16),
}
impl sc2_core::bsn::FromBsn for ClientS2MasterCurrentSeasonResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientS2MasterCurrentSeasonResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::s2master::ClientS2MasterCurrentSeasonResponseResultSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientS2MasterCurrentSeasonResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MasterCurrentSeasonResponseResultSuccess {
    #[bsn(name = "m_season")]
    pub season: super::matchmaker::MatchMakerSeasonAuthorityUpdate,
    #[bsn(name = "m_leagueConfigs")]
    pub league_configs: Vec<super::matchmaker::MatchMakerLeagueConfig>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S2MasterAdvertPostModeEnum {
    JOINPUBLIC,
    JOINORCREATEPUBLIC,
    CREATEPUBLIC,
    CREATEPRIVATE,
    RANDOMHOTORNOT,
}
impl sc2_core::bsn::FromBsn for S2MasterAdvertPostModeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            1i128 => Ok(Self::JOINPUBLIC),
            2i128 => Ok(Self::JOINORCREATEPUBLIC),
            3i128 => Ok(Self::CREATEPUBLIC),
            4i128 => Ok(Self::CREATEPRIVATE),
            5i128 => Ok(Self::RANDOMHOTORNOT),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid S2MasterAdvertPostModeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MasterReplayFileData {
    #[bsn(name = "m_replayHandle")]
    pub replay_handle: Bytes,
    #[bsn(name = "m_archiveHandles")]
    pub archive_handles: Vec<Bytes>,
}
