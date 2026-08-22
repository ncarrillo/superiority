#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct SessionBillingInfo {
    #[bsn(name = "m_unitsRemaining")]
    pub units_remaining: Option<u32>,
    #[bsn(name = "m_subscriptionExpires")]
    pub subscription_expires: Option<i32>,
    #[bsn(name = "m_flags")]
    pub flags: u32,
    #[bsn(name = "m_boxLevel")]
    pub box_level: u8,
}
