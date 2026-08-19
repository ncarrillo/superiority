#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct IP4AddressPort {
    #[bsn(name = "m_address")]
    pub address: Bytes,
    #[bsn(name = "m_port")]
    pub port: Bytes,
}

