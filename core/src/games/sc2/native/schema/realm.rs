#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct RealmHandle {
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
    #[bsn(name = "m_region")]
    pub region: u8,
    #[bsn(name = "m_id")]
    pub id: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct RealmRealmMap {
    #[bsn(name = "m_to")]
    pub to: super::realm::RealmHandle,
    #[bsn(name = "m_fromList")]
    pub from_list: Vec<u32>,
}
