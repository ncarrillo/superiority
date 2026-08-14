#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapHandle {
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_version")]
    pub version: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapLinkEntry {
    #[bsn(name = "m_handle")]
    pub handle: super::s2map::S2MapHandle,
    #[bsn(name = "m_type")]
    pub type_: super::s2map::S2MapLinkEntryTypeEnum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S2MapLinkEntryTypeEnum {
    PRIMARY,
    OPTIONAL,
    DEPENDENCY,
}
impl sc2_core::bsn::FromBsn for S2MapLinkEntryTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::PRIMARY),
            1i128 => Ok(Self::OPTIONAL),
            2i128 => Ok(Self::DEPENDENCY),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid S2MapLinkEntryTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapShortLink {
    #[bsn(name = "m_variantIndex")]
    pub variant_index: u8,
    #[bsn(name = "m_speed")]
    pub speed: FourCc,
    #[bsn(name = "m_entries")]
    pub entries: Vec<super::s2map::S2MapLinkEntry>,
}
