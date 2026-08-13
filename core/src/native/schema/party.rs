#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientPartyMapOptionsChange {
    #[bsn(name = "m_changerHandle")]
    pub changer_handle: u32,
    #[bsn(name = "m_mapOptions")]
    pub map_options: super::matchmaker::MatchMakerMapOptions,
}
