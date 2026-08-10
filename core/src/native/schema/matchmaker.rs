#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

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
impl sc2_core::bsn::FromBsn for MatchMakerSeasonStateEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::ROLLINPROGRESS),
            1i128 => Ok(Self::ACTIVE),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid MatchMakerSeasonStateEnum"
            ))),
        }
    }
}
