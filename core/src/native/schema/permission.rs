#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct PermissionHandle {
    #[bsn(name = "m_scope")]
    pub scope: FourCc,
    #[bsn(name = "m_id")]
    pub id: u64,
}
