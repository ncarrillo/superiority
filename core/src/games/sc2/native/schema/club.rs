#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubClubChangeNotification {
    #[bsn(name = "m_deltas")]
    pub deltas: Vec<super::club::ClubClubChangeInfo>,
}

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
pub struct ClientClubClubSubscribeRequest {
    #[bsn(name = "m_subscriptions")]
    pub subscriptions: Vec<super::club::ClubSubscriptionSyncInfo>,
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
impl superiority_core::bsn::FromBsn for ClientClubGetClubInfoResponseResult {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubGetClubInfoResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Infos(<super::club::ClientClubGetClubInfoResponseResultInfos as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClientClubGetClubInfoResponseResult variant"))),
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
impl superiority_core::bsn::FromBsn for ClientClubGetToonClubsResponseResult {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubGetToonClubsResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::club::ClientClubGetToonClubsResponseResultSuccess as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClientClubGetToonClubsResponseResult variant"))),
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
impl superiority_core::bsn::FromBsn for ClientClubSearchClubsRequestSearch {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubSearchClubsRequestSearch, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Name(<super::club::ClientClubSearchClubsRequestSearchName as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Tag(<super::club::ClientClubSearchClubsRequestSearchTag as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Browse(<super::club::ClientClubSearchClubsRequestSearchBrowse as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Featured(<() as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClientClubSearchClubsRequestSearch variant"))),
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
impl superiority_core::bsn::FromBsn for ClientClubSearchClubsResponseResult {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClientClubSearchClubsResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::club::ClientClubSearchClubsResponseResultSuccess as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<u16 as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClientClubSearchClubsResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientClubSearchClubsResponseResultSuccess {
    #[bsn(name = "m_clubs")]
    pub clubs: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubChangeTypeEnum {
    INSERT,
    UPDATE,
    REMOVE,
    SYNC,
}
impl superiority_core::bsn::FromBsn for ClubChangeTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::INSERT),
            1i128 => Ok(Self::UPDATE),
            2i128 => Ok(Self::REMOVE),
            3i128 => Ok(Self::SYNC),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClubChangeTypeEnum"
            ))),
        }
    }
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
impl superiority_core::bsn::FromBsn for ClubClubCategoryEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
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
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClubClubCategoryEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubChangeInfo {
    #[bsn(name = "m_clubId")]
    pub club_id: u32,
    #[bsn(name = "m_changeType")]
    pub change_type: super::club::ClubChangeTypeEnum,
    #[bsn(name = "m_syncStamp")]
    pub sync_stamp: u64,
    #[bsn(name = "m_info")]
    pub info: super::club::ClubClubChangeInfoInfo,
}

#[derive(Clone, Debug)]
pub enum ClubClubChangeInfoInfo {
    SummaryInfoDeltaList(Vec<super::club::ClubClubSummaryChangeRequest>),
    SummaryInfoFull(super::club::ClubClubSummaryInfo),
    OnlineStatus(()),
    Announcement(super::club::ClubClubUserText),
    AnnouncementSimple(super::club::ClubClubUserTextSimple),
    Event(super::club::ClubClubEvent),
    EventSimple(super::club::ClubClubEventSimple),
    Description(super::club::ClubClubUserText),
    MessageBoard(super::club::ClubClubUserText),
}
impl superiority_core::bsn::FromBsn for ClubClubChangeInfoInfo {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClubClubChangeInfoInfo, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::SummaryInfoDeltaList(<Vec<
                super::club::ClubClubSummaryChangeRequest,
            > as superiority_core::bsn::FromBsn>::from_bsn(
                inner
            )?)),
            1i128 => Ok(Self::SummaryInfoFull(
                <super::club::ClubClubSummaryInfo as superiority_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            2i128 => Ok(Self::OnlineStatus(
                <() as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            3i128 => Ok(Self::Announcement(
                <super::club::ClubClubUserText as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            4i128 => Ok(Self::AnnouncementSimple(
                <super::club::ClubClubUserTextSimple as superiority_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            5i128 => Ok(Self::Event(
                <super::club::ClubClubEvent as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            6i128 => Ok(Self::EventSimple(
                <super::club::ClubClubEventSimple as superiority_core::bsn::FromBsn>::from_bsn(
                    inner,
                )?,
            )),
            7i128 => Ok(Self::Description(
                <super::club::ClubClubUserText as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            8i128 => Ok(Self::MessageBoard(
                <super::club::ClubClubUserText as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a ClubClubChangeInfoInfo variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubEvent {
    #[bsn(name = "m_created")]
    pub created: i32,
    #[bsn(name = "m_author")]
    pub author: u64,
    #[bsn(name = "m_text")]
    pub text: String,
    #[bsn(name = "m_links")]
    pub links: Vec<super::club::ClubClubLinkField>,
    #[bsn(name = "m_eventStartTime")]
    pub event_start_time: i32,
    #[bsn(name = "m_eventEndTime")]
    pub event_end_time: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubEventSimple {
    #[bsn(name = "m_created")]
    pub created: i32,
    #[bsn(name = "m_title")]
    pub title: String,
    #[bsn(name = "m_eventStartTime")]
    pub event_start_time: i32,
    #[bsn(name = "m_eventEndTime")]
    pub event_end_time: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubInfo {
    #[bsn(name = "m_summary")]
    pub summary: super::club::ClubClubSummaryInfo,
    #[bsn(name = "m_status")]
    pub status: super::club::ClubClubOnlineStatus,
}

#[derive(Clone, Debug)]
pub enum ClubClubLinkField {
    Shortlink(super::s2map::S2MapShortLink),
}
impl superiority_core::bsn::FromBsn for ClubClubLinkField {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClubClubLinkField, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Shortlink(
                <super::s2map::S2MapShortLink as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a ClubClubLinkField variant"
            ))),
        }
    }
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

#[derive(Clone, Debug)]
pub enum ClubClubSummaryChangeRequest {
    Name(String),
    Tag(String),
    Type(super::club::ClubClubTypeEnum),
    Category(super::club::ClubClubCategoryEnum),
    Locale(FourCc),
    Flag(super::club::ClubClubSummaryChangeRequestFlag),
    MemberCount(u32),
    ClubFile(super::club::ClubClubSummaryChangeRequestClubFile),
}
impl superiority_core::bsn::FromBsn for ClubClubSummaryChangeRequest {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for ClubClubSummaryChangeRequest, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Name(<String as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Tag(<String as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Type(<super::club::ClubClubTypeEnum as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            3i128 => Ok(Self::Category(<super::club::ClubClubCategoryEnum as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            4i128 => Ok(Self::Locale(<FourCc as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            5i128 => Ok(Self::Flag(<super::club::ClubClubSummaryChangeRequestFlag as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            6i128 => Ok(Self::MemberCount(<u32 as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            7i128 => Ok(Self::ClubFile(<super::club::ClubClubSummaryChangeRequestClubFile as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a ClubClubSummaryChangeRequest variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubSummaryChangeRequestClubFile {
    #[bsn(name = "m_fileHandle")]
    pub file_handle: Option<Bytes>,
    #[bsn(name = "m_fileType")]
    pub file_type: super::club::ClubFileTypeEnum,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubSummaryChangeRequestFlag {
    #[bsn(name = "m_flagValue")]
    pub flag_value: u32,
    #[bsn(name = "m_operation")]
    pub operation: super::flagdelta::FlagDeltaEnum,
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
impl superiority_core::bsn::FromBsn for ClubClubTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::UNSPECIFIED),
            1i128 => Ok(Self::GROUP),
            2i128 => Ok(Self::CLAN),
            3i128 => Ok(Self::TEAM),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClubClubTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubUserText {
    #[bsn(name = "m_created")]
    pub created: i32,
    #[bsn(name = "m_author")]
    pub author: u64,
    #[bsn(name = "m_text")]
    pub text: String,
    #[bsn(name = "m_links")]
    pub links: Vec<super::club::ClubClubLinkField>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubClubUserTextSimple {
    #[bsn(name = "m_created")]
    pub created: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubFileTypeEnum {
    ICON,
    DECAL,
}
impl superiority_core::bsn::FromBsn for ClubFileTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::ICON),
            1i128 => Ok(Self::DECAL),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClubFileTypeEnum"
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
impl superiority_core::bsn::FromBsn for ClubInviteCodeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::INVITED),
            1i128 => Ok(Self::ACCEPTED),
            2i128 => Ok(Self::DECLINED),
            3i128 => Ok(Self::AUTODECLINED),
            other => Err(superiority_core::Error::BsnWire(format!(
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
impl superiority_core::bsn::FromBsn for ClubMemberRankEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
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
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClubMemberRankEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClubSubscriptionSyncInfo {
    #[bsn(name = "m_clubId")]
    pub club_id: u32,
    #[bsn(name = "m_type")]
    pub type_: super::club::ClubSubscriptionTypeEnum,
    #[bsn(name = "m_stamp")]
    pub stamp: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClubSubscriptionTypeEnum {
    INVALID,
    ALL,
    EVENTS,
    EVENTSIMPLE,
    ANNOUNCEMENTS,
    ANNOUNCEMENTSIMPLE,
    MESSAGEBOARD,
    ROSTER,
}
impl superiority_core::bsn::FromBsn for ClubSubscriptionTypeEnum {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        match superiority_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::INVALID),
            1i128 => Ok(Self::ALL),
            2i128 => Ok(Self::EVENTS),
            3i128 => Ok(Self::EVENTSIMPLE),
            4i128 => Ok(Self::ANNOUNCEMENTS),
            5i128 => Ok(Self::ANNOUNCEMENTSIMPLE),
            6i128 => Ok(Self::MESSAGEBOARD),
            7i128 => Ok(Self::ROSTER),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a valid ClubSubscriptionTypeEnum"
            ))),
        }
    }
}
