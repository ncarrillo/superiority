#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug)]
pub enum RegulatorInfo {
    None(()),
    LeakyBucket(super::regulator::RegulatorInfoLeakyBucket),
}
impl sc2_core::bsn::FromBsn for RegulatorInfo {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for RegulatorInfo, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::None(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::LeakyBucket(
                <super::regulator::RegulatorInfoLeakyBucket as sc2_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a RegulatorInfo variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct RegulatorInfoLeakyBucket {
    #[bsn(name = "m_params")]
    pub params: super::regulator::RegulatorLeakyBucketParams,
}

#[derive(Clone, Debug, FromBsn)]
pub struct RegulatorLeakyBucketParams {
    #[bsn(name = "m_threshold")]
    pub threshold: u32,
    #[bsn(name = "m_rate")]
    pub rate: u32,
}
