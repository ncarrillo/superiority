#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MapGameGroupUpdate {
    #[bsn(name = "m_isFirst")]
    pub is_first: bool,
    #[bsn(name = "m_isLast")]
    pub is_last: bool,
    #[bsn(name = "m_tag")]
    pub tag: FourCc,
    #[bsn(name = "m_results")]
    pub results: super::s2map::ClientS2MapGameGroupUpdateResults,
}

#[derive(Clone, Debug)]
pub enum ClientS2MapGameGroupUpdateResults {
    GameGroup(Vec<super::s2map::S2MapShortLink>),
    MapGroup(Vec<u32>),
}
impl sc2_core::bsn::FromBsn for ClientS2MapGameGroupUpdateResults {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientS2MapGameGroupUpdateResults, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::GameGroup(
                <Vec<super::s2map::S2MapShortLink> as sc2_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            1i128 => Ok(Self::MapGroup(
                <Vec<u32> as sc2_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a ClientS2MapGameGroupUpdateResults variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MapS2ListMapFavorites {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MapS2ListMapFavoritesRequest {
    #[bsn(name = "S2ListMapFavorites")]
    pub s2_list_map_favorites: super::s2map::ClientS2MapS2ListMapFavorites,
    #[bsn(name = "m_toonFullName")]
    pub toon_full_name: Option<super::toon::ToonFullName>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MapS2ListMapFavoritesResponse {
    #[bsn(name = "S2ListMapFavorites")]
    pub s2_list_map_favorites: super::s2map::ClientS2MapS2ListMapFavorites,
    #[bsn(name = "m_result")]
    pub result: super::s2map::ClientS2MapS2ListMapFavoritesResponseResult,
}

#[derive(Clone, Debug)]
pub enum ClientS2MapS2ListMapFavoritesResponseResult {
    Success(super::s2map::ClientS2MapS2ListMapFavoritesResponseResultSuccess),
    Failure(super::s2map::ClientS2MapS2ListMapFavoritesResponseResultFailure),
}
impl sc2_core::bsn::FromBsn for ClientS2MapS2ListMapFavoritesResponseResult {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        let (index, inner) = match value {
            sc2_core::bsn::value::BsnValue::Choice { index, value } => (*index, value.as_ref()),
            other => {
                return Err(sc2_core::Error::BsnWire(format!(
                    "expected a choice for ClientS2MapS2ListMapFavoritesResponseResult, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Success(<super::s2map::ClientS2MapS2ListMapFavoritesResponseResultSuccess as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Failure(<super::s2map::ClientS2MapS2ListMapFavoritesResponseResultFailure as sc2_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a ClientS2MapS2ListMapFavoritesResponseResult variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MapS2ListMapFavoritesResponseResultFailure {
    #[bsn(name = "m_reason")]
    pub reason: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientS2MapS2ListMapFavoritesResponseResultSuccess {
    #[bsn(name = "m_favorites")]
    pub favorites: Vec<super::s2map::S2MapMapFavorite>,
    #[bsn(name = "m_endOfList")]
    pub end_of_list: bool,
    #[bsn(name = "m_externalProfile")]
    pub external_profile: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapHandle {
    #[bsn(name = "m_id")]
    pub id: u32,
    #[bsn(name = "m_version")]
    pub version: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapLinkEntry {
    #[bsn(name = "m_handle")]
    pub handle: super::s2map::S2MapHandle,
    #[bsn(name = "m_type")]
    pub type_: super::s2map::S2MapLinkEntryTypeEnum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S2MapLinkEntryTypeEnum {
    PRIMARY,
    OPTIONAL,
    DEPENDENCY,
}
impl sc2_core::bsn::FromBsn for S2MapLinkEntryTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::PRIMARY),
            1i128 => Ok(Self::OPTIONAL),
            2i128 => Ok(Self::DEPENDENCY),
            other => Err(sc2_core::Error::BsnWire(format!(
                "{other} is not a valid S2MapLinkEntryTypeEnum"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapMapFavorite {
    #[bsn(name = "m_link")]
    pub link: super::s2map::S2MapShortLink,
    #[bsn(name = "m_timeAdded")]
    pub time_added: i32,
    #[bsn(name = "m_profileIndex")]
    pub profile_index: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct S2MapShortLink {
    #[bsn(name = "m_variantIndex")]
    pub variant_index: u8,
    #[bsn(name = "m_speed")]
    pub speed: FourCc,
    #[bsn(name = "m_entries")]
    pub entries: Vec<super::s2map::S2MapLinkEntry>,
}
