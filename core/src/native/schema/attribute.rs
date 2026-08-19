#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct AttributeNonLobbyAttribute {
    #[bsn(name = "m_nameSpace")]
    pub name_space: u32,
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_index")]
    pub index: u16,
}

