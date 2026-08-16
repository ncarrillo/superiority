#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct AuthenticationModuleInput {
    #[bsn(name = "m_id")]
    pub id: Bytes,
    #[bsn(name = "m_data")]
    pub data: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct AuthenticationModuleOutput {
    #[bsn(name = "m_data")]
    pub data: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationConfiguration {
    #[bsn(name = "m_useS3Depot")]
    pub use_s3_depot: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationGenerateWebTokenRequest {
    #[bsn(name = "m_token")]
    pub token: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationGenerateWebTokenResponse {
    #[bsn(name = "m_token")]
    pub token: u32,
    #[bsn(name = "m_result")]
    pub result: super::authentication::ClientAuthenticationGenerateWebTokenResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientAuthenticationGenerateWebTokenResponseResult {
    Success(super::authentication::ClientAuthenticationGenerateWebTokenResponseResultSuccess),
    Failure(u16),
}
impl sc2_core::bsn::FromBsn for ClientAuthenticationGenerateWebTokenResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientAuthenticationGenerateWebTokenResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::authentication::ClientAuthenticationGenerateWebTokenResponseResultSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientAuthenticationGenerateWebTokenResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationGenerateWebTokenResponseResultSuccess {
    #[bsn(name = "m_webToken")]
    pub web_token: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationLogon {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationLogonRequest3 {
    #[bsn(name = "m_requestCommon")]
    pub request_common: super::authentication::ClientAuthenticationRequestCommon,
    #[bsn(name = "m_account")]
    pub account: Option<Bytes>,
    #[bsn(name = "m_compatibility")]
    pub compatibility: u64,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationLogonResponse {
    #[bsn(name = "Logon")]
    pub logon: super::authentication::ClientAuthenticationLogon,
    #[bsn(name = "m_result")]
    pub result: super::authentication::ClientAuthenticationLogonResponseResult,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationLogonResponse3 {
    #[bsn(name = "LogonResponse")]
    pub logon_response: super::authentication::ClientAuthenticationLogonResponse,
    #[bsn(name = "m_raf")]
    pub raf: Option<Bytes>,
}

#[derive(Clone, Debug)]
pub enum ClientAuthenticationLogonResponseResult {
    Success(super::authentication::ClientAuthenticationLogonResponseSuccess),
    Failure(super::authentication::ClientAuthenticationResponseFailure),
}
impl sc2_core::bsn::FromBsn for ClientAuthenticationLogonResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientAuthenticationLogonResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::authentication::ClientAuthenticationLogonResponseSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<super::authentication::ClientAuthenticationResponseFailure as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientAuthenticationLogonResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationLogonResponseSuccess {
    #[bsn(name = "ResponseSuccessCommon")]
    pub response_success_common: super::authentication::ClientAuthenticationResponseSuccessCommon,
    #[bsn(name = "m_fullName")]
    pub full_name: super::account::AccountFullName,
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
    #[bsn(name = "m_accountRegion")]
    pub account_region: u8,
    #[bsn(name = "m_accountFlags")]
    pub account_flags: u64,
    #[bsn(name = "m_gameAccountRegion")]
    pub game_account_region: u8,
    #[bsn(name = "m_gameAccountName")]
    pub game_account_name: Bytes,
    #[bsn(name = "m_gameAccountFlags")]
    pub game_account_flags: u64,
    #[bsn(name = "m_logonFailures")]
    pub logon_failures: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationProof {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationProofRequest {
    #[bsn(name = "Proof")]
    pub proof: super::authentication::ClientAuthenticationProof,
    #[bsn(name = "m_request")]
    pub request: Vec<super::authentication::AuthenticationModuleInput>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationProofResponse {
    #[bsn(name = "Proof")]
    pub proof: super::authentication::ClientAuthenticationProof,
    #[bsn(name = "m_response")]
    pub response: Vec<super::authentication::AuthenticationModuleOutput>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationRequestCommon {
    #[bsn(name = "m_program")]
    pub program: FourCc,
    #[bsn(name = "m_platform")]
    pub platform: FourCc,
    #[bsn(name = "m_locale")]
    pub locale: FourCc,
    #[bsn(name = "m_versions")]
    pub versions: Vec<super::version::VersionRecord>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResponseFailure {
    #[bsn(name = "m_strings")]
    pub strings: Option<Bytes>,
    #[bsn(name = "m_result")]
    pub result: super::authentication::ClientAuthenticationResponseFailureResult,
}

#[derive(Clone, Debug)]
pub enum ClientAuthenticationResponseFailureResult {
    Update(()),
    Failure(super::authentication::ClientAuthenticationResponseFailureResultFailure),
    VersionCheckDisconnect(()),
}
impl sc2_core::bsn::FromBsn for ClientAuthenticationResponseFailureResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientAuthenticationResponseFailureResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Update(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<super::authentication::ClientAuthenticationResponseFailureResultFailure as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::VersionCheckDisconnect(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientAuthenticationResponseFailureResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResponseFailureResultFailure {
    #[bsn(name = "m_error")]
    pub error: u16,
    #[bsn(name = "m_wait")]
    pub wait: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResponseSuccessCommon {
    #[bsn(name = "m_finalRequest")]
    pub final_request: Vec<super::authentication::AuthenticationModuleInput>,
    #[bsn(name = "m_pingTimeout")]
    pub ping_timeout: i32,
    #[bsn(name = "m_regulatorRules")]
    pub regulator_rules: Option<super::regulator::RegulatorInfo>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResume {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResumeRequest {
    #[bsn(name = "RequestCommon")]
    pub request_common: super::authentication::ClientAuthenticationRequestCommon,
    #[bsn(name = "m_account")]
    pub account: Bytes,
    #[bsn(name = "m_gameAccountRegion")]
    pub game_account_region: u8,
    #[bsn(name = "m_gameAccountName")]
    pub game_account_name: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResumeResponse {
    #[bsn(name = "Resume")]
    pub resume: super::authentication::ClientAuthenticationResume,
    #[bsn(name = "m_result")]
    pub result: super::authentication::ClientAuthenticationResumeResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientAuthenticationResumeResponseResult {
    Success(super::authentication::ClientAuthenticationResumeResponseSuccess),
    Failure(super::authentication::ClientAuthenticationResponseFailure),
}
impl sc2_core::bsn::FromBsn for ClientAuthenticationResumeResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientAuthenticationResumeResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::authentication::ClientAuthenticationResumeResponseSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<super::authentication::ClientAuthenticationResponseFailure as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientAuthenticationResumeResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationResumeResponseSuccess {
    #[bsn(name = "ResponseSuccessCommon")]
    pub response_success_common: super::authentication::ClientAuthenticationResponseSuccessCommon,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientAuthenticationSingleSignOnRequest3 {
    #[bsn(name = "m_requestCommon")]
    pub request_common: super::authentication::ClientAuthenticationRequestCommon,
    #[bsn(name = "m_ssoId")]
    pub sso_id: Bytes,
    #[bsn(name = "m_compatibility")]
    pub compatibility: u64,
}
