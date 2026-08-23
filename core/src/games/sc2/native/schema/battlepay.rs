#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattlePayCurrencyEnum {
    INVALID,
    USD,
    GBP,
    KRW,
    EUR,
    RUB,
    COP,
    PEN,
    ARS,
    CLP,
    MXN,
    BRL,
    AUD,
    SGD,
    CPT,
    TPT,
    XTS,
    XHG,
    XHS,
    XHC,
    NZD,
    JPY,
    CAD,
    THB,
}
impl superiority_core::bsn::FromBsn for BattlePayCurrencyEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::INVALID),
            1i128 => Ok(Self::USD),
            2i128 => Ok(Self::GBP),
            3i128 => Ok(Self::KRW),
            4i128 => Ok(Self::EUR),
            5i128 => Ok(Self::RUB),
            6i128 => Ok(Self::COP),
            7i128 => Ok(Self::PEN),
            8i128 => Ok(Self::ARS),
            9i128 => Ok(Self::CLP),
            10i128 => Ok(Self::MXN),
            11i128 => Ok(Self::BRL),
            12i128 => Ok(Self::AUD),
            13i128 => Ok(Self::SGD),
            14i128 => Ok(Self::CPT),
            15i128 => Ok(Self::TPT),
            16i128 => Ok(Self::XTS),
            17i128 => Ok(Self::XHG),
            18i128 => Ok(Self::XHS),
            19i128 => Ok(Self::XHC),
            20i128 => Ok(Self::NZD),
            21i128 => Ok(Self::JPY),
            22i128 => Ok(Self::CAD),
            23i128 => Ok(Self::THB),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid BattlePayCurrencyEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct BattlePayLicenseInstance {
    #[bsn(name = "m_id")]
    pub id: u64,
    #[bsn(name = "m_flags")]
    pub flags: u16,
    #[bsn(name = "m_expireTime")]
    pub expire_time: Option<i32>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct BattlePayWalletInfo {
    #[bsn(name = "m_id")]
    pub id: i64,
    #[bsn(name = "m_type")]
    pub type_: super::battlepay::BattlePayWalletTypeEnum,
    #[bsn(name = "m_name")]
    pub name: String,
    #[bsn(name = "m_isPrimary")]
    pub is_primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattlePayWalletTypeEnum {
    CreditCard,
    DirectDebit,
    Paypal,
    EBalance,
    GTAPP,
    GenericPaymentProvider,
}
impl superiority_core::bsn::FromBsn for BattlePayWalletTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            1i128 => Ok(Self::CreditCard),
            2i128 => Ok(Self::DirectDebit),
            3i128 => Ok(Self::Paypal),
            4i128 => Ok(Self::EBalance),
            5i128 => Ok(Self::GTAPP),
            6i128 => Ok(Self::GenericPaymentProvider),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid BattlePayWalletTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayBalance {
    #[bsn(name = "m_currency")]
    pub currency: super::battlepay::BattlePayCurrencyEnum,
    #[bsn(name = "m_amount")]
    pub amount: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayGetInfo {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayGetInfoResponse {
    #[bsn(name = "GetInfo")]
    pub get_info: super::battlepay::ClientBattlePayGetInfo,
    #[bsn(name = "m_licenseResult")]
    pub license_result: u16,
    #[bsn(name = "m_accountCountry")]
    pub account_country: Bytes,
    #[bsn(name = "m_productCatalog")]
    pub product_catalog: Bytes,
    #[bsn(name = "m_licenseCatalog")]
    pub license_catalog: Bytes,
    #[bsn(name = "m_currencies")]
    pub currencies: Vec<super::battlepay::BattlePayCurrencyEnum>,
    #[bsn(name = "m_balances")]
    pub balances: Vec<super::battlepay::ClientBattlePayBalance>,
    #[bsn(name = "m_licenses")]
    pub licenses: Vec<super::battlepay::BattlePayLicenseInstance>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayGetWallets {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayGetWalletsResponse {
    #[bsn(name = "GetWallets")]
    pub get_wallets: super::battlepay::ClientBattlePayGetWallets,
    #[bsn(name = "m_result")]
    pub result: super::battlepay::ClientBattlePayGetWalletsResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientBattlePayGetWalletsResponseResult {
    Success(super::battlepay::ClientBattlePayGetWalletsResponseResultSuccess),
    Failure(super::battlepay::ClientBattlePayGetWalletsResponseResultFailure),
}
impl superiority_core::bsn::FromBsn for ClientBattlePayGetWalletsResponseResult {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientBattlePayGetWalletsResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::battlepay::ClientBattlePayGetWalletsResponseResultSuccess as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<super::battlepay::ClientBattlePayGetWalletsResponseResultFailure as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClientBattlePayGetWalletsResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayGetWalletsResponseResultFailure {
    #[bsn(name = "m_errorCode")]
    pub error_code: u16,
    #[bsn(name = "m_bpayCode")]
    pub bpay_code: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientBattlePayGetWalletsResponseResultSuccess {
    #[bsn(name = "m_wallets")]
    pub wallets: Vec<super::battlepay::BattlePayWalletInfo>,
}
