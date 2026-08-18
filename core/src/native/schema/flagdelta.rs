#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagDeltaEnum {
    UNSET,
    SET,
    TOGGLE,
}
impl sc2_core::bsn::FromBsn for FlagDeltaEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNSET),
            1i128 => Ok(Self::SET),
            2i128 => Ok(Self::TOGGLE),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid FlagDeltaEnum"
            ))),
        }
    }
}
