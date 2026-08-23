#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientLadderGetRankings {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientLadderGetRankingsResponse {
    #[bsn(name = "GetRankings")]
    pub get_rankings: super::ladder::ClientLadderGetRankings,
    #[bsn(name = "m_rankings")]
    pub rankings: Vec<super::ladder::LadderRankingResponse>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct LadderGameData {
    #[bsn(name = "m_name")]
    pub name: Option<String>,
    #[bsn(name = "m_values")]
    pub values: Option<Vec<super::ladder::LadderKeyValue>>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct LadderKeyValue {
    #[bsn(name = "m_key")]
    pub key: u32,
    #[bsn(name = "m_value")]
    pub value: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct LadderMembership {
    #[bsn(name = "m_memberId")]
    pub member_id: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_ladderId")]
    pub ladder_id: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct LadderRanking {
    #[bsn(name = "m_count")]
    pub count: u32,
    #[bsn(name = "m_rank")]
    pub rank: u32,
    #[bsn(name = "m_exact")]
    pub exact: bool,
}

#[derive(Clone, Debug)]
pub enum LadderRankingResponse {
    Success(super::ladder::LadderRankingResponseSuccess),
    Failure(u16),
}
impl superiority_core::bsn::FromBsn for LadderRankingResponse {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for LadderRankingResponse, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::ladder::LadderRankingResponseSuccess as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a LadderRankingResponse variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct LadderRankingResponseSuccess {
    #[bsn(name = "m_membership")]
    pub membership: super::ladder::LadderMembership,
    #[bsn(name = "m_ranking")]
    pub ranking: super::ladder::LadderRanking,
    #[bsn(name = "m_gameData")]
    pub game_data: super::ladder::LadderGameData,
}
