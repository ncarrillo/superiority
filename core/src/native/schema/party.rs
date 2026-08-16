#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPartyBeginReadyProcess {
    #[bsn(name = "m_process")]
    pub process: super::party::PartyReadyProcess,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPartyMapOptionsChange {
    #[bsn(name = "m_changerHandle")]
    pub changer_handle: u32,
    #[bsn(name = "m_mapOptions")]
    pub map_options: super::matchmaker::MatchMakerMapOptions,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPartyReadyProcessUpdate {
    #[bsn(name = "m_memberHandle")]
    pub member_handle: Option<u32>,
    #[bsn(name = "m_reason")]
    pub reason: u16,
}

#[derive(Clone, Debug)]
pub enum PartyReadyProcess {
    JoinAmm(super::party::PartyReadyProcessJoinAmm),
    JoinCustomGame(super::party::PartyReadyProcessJoinCustomGame),
    JoinFunOrNot(super::party::PartyReadyProcessJoinFunOrNot),
    JoinReplay(super::party::PartyReadyProcessJoinReplay),
}
impl sc2_core::bsn::FromBsn for PartyReadyProcess {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for PartyReadyProcess, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::JoinAmm(<super::party::PartyReadyProcessJoinAmm as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::JoinCustomGame(<super::party::PartyReadyProcessJoinCustomGame as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::JoinFunOrNot(<super::party::PartyReadyProcessJoinFunOrNot as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::JoinReplay(<super::party::PartyReadyProcessJoinReplay as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a PartyReadyProcess variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct PartyReadyProcessJoinAmm {
    #[bsn(name = "m_mmqHandle")]
    pub mmq_handle: super::matchmaker::MatchMakerHandle,
    #[bsn(name = "m_cacheHandle")]
    pub cache_handle: Bytes,
    #[bsn(name = "m_profileAddress")]
    pub profile_address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_vetoes")]
    pub vetoes: Vec<super::s2map::S2MapHandle>,
    #[bsn(name = "m_isRanked")]
    pub is_ranked: bool,
    #[bsn(name = "m_coopMode")]
    pub coop_mode: Option<super::matchmaker::MatchMakerCoopModeEnum>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PartyReadyProcessJoinCustomGame {
    #[bsn(name = "m_map")]
    pub map: super::s2map::S2MapShortLink,
    #[bsn(name = "m_mode")]
    pub mode: super::s2master::S2MasterAdvertPostModeEnum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct PartyReadyProcessJoinFunOrNot {}

#[derive(Clone, Debug, FromBsn)]
pub struct PartyReadyProcessJoinReplay {
    #[bsn(name = "m_fileData")]
    pub file_data: super::s2master::S2MasterReplayFileData,
    #[bsn(name = "m_lobbyContextData")]
    pub lobby_context_data: super::s2game::S2GameReplayLobbyContextData,
}
