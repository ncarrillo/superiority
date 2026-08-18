#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileAddressQuery {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileAddressQueryRequest {
    #[bsn(name = "AddressQuery")]
    pub address_query: super::profile::ClientProfileAddressQuery,
    #[bsn(name = "m_requestId")]
    pub request_id: u32,
    #[bsn(name = "m_playerTarget")]
    pub player_target: super::defines::ClientDefinesPlayerTarget,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileAddressQueryResponse {
    #[bsn(name = "AddressQuery")]
    pub address_query: super::profile::ClientProfileAddressQuery,
    #[bsn(name = "m_requestId")]
    pub request_id: u32,
    #[bsn(name = "m_result")]
    pub result: super::profile::ClientProfileAddressQueryResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientProfileAddressQueryResponseResult {
    Success(super::profile::ClientProfileAddressQueryResponseResultSuccess),
    Failure(super::profile::ClientProfileAddressQueryResponseResultFailure),
}
impl sc2_core::bsn::FromBsn for ClientProfileAddressQueryResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientProfileAddressQueryResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::profile::ClientProfileAddressQueryResponseResultSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<super::profile::ClientProfileAddressQueryResponseResultFailure as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientProfileAddressQueryResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileAddressQueryResponseResultFailure {
    #[bsn(name = "m_error")]
    pub error: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileAddressQueryResponseResultSuccess {
    #[bsn(name = "m_address")]
    pub address: super::profile::ProfileRecordAddress,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileHandleToNameResponse {
    #[bsn(name = "m_result")]
    pub result: u16,
    #[bsn(name = "m_handle")]
    pub handle: super::toon::ToonHandle,
    #[bsn(name = "m_name")]
    pub name: Option<super::toon::ToonFullName>,
    #[bsn(name = "m_tag")]
    pub tag: Option<String>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileRead {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileReadRequest {
    #[bsn(name = "Read")]
    pub read: super::profile::ClientProfileRead,
    #[bsn(name = "m_requestId")]
    pub request_id: u32,
    #[bsn(name = "m_recordAddress")]
    pub record_address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_specification")]
    pub specification: super::profile::ProfileFullReadSpecification,
    #[bsn(name = "m_clientHash")]
    pub client_hash: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileReadResponse {
    #[bsn(name = "Read")]
    pub read: super::profile::ClientProfileRead,
    #[bsn(name = "m_requestId")]
    pub request_id: u32,
    #[bsn(name = "m_result")]
    pub result: super::profile::ProfileProfileDataResponse,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileResolveToonHandleToName {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileResolveToonHandleToNameRequest {
    #[bsn(name = "ResolveToonHandleToName")]
    pub resolve_toon_handle_to_name: super::profile::ClientProfileResolveToonHandleToName,
    #[bsn(name = "m_handles")]
    pub handles: Vec<super::toon::ToonHandle>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileResolveToonHandleToNameResponse {
    #[bsn(name = "ResolveToonHandleToName")]
    pub resolve_toon_handle_to_name: super::profile::ClientProfileResolveToonHandleToName,
    #[bsn(name = "m_responses")]
    pub responses: Vec<super::profile::ClientProfileHandleToNameResponse>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileResolveToonNameToHandle {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileResolveToonNameToHandleRequest {
    #[bsn(name = "ResolveToonNameToHandle")]
    pub resolve_toon_name_to_handle: super::profile::ClientProfileResolveToonNameToHandle,
    #[bsn(name = "m_name")]
    pub name: super::toon::ToonFullName,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileResolveToonNameToHandleResponse {
    #[bsn(name = "ResolveToonNameToHandle")]
    pub resolve_toon_name_to_handle: super::profile::ClientProfileResolveToonNameToHandle,
    #[bsn(name = "m_name")]
    pub name: super::toon::ToonFullName,
    #[bsn(name = "m_result")]
    pub result: u16,
    #[bsn(name = "m_handle")]
    pub handle: Option<super::toon::ToonHandle>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileSendStatsUIEvent {
    #[bsn(name = "m_events")]
    pub events: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientProfileSettingsAvailable {
    #[bsn(name = "m_type")]
    pub type_: super::profile::ClientProfileSettingsTypeEnum,
    #[bsn(name = "m_address")]
    pub address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_path")]
    pub path: Bytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientProfileSettingsTypeEnum {
    SETTINGACCOUNT,
    SETTINGGAME,
    SETTINGTOON,
}
impl sc2_core::bsn::FromBsn for ClientProfileSettingsTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            1i128 => Ok(Self::SETTINGACCOUNT),
            2i128 => Ok(Self::SETTINGGAME),
            3i128 => Ok(Self::SETTINGTOON),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ClientProfileSettingsTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileFullReadSpecification {
    #[bsn(name = "m_selection")]
    pub selection: super::profile::ProfileReadSelection,
    #[bsn(name = "m_reader")]
    pub reader: Option<Vec<u64>>,
}

#[derive(Clone, Debug)]
pub enum ProfileHeroStatSelection {
    AllForSeason(super::profile::ProfileHeroStatSelectionAllForSeason),
    TopForSeason(super::profile::ProfileHeroStatSelectionTopForSeason),
}
impl sc2_core::bsn::FromBsn for ProfileHeroStatSelection {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ProfileHeroStatSelection, found {other:?}"
                )));
            }
        };
        match index {
            1i128 => Ok(Self::AllForSeason(<super::profile::ProfileHeroStatSelectionAllForSeason as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::TopForSeason(<super::profile::ProfileHeroStatSelectionTopForSeason as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ProfileHeroStatSelection variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileHeroStatSelectionAllForSeason {
    #[bsn(name = "m_seasonKey")]
    pub season_key: super::profile::ProfileMmqSeasonKey,
    #[bsn(name = "m_hero")]
    pub hero: Option<FourCc>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileHeroStatSelectionTopForSeason {
    #[bsn(name = "m_seasonKey")]
    pub season_key: super::profile::ProfileMmqSeasonKey,
    #[bsn(name = "m_handle")]
    pub handle: u64,
    #[bsn(name = "m_count")]
    pub count: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileMmqSeasonKey {
    #[bsn(name = "m_mmqId")]
    pub mmq_id: u32,
    #[bsn(name = "m_season")]
    pub season: u16,
}

#[derive(Clone, Debug)]
pub enum ProfileProfileDataResponse {
    Start(super::profile::ProfileProfileDataResponseStart),
    Block(Bytes),
    Failure(u16),
    Cache(()),
}
impl sc2_core::bsn::FromBsn for ProfileProfileDataResponse {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ProfileProfileDataResponse, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Start(<super::profile::ProfileProfileDataResponseStart as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Block(<Bytes as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Failure(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Cache(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ProfileProfileDataResponse variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileProfileDataResponseStart {
    #[bsn(name = "m_numPackets")]
    pub num_packets: u32,
    #[bsn(name = "m_type")]
    pub type_: u32,
}

#[derive(Clone, Debug)]
pub enum ProfileReadSelection {
    All(super::profile::ProfileReadSelectionAll),
    Slice(super::profile::ProfileReadSelectionSlice),
    Random(super::profile::ProfileReadSelectionRandom),
    MultiPath(super::profile::ProfileReadSelectionMultiPath),
    Range(super::profile::ProfileReadSelectionRange),
    HeroStats(super::profile::ProfileHeroStatSelection),
}
impl sc2_core::bsn::FromBsn for ProfileReadSelection {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ProfileReadSelection, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::All(<super::profile::ProfileReadSelectionAll as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Slice(<super::profile::ProfileReadSelectionSlice as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Random(<super::profile::ProfileReadSelectionRandom as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::MultiPath(<super::profile::ProfileReadSelectionMultiPath as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            4i128 => Ok(Self::Range(<super::profile::ProfileReadSelectionRange as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            5i128 => Ok(Self::HeroStats(<super::profile::ProfileHeroStatSelection as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ProfileReadSelection variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileReadSelectionAll {
    #[bsn(name = "m_prefixPath")]
    pub prefix_path: Bytes,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileReadSelectionMultiPath {
    #[bsn(name = "m_paths")]
    pub paths: Vec<Bytes>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileReadSelectionRandom {
    #[bsn(name = "m_prefixPath")]
    pub prefix_path: Bytes,
    #[bsn(name = "m_indices")]
    pub indices: Vec<u64>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileReadSelectionRange {
    #[bsn(name = "m_prefixPath")]
    pub prefix_path: Bytes,
    #[bsn(name = "m_rangeStart")]
    pub range_start: Option<u64>,
    #[bsn(name = "m_rangeEnd")]
    pub range_end: Option<u64>,
    #[bsn(name = "m_rowLimit")]
    pub row_limit: Option<u32>,
    #[bsn(name = "m_singleDepth")]
    pub single_depth: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileReadSelectionSlice {
    #[bsn(name = "m_prefixPath")]
    pub prefix_path: Bytes,
    #[bsn(name = "m_sliceStart")]
    pub slice_start: Option<i64>,
    #[bsn(name = "m_sliceEnd")]
    pub slice_end: Option<i64>,
    #[bsn(name = "m_rowLimit")]
    pub row_limit: Option<u32>,
    #[bsn(name = "m_singleDepth")]
    pub single_depth: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ProfileRecordAddress {
    #[bsn(name = "m_label")]
    pub label: u32,
    #[bsn(name = "m_id")]
    pub id: u64,
}
