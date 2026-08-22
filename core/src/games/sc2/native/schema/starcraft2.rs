#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Starcraft2ExpansionLevelEnum {
    NOEXPANSION,
    WINGSOFLIBERTY,
    HEARTOFTHESWARM,
    LEGACYOFTHEVOID,
}
impl superiority_core::bsn::FromBsn for Starcraft2ExpansionLevelEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::NOEXPANSION),
            1i128 => Ok(Self::WINGSOFLIBERTY),
            2i128 => Ok(Self::HEARTOFTHESWARM),
            3i128 => Ok(Self::LEGACYOFTHEVOID),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid Starcraft2ExpansionLevelEnum"
            ))),
        }
    }
}
