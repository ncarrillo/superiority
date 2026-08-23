#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct PacketInfo {
    #[bsn(name = "m_layer")]
    pub layer: FourCc,
    #[bsn(name = "m_command")]
    pub command: FourCc,
    #[bsn(name = "m_offset")]
    pub offset: u16,
    #[bsn(name = "m_size")]
    pub size: u16,
    #[bsn(name = "m_time")]
    pub time: u32,
}
