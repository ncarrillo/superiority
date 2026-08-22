#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct VersionRecord {
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
    #[bsn(name = "m_component")]
    pub component: FourCc,
    #[bsn(name = "m_version")]
    pub version: u32,
}
