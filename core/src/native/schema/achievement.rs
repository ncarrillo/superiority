#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct AchievementProgramHandleAggregation {
    #[bsn(name = "m_program")]
    pub program: FourCc,
    #[bsn(name = "m_handle")]
    pub handle: Bytes,
}
