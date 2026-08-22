#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonBillingUpdateNotify {
    #[bsn(name = "m_info")]
    pub info: super::session::SessionBillingInfo,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonCaisTimeUpdate {
    #[bsn(name = "m_rested")]
    pub rested: u32,
    #[bsn(name = "m_played")]
    pub played: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonFailure {
    #[bsn(name = "m_error")]
    pub error: u16,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonInitialNotifiesComplete {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonCreateCancel {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonCreateFinal {
    #[bsn(name = "m_toonCreateData")]
    pub toon_create_data: super::toon::ToonToonCreationData,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonCreateInit {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonCreateSetup {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonCreated {
    #[bsn(name = "m_toonName")]
    pub toon_name: String,
    #[bsn(name = "m_realm")]
    pub realm: u32,
    #[bsn(name = "m_recordAddress")]
    pub record_address: super::profile::ProfileRecordAddress,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonList {
    #[bsn(name = "m_toonDisplays")]
    pub toon_displays: Vec<super::toon::ToonDisplay>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonToonSelected {
    #[bsn(name = "m_toonName")]
    pub toon_name: String,
    #[bsn(name = "m_toonHandle")]
    pub toon_handle: super::toon::ToonHandle,
    #[bsn(name = "m_realm")]
    pub realm: u32,
    #[bsn(name = "m_recordAddress")]
    pub record_address: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_lastLogon")]
    pub last_logon: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientToonWelcome {
    #[bsn(name = "m_unlockablesFiles")]
    pub unlockables_files: Vec<super::toon::ToonUnlockDefinitionFile>,
    #[bsn(name = "m_defaultPortrait")]
    pub default_portrait: u32,
    #[bsn(name = "m_intermediateNameRestriction")]
    pub intermediate_name_restriction: String,
    #[bsn(name = "m_finalNameRestriction")]
    pub final_name_restriction: String,
    #[bsn(name = "m_achievementHandles")]
    pub achievement_handles: Vec<super::achievement::AchievementProgramHandleAggregation>,
    #[bsn(name = "m_depotRegion")]
    pub depot_region: FourCc,
    #[bsn(name = "m_maxMapFavorites")]
    pub max_map_favorites: u16,
    #[bsn(name = "m_realmMapList")]
    pub realm_map_list: Vec<super::realm::RealmRealmMap>,
    #[bsn(name = "m_currentTime")]
    pub current_time: i32,
    #[bsn(name = "m_programFlags")]
    pub program_flags: BsnBitArray,
    #[bsn(name = "m_programName")]
    pub program_name: Bytes,
    #[bsn(name = "m_isPlayingFromIGR")]
    pub is_playing_from_igr: bool,
    #[bsn(name = "m_maxGameServerConnectTimeoutMS")]
    pub max_game_server_connect_timeout_ms: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ToonDisplay {
    #[bsn(name = "m_name")]
    pub name: String,
    #[bsn(name = "m_realm")]
    pub realm: u32,
    #[bsn(name = "m_profile")]
    pub profile: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_lastOnline")]
    pub last_online: i32,
    #[bsn(name = "m_flags")]
    pub flags: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ToonFullName {
    #[bsn(name = "m_region")]
    pub region: u8,
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
    #[bsn(name = "m_realm")]
    pub realm: u32,
    #[bsn(name = "m_name")]
    pub name: String,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ToonHandle {
    #[bsn(name = "m_region")]
    pub region: u8,
    #[bsn(name = "m_programId")]
    pub program_id: FourCc,
    #[bsn(name = "m_realm")]
    pub realm: u32,
    #[bsn(name = "m_id")]
    pub id: u64,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ToonInfo {
    #[bsn(name = "Handle")]
    pub handle: super::toon::ToonHandle,
    #[bsn(name = "m_name")]
    pub name: String,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ToonToonCreationData {
    #[bsn(name = "m_displayName")]
    pub display_name: String,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ToonUnlockDefinitionFile {
    #[bsn(name = "m_unlockDefinitionFileCacheHandle")]
    pub unlock_definition_file_cache_handle: Bytes,
}
