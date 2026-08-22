#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerAnnounce {
    #[bsn(name = "m_handle")]
    pub handle: super::matchmaker::MatchMakerHandle,
    #[bsn(name = "m_info")]
    pub info: super::matchmaker::MatchMakerStaticInfo,
    #[bsn(name = "m_stats")]
    pub stats: super::matchmaker::MatchMakerHistogramSet,
    #[bsn(name = "m_allowingAgents")]
    pub allowing_agents: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchMakerCoopModeEnum {
    WITHAI,
    WITHMMPLAYERS,
}
impl superiority_core::bsn::FromBsn for MatchMakerCoopModeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::WITHAI),
            1i128 => Ok(Self::WITHMMPLAYERS),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid MatchMakerCoopModeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerFilter {
    #[bsn(name = "m_tags")]
    pub tags: Option<Vec<FourCc>>,
    #[bsn(name = "m_active")]
    pub active: Option<bool>,
    #[bsn(name = "m_mmqId")]
    pub mmq_id: Option<u32>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerHandle {
    #[bsn(name = "m_region")]
    pub region: u8,
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_version")]
    pub version: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerHistogramSet {
    #[bsn(name = "m_avgWaitTime")]
    pub avg_wait_time: Vec<Option<u32>>,
    #[bsn(name = "m_playerMatchRate")]
    pub player_match_rate: Vec<Option<u32>>,
    #[bsn(name = "m_playersInQueue")]
    pub players_in_queue: Option<u32>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerLeagueConfig {
    #[bsn(name = "m_league")]
    pub league: super::matchmaker::MatchMakerLeagueKey,
    #[bsn(name = "m_bonusAccrualRate")]
    pub bonus_accrual_rate: f64,
    #[bsn(name = "m_startDate")]
    pub start_date: Option<i32>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerLeagueKey {
    #[bsn(name = "RankedMatchmakerKey")]
    pub ranked_matchmaker_key: super::matchmaker::MatchMakerRankedMatchmakerKey,
    #[bsn(name = "m_leagueId")]
    pub league_id: super::league::LeagueLeagueEnum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerLeagueProfile {
    #[bsn(name = "m_league")]
    pub league: super::matchmaker::MatchMakerLeagueKey,
    #[bsn(name = "m_profile")]
    pub profile: super::profile::ProfileRecordAddress,
}

#[derive(Clone, Debug)]
pub enum MatchMakerMapOptions {
    Vetoes(super::matchmaker::MatchMakerMapPreferences),
    Selection(u32),
}
impl superiority_core::bsn::FromBsn for MatchMakerMapOptions {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for MatchMakerMapOptions, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Vetoes(
                <super::matchmaker::MatchMakerMapPreferences as superiority_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            1i128 => Ok(Self::Selection(<u32 as superiority_core::bsn::FromBsn>::from_bsn(
                inner,
            )?)),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a MatchMakerMapOptions variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerMapPreferences {
    #[bsn(name = "m_vetoedMapIds")]
    pub vetoed_map_ids: Vec<u32>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerPerGameQueueInfo {
    #[bsn(name = "m_minCommanderLevel")]
    pub min_commander_level: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerRankedMatchmakerConfig {
    #[bsn(name = "m_key")]
    pub key: super::matchmaker::MatchMakerRankedMatchmakerKey,
    #[bsn(name = "m_ratingScale")]
    pub rating_scale: f64,
    #[bsn(name = "m_ratingShift")]
    pub rating_shift: f64,
    #[bsn(name = "m_leaderboardLadderId")]
    pub leaderboard_ladder_id: u32,
    #[bsn(name = "m_leaderboardProfile")]
    pub leaderboard_profile: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_placementMatches")]
    pub placement_matches: u16,
    #[bsn(name = "m_ignoreMatches")]
    pub ignore_matches: u16,
    #[bsn(name = "m_lowballMatches")]
    pub lowball_matches: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerRankedMatchmakerKey {
    #[bsn(name = "m_program")]
    pub program: FourCc,
    #[bsn(name = "m_seasonId")]
    pub season_id: u16,
    #[bsn(name = "m_mmqId")]
    pub mmq_id: u32,
    #[bsn(name = "m_teamType")]
    pub team_type: super::league::LeagueTeamTypeEnum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerSeasonAuthorityUpdate {
    #[bsn(name = "m_state")]
    pub state: super::matchmaker::MatchMakerSeasonStateEnum,
    #[bsn(name = "m_season")]
    pub season: super::matchmaker::MatchMakerSeasonInfo,
    #[bsn(name = "m_leagues")]
    pub leagues: Vec<super::matchmaker::MatchMakerLeagueProfile>,
    #[bsn(name = "m_rankedMatchmakers")]
    pub ranked_matchmakers: Vec<super::matchmaker::MatchMakerRankedMatchmakerConfig>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerSeasonInfo {
    #[bsn(name = "m_version")]
    pub version: u16,
    #[bsn(name = "m_seasonId")]
    pub season_id: u16,
    #[bsn(name = "m_versionedSeasonId")]
    pub versioned_season_id: u16,
    #[bsn(name = "m_startDate")]
    pub start_date: i32,
    #[bsn(name = "m_endDate")]
    pub end_date: i32,
    #[bsn(name = "m_bonusLockDate")]
    pub bonus_lock_date: Option<i32>,
    #[bsn(name = "m_reassignLockDate")]
    pub reassign_lock_date: Option<i32>,
    #[bsn(name = "m_eloMean")]
    pub elo_mean: f64,
    #[bsn(name = "m_number")]
    pub number: Option<u16>,
    #[bsn(name = "m_versionedNumber")]
    pub versioned_number: Option<u16>,
    #[bsn(name = "m_year")]
    pub year: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchMakerSeasonStateEnum {
    ROLLINPROGRESS,
    ACTIVE,
}
impl superiority_core::bsn::FromBsn for MatchMakerSeasonStateEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::ROLLINPROGRESS),
            1i128 => Ok(Self::ACTIVE),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid MatchMakerSeasonStateEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct MatchMakerStaticInfo {
    #[bsn(name = "m_cacheHandle")]
    pub cache_handle: Bytes,
    #[bsn(name = "m_profileAddress")]
    pub profile_address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_tags")]
    pub tags: Vec<FourCc>,
    #[bsn(name = "m_active")]
    pub active: bool,
    #[bsn(name = "m_teamSize")]
    pub team_size: u8,
    #[bsn(name = "m_requiredPermissions")]
    pub required_permissions: Vec<super::permission::PermissionHandle>,
    #[bsn(name = "m_gameSpecific")]
    pub game_specific: super::matchmaker::MatchMakerPerGameQueueInfo,
}
