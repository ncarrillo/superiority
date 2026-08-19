#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct AccountFullName {
    #[bsn(name = "m_givenName")]
    pub given_name: String,
    #[bsn(name = "m_surname")]
    pub surname: String,
}

