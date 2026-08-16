#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug)]
pub enum AchievementCriteriaComparandChoice {
    None(()),
    FourCharCode(FourCc),
    Bool(bool),
    USixteen(u16),
    UThirtyTwo(u32),
    USixtyFour(u64),
    SThirtyTwo(i32),
    UnlockableHandle(super::achievement::AchievementCriteriaComparandChoiceUnlockableHandle),
}
impl sc2_core::bsn::FromBsn for AchievementCriteriaComparandChoice {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for AchievementCriteriaComparandChoice, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::None(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::FourCharCode(<FourCc as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Bool(<bool as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::USixteen(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            4i128 => Ok(Self::UThirtyTwo(<u32 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            5i128 => Ok(Self::USixtyFour(<u64 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            6i128 => Ok(Self::SThirtyTwo(<i32 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            7i128 => Ok(Self::UnlockableHandle(<super::achievement::AchievementCriteriaComparandChoiceUnlockableHandle as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a AchievementCriteriaComparandChoice variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementCriteriaComparandChoiceUnlockableHandle {
    #[bsn(name = "m_tag")]
    pub tag: FourCc,
    #[bsn(name = "m_index")]
    pub index: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementCriteriaComparandDetails {
    #[bsn(name = "m_comparand")]
    pub comparand: u64,
    #[bsn(name = "m_quantity")]
    pub quantity: u64,
    #[bsn(name = "m_comparandChoice")]
    pub comparand_choice: Option<super::achievement::AchievementCriteriaComparandChoice>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementCriteriaUpdateRecord {
    #[bsn(name = "m_criteriaId")]
    pub criteria_id: u64,
    #[bsn(name = "m_startTime")]
    pub start_time: Option<i32>,
    #[bsn(name = "m_quantity")]
    pub quantity: Option<u64>,
    #[bsn(name = "m_flags")]
    pub flags: u32,
    #[bsn(name = "m_comparands")]
    pub comparands: Option<Vec<super::achievement::AchievementCriteriaComparandDetails>>,
}

#[derive(Clone, Debug)]
pub enum AchievementDataSegment {
    Achievements(Vec<super::achievement::AchievementPersistentRecord>),
    Criteria(Vec<super::achievement::AchievementCriteriaUpdateRecord>),
    Notification(super::achievement::AchievementDataSegmentNotification),
    Quests(Vec<super::achievement::AchievementQuestUpdateRecord>),
}
impl sc2_core::bsn::FromBsn for AchievementDataSegment {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for AchievementDataSegment, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Achievements(<Vec<super::achievement::AchievementPersistentRecord> as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Criteria(<Vec<super::achievement::AchievementCriteriaUpdateRecord> as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Notification(<super::achievement::AchievementDataSegmentNotification as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Quests(<Vec<super::achievement::AchievementQuestUpdateRecord> as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a AchievementDataSegment variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementDataSegmentNotification {
    #[bsn(name = "m_code")]
    pub code: super::achievement::AchievementNotificationEnum,
    #[bsn(name = "m_programs")]
    pub programs: Vec<FourCc>,
    #[bsn(name = "m_evaluators")]
    pub evaluators: Vec<FourCc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AchievementListenModeEnum {
    SUBSCRIPTION,
    SNAPSHOT,
}
impl sc2_core::bsn::FromBsn for AchievementListenModeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::SUBSCRIPTION),
            1i128 => Ok(Self::SNAPSHOT),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid AchievementListenModeEnum"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AchievementListenScopeEnum {
    ACHIEVEMENTSONLY,
    ACHIEVEMENTSANDCRITERIA,
}
impl sc2_core::bsn::FromBsn for AchievementListenScopeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::ACHIEVEMENTSONLY),
            1i128 => Ok(Self::ACHIEVEMENTSANDCRITERIA),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid AchievementListenScopeEnum"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AchievementNotificationEnum {
    SNAPSHOTCOMPLETE,
    CONNECTIONINTERRUPTED,
    CONNECTIONRESUMED,
    PERMANENTFAILURE,
    STATICDATAUNAVAILABLE,
    EXPECTEDCANCELLATION,
    UNEXPECTEDCANCELLATION,
    STATICDATAINITIALIZING,
}
impl sc2_core::bsn::FromBsn for AchievementNotificationEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::SNAPSHOTCOMPLETE),
            1i128 => Ok(Self::CONNECTIONINTERRUPTED),
            2i128 => Ok(Self::CONNECTIONRESUMED),
            3i128 => Ok(Self::PERMANENTFAILURE),
            4i128 => Ok(Self::STATICDATAUNAVAILABLE),
            5i128 => Ok(Self::EXPECTEDCANCELLATION),
            6i128 => Ok(Self::UNEXPECTEDCANCELLATION),
            7i128 => Ok(Self::STATICDATAINITIALIZING),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid AchievementNotificationEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementPersistentRecord {
    #[bsn(name = "m_achievementId")]
    pub achievement_id: u64,
    #[bsn(name = "m_completion")]
    pub completion: i32,
    #[bsn(name = "m_earnedCount")]
    pub earned_count: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementProgramHandleAggregation {
    #[bsn(name = "m_program")]
    pub program: FourCc,
    #[bsn(name = "m_handle")]
    pub handle: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementQuestUpdateRecord {
    #[bsn(name = "m_lastDailyAwarded")]
    pub last_daily_awarded: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAchievementData {
    #[bsn(name = "m_address")]
    pub address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_segment")]
    pub segment: super::achievement::AchievementDataSegment,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAchievementListenRequest {
    #[bsn(name = "m_address")]
    pub address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_mode")]
    pub mode: super::achievement::AchievementListenModeEnum,
    #[bsn(name = "m_scope")]
    pub scope: super::achievement::AchievementListenScopeEnum,
}
