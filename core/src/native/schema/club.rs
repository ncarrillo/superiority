#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubClubSettings {
    #[bsn(name = "m_cacheSettings")]
    pub cache_settings: super::club::ClientClubClubSettingsCacheSettings,
    #[bsn(name = "m_clubNameRegEx")]
    pub club_name_reg_ex: String,
    #[bsn(name = "m_clubTagRegEx")]
    pub club_tag_reg_ex: String,
    #[bsn(name = "m_eventExpirySec")]
    pub event_expiry_sec: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubClubSettingsCacheSettings {
    #[bsn(name = "m_infoCacheMaxSize")]
    pub info_cache_max_size: u32,
    #[bsn(name = "m_memberCacheMaxSize")]
    pub member_cache_max_size: u32,
    #[bsn(name = "m_infoCacheExpirySec")]
    pub info_cache_expiry_sec: i32,
    #[bsn(name = "m_onlineStatusExpirySec")]
    pub online_status_expiry_sec: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetClubInfoRequest {
    #[bsn(name = "m_token")]
    pub token: u32,
    #[bsn(name = "m_clubs")]
    pub clubs: Vec<u32>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetClubInfoResponse {
    #[bsn(name = "m_token")]
    pub token: u32,
    #[bsn(name = "m_result")]
    pub result: super::club::ClientClubGetClubInfoResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientClubGetClubInfoResponseResult {
    Infos(super::club::ClientClubGetClubInfoResponseResultInfos),
    Failure(u16),
}
impl sc2_core::bsn::FromBsn for ClientClubGetClubInfoResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubGetClubInfoResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Infos(<super::club::ClientClubGetClubInfoResponseResultInfos as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientClubGetClubInfoResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetClubInfoResponseResultInfos {
    #[bsn(name = "m_infoList")]
    pub info_list: Vec<super::club::ClubClubInfo>,
    #[bsn(name = "m_isLastPacket")]
    pub is_last_packet: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetMemberClanTags {
    #[bsn(name = "m_token")]
    pub token: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetMemberClanTagsResponse {
    #[bsn(name = "GetMemberClanTags")]
    pub get_member_clan_tags: super::club::ClientClubGetMemberClanTags,
    #[bsn(name = "m_result")]
    pub result: u16,
    #[bsn(name = "m_clubId")]
    pub club_id: Option<u32>,
    #[bsn(name = "m_clubTag")]
    pub club_tag: Option<String>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetToonClubs {
    #[bsn(name = "m_token")]
    pub token: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetToonClubsResponse {
    #[bsn(name = "GetToonClubs")]
    pub get_toon_clubs: super::club::ClientClubGetToonClubs,
    #[bsn(name = "m_result")]
    pub result: super::club::ClientClubGetToonClubsResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientClubGetToonClubsResponseResult {
    Success(super::club::ClientClubGetToonClubsResponseResultSuccess),
    Failure(u16),
}
impl sc2_core::bsn::FromBsn for ClientClubGetToonClubsResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubGetToonClubsResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::club::ClientClubGetToonClubsResponseResultSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientClubGetToonClubsResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubGetToonClubsResponseResultSuccess {
    #[bsn(name = "m_clubInfo")]
    pub club_info: Vec<super::club::ClubClubInfo>,
    #[bsn(name = "m_rankInfo")]
    pub rank_info: Vec<super::club::ClubMemberRankEnum>,
    #[bsn(name = "m_isLastPacket")]
    pub is_last_packet: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubInviteAction {
    #[bsn(name = "m_action")]
    pub action: super::club::ClubInviteAction,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubs {
    #[bsn(name = "m_token")]
    pub token: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsRequest {
    #[bsn(name = "SearchClubs")]
    pub search_clubs: super::club::ClientClubSearchClubs,
    #[bsn(name = "m_search")]
    pub search: super::club::ClientClubSearchClubsRequestSearch,
}

#[derive(Clone, Debug)]
pub enum ClientClubSearchClubsRequestSearch {
    Name(super::club::ClientClubSearchClubsRequestSearchName),
    Tag(super::club::ClientClubSearchClubsRequestSearchTag),
    Browse(super::club::ClientClubSearchClubsRequestSearchBrowse),
    Featured(()),
}
impl sc2_core::bsn::FromBsn for ClientClubSearchClubsRequestSearch {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubSearchClubsRequestSearch, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Name(<super::club::ClientClubSearchClubsRequestSearchName as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Tag(<super::club::ClientClubSearchClubsRequestSearchTag as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Browse(<super::club::ClientClubSearchClubsRequestSearchBrowse as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Featured(<() as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientClubSearchClubsRequestSearch variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsRequestSearchBrowse {
    #[bsn(name = "m_page")]
    pub page: u32,
    #[bsn(name = "m_type")]
    pub type_: super::club::ClubClubTypeEnum,
    #[bsn(name = "m_category")]
    pub category: super::club::ClubClubCategoryEnum,
    #[bsn(name = "m_locale")]
    pub locale: FourCc,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsRequestSearchName {
    #[bsn(name = "m_name")]
    pub name: String,
    #[bsn(name = "m_type")]
    pub type_: super::club::ClubClubTypeEnum,
    #[bsn(name = "m_category")]
    pub category: super::club::ClubClubCategoryEnum,
    #[bsn(name = "m_locale")]
    pub locale: FourCc,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsRequestSearchTag {
    #[bsn(name = "m_tag")]
    pub tag: String,
    #[bsn(name = "m_type")]
    pub type_: super::club::ClubClubTypeEnum,
    #[bsn(name = "m_category")]
    pub category: super::club::ClubClubCategoryEnum,
    #[bsn(name = "m_locale")]
    pub locale: FourCc,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsResponse {
    #[bsn(name = "SearchClubs")]
    pub search_clubs: super::club::ClientClubSearchClubs,
    #[bsn(name = "m_result")]
    pub result: super::club::ClientClubSearchClubsResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientClubSearchClubsResponseResult {
    Success(super::club::ClientClubSearchClubsResponseResultSuccess),
    Failure(u16),
}
impl sc2_core::bsn::FromBsn for ClientClubSearchClubsResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubSearchClubsResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::club::ClientClubSearchClubsResponseResultSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientClubSearchClubsResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsResponseResultSuccess {
    #[bsn(name = "m_clubs")]
    pub clubs: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubClubCategoryEnum {
    UNSPECIFIED,
    COMMUNITY,
    BARCRAFT,
    ESPORTSTEAMS,
    COACHING,
    COMPANY,
    REGION,
    SCHOOL,
    SHOUTCAST,
    OTHER,
    ESPORTSLEAGUES,
    ARCADE,
    IGR,
}
impl sc2_core::bsn::FromBsn for ClubClubCategoryEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNSPECIFIED),
            1i128 => Ok(Self::COMMUNITY),
            2i128 => Ok(Self::BARCRAFT),
            3i128 => Ok(Self::ESPORTSTEAMS),
            4i128 => Ok(Self::COACHING),
            5i128 => Ok(Self::COMPANY),
            6i128 => Ok(Self::REGION),
            7i128 => Ok(Self::SCHOOL),
            8i128 => Ok(Self::SHOUTCAST),
            9i128 => Ok(Self::OTHER),
            10i128 => Ok(Self::ESPORTSLEAGUES),
            11i128 => Ok(Self::ARCADE),
            12i128 => Ok(Self::IGR),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ClubClubCategoryEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubInfo {
    #[bsn(name = "m_summary")]
    pub summary: super::club::ClubClubSummaryInfo,
    #[bsn(name = "m_status")]
    pub status: super::club::ClubClubOnlineStatus,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubOnlineStatus {
    #[bsn(name = "m_online")]
    pub online: u32,
    #[bsn(name = "m_ingame")]
    pub ingame: u32,
    #[bsn(name = "m_inchat")]
    pub inchat: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubSummaryInfo {
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_program")]
    pub program: FourCc,
    #[bsn(name = "m_name")]
    pub name: String,
    #[bsn(name = "m_tag")]
    pub tag: Option<String>,
    #[bsn(name = "m_type")]
    pub type_: super::club::ClubClubTypeEnum,
    #[bsn(name = "m_category")]
    pub category: super::club::ClubClubCategoryEnum,
    #[bsn(name = "m_locale")]
    pub locale: FourCc,
    #[bsn(name = "m_flags")]
    pub flags: u32,
    #[bsn(name = "m_fileHandles")]
    pub file_handles: Vec<Option<Bytes>>,
    #[bsn(name = "m_recordAddress")]
    pub record_address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_memberCount")]
    pub member_count: u32,
    #[bsn(name = "m_created")]
    pub created: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubClubTypeEnum {
    UNSPECIFIED,
    GROUP,
    CLAN,
    TEAM,
}
impl sc2_core::bsn::FromBsn for ClubClubTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNSPECIFIED),
            1i128 => Ok(Self::GROUP),
            2i128 => Ok(Self::CLAN),
            3i128 => Ok(Self::TEAM),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ClubClubTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubInviteAction {
    #[bsn(name = "m_clubId")]
    pub club_id: u32,
    #[bsn(name = "m_member")]
    pub member: super::toon::ToonHandle,
    #[bsn(name = "m_code")]
    pub code: super::club::ClubInviteCodeEnum,
    #[bsn(name = "m_result")]
    pub result: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubInviteCodeEnum {
    INVITED,
    ACCEPTED,
    DECLINED,
    AUTODECLINED,
}
impl sc2_core::bsn::FromBsn for ClubInviteCodeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::INVITED),
            1i128 => Ok(Self::ACCEPTED),
            2i128 => Ok(Self::DECLINED),
            3i128 => Ok(Self::AUTODECLINED),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ClubInviteCodeEnum"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubMemberRankEnum {
    NOTSET,
    BANNED,
    VISITOR,
    REJECTED,
    REQUESTED,
    SUGGESTED,
    INVITED,
    BENCHWARMER,
    HONORARY,
    MEMBER,
    OFFICER,
    OWNER,
}
impl sc2_core::bsn::FromBsn for ClubMemberRankEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::NOTSET),
            5i128 => Ok(Self::BANNED),
            10i128 => Ok(Self::VISITOR),
            13i128 => Ok(Self::REJECTED),
            16i128 => Ok(Self::REQUESTED),
            18i128 => Ok(Self::SUGGESTED),
            20i128 => Ok(Self::INVITED),
            24i128 => Ok(Self::BENCHWARMER),
            25i128 => Ok(Self::HONORARY),
            30i128 => Ok(Self::MEMBER),
            40i128 => Ok(Self::OFFICER),
            50i128 => Ok(Self::OWNER),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid ClubMemberRankEnum"
            ))),
        }
    }
}
