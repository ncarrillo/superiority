#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug)]
pub enum ClientDefinesPlayerTarget {
    PresenceId(u32),
    ToonName(super::toon::ToonFullName),
    AccountMail(Bytes),
    AccountId(u32),
    ProfileRecordAddress(super::profile::ProfileRecordAddress),
    ToonHandle(super::toon::ToonHandle),
}
impl superiority_core::bsn::FromBsn for ClientDefinesPlayerTarget {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientDefinesPlayerTarget, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::PresenceId(
                <u32 as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            1i128 => Ok(Self::ToonName(
                <super::toon::ToonFullName as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            2i128 => Ok(Self::AccountMail(
                <Bytes as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            3i128 => Ok(Self::AccountId(
                <u32 as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            4i128 => Ok(Self::ProfileRecordAddress(
                <super::profile::ProfileRecordAddress as superiority_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            5i128 => Ok(Self::ToonHandle(
                <super::toon::ToonHandle as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a ClientDefinesPlayerTarget variant"
            ))),
        }
    }
}
