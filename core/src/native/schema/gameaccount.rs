#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct GameAccountHandle {
    #[bsn(name = "m_region")]
    pub region: u8,
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
    #[bsn(name = "m_id")]
    pub id: u32,
}

