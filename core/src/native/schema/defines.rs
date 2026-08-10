#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug)]
pub enum ClientDefinesPlayerTarget {
    PresenceId(u32),
    ToonName(super::toon::ToonFullName),
    AccountMail(Bytes),
    AccountId(u32),
    ProfileRecordAddress(super::profile::ProfileRecordAddress),
    ToonHandle(super::toon::ToonHandle),
}
impl sc2_core::bsn::FromBsn for ClientDefinesPlayerTarget {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientDefinesPlayerTarget, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::PresenceId(<u32 as sc2_core::bsn::FromBsn>::from_bsn(
                inner,
            )?)),
            1i128 => Ok(Self::ToonName(
                <super::toon::ToonFullName as sc2_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            2i128 => Ok(Self::AccountMail(
                <Bytes as sc2_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            3i128 => Ok(Self::AccountId(<u32 as sc2_core::bsn::FromBsn>::from_bsn(
                inner,
            )?)),
            4i128 => Ok(Self::ProfileRecordAddress(
                <super::profile::ProfileRecordAddress as sc2_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            5i128 => Ok(Self::ToonHandle(
                <super::toon::ToonHandle as sc2_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a ClientDefinesPlayerTarget variant"
            ))),
        }
    }
}
