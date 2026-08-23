#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct Header {
    #[bsn(name = "m_command")]
    pub command: u8,
    #[bsn(name = "m_channel")]
    pub channel: Option<u8>,
}
