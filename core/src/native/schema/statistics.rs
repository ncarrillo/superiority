#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct StatisticsClientValue {
    #[bsn(name = "m_report")]
    pub report: u32,
    #[bsn(name = "m_value")]
    pub value: u64,
}

