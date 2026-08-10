#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct S2GameSiteDataForClient {
    #[bsn(name = "m_name")]
    pub name: Bytes,
    #[bsn(name = "m_addressPort")]
    pub address_port: Option<super::ip4::IP4AddressPort>,
}
