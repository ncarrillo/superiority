#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsAccountBlockAddedNotify {
    #[bsn(name = "m_blocks")]
    pub blocks: Vec<super::friends::FriendsAccountBlockContainer>,
    #[bsn(name = "m_isEndOfList")]
    pub is_end_of_list: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsFriendInvitationAddedNotify {
    #[bsn(name = "m_invitation")]
    pub invitation: super::friends::FriendsFriendInvitation,
    #[bsn(name = "m_isEndOfList")]
    pub is_end_of_list: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsFriendsListNotify5 {
    #[bsn(name = "m_friends")]
    pub friends: Vec<super::friends::FriendsFriendshipUpdate5>,
    #[bsn(name = "m_isEndOfList")]
    pub is_end_of_list: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsToonBlockNotify {
    #[bsn(name = "m_blocks")]
    pub blocks: Vec<super::friends::FriendsToonBlockContainer>,
    #[bsn(name = "m_isEndOfList")]
    pub is_end_of_list: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsToonsOfFriendPacket {}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsToonsOfFriendsNotify {
    #[bsn(name = "ToonsOfFriendPacket")]
    pub toons_of_friend_packet: super::friends::ClientFriendsToonsOfFriendPacket,
    #[bsn(name = "m_toons")]
    pub toons: Vec<super::friends::FriendsToonOfFriend>,
    #[bsn(name = "m_isEndOfList")]
    pub is_end_of_list: bool,
}

#[derive(Clone, Debug, FromBsn)]
pub struct ClientFriendsToonsOfFriendsRequest {
    #[bsn(name = "ToonsOfFriendPacket")]
    pub toons_of_friend_packet: super::friends::ClientFriendsToonsOfFriendPacket,
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsAccountBlockContainer {
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
    #[bsn(name = "m_fullName")]
    pub full_name: Option<super::account::AccountFullName>,
    #[bsn(name = "m_nickname")]
    pub nickname: Option<String>,
    #[bsn(name = "m_role")]
    pub role: u32,
}

#[derive(Clone, Debug)]
pub enum FriendsFriendContainer5 {
    Character(super::friends::FriendsFriendContainer5Character),
    Account(super::friends::FriendsFriendContainer5Account),
    PersistentPresenceUpdate(super::friends::FriendsFriendContainer5PersistentPresenceUpdate),
}
impl superiority_core::bsn::FromBsn for FriendsFriendContainer5 {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for FriendsFriendContainer5, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Character(<super::friends::FriendsFriendContainer5Character as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Account(<super::friends::FriendsFriendContainer5Account as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::PersistentPresenceUpdate(<super::friends::FriendsFriendContainer5PersistentPresenceUpdate as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a FriendsFriendContainer5 variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendContainer5Account {
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
    #[bsn(name = "m_fullName")]
    pub full_name: Option<super::account::AccountFullName>,
    #[bsn(name = "m_nickname")]
    pub nickname: Option<String>,
    #[bsn(name = "m_profile")]
    pub profile: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_customMsg")]
    pub custom_msg: Option<super::presence::PresenceCustomMessage>,
    #[bsn(name = "m_note")]
    pub note: Option<String>,
    #[bsn(name = "m_lastOnline")]
    pub last_online: i32,
    #[bsn(name = "m_marketingFlags")]
    pub marketing_flags: u64,
    #[bsn(name = "m_friendshipRole")]
    pub friendship_role: u32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendContainer5Character {
    #[bsn(name = "m_toonInfo")]
    pub toon_info: super::toon::ToonInfo,
    #[bsn(name = "m_profile")]
    pub profile: super::profile::ProfileRecordAddress,
    #[bsn(name = "m_note")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendContainer5PersistentPresenceUpdate {
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
    #[bsn(name = "m_customMsg")]
    pub custom_msg: Option<super::presence::PresenceCustomMessage>,
    #[bsn(name = "m_lastOnline")]
    pub last_online: i32,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendInvitation {
    #[bsn(name = "m_presenceId")]
    pub presence_id: Option<u32>,
    #[bsn(name = "m_accountId")]
    pub account_id: u32,
    #[bsn(name = "m_fullName")]
    pub full_name: Option<super::account::AccountFullName>,
    #[bsn(name = "m_nickname")]
    pub nickname: Option<String>,
    #[bsn(name = "m_role")]
    pub role: u32,
    #[bsn(name = "m_createdTime")]
    pub created_time: i32,
    #[bsn(name = "m_profile")]
    pub profile: Option<super::profile::ProfileRecordAddress>,
    #[bsn(name = "m_popupToast")]
    pub popup_toast: Option<bool>,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendshipUpdate5 {
    #[bsn(name = "m_update")]
    pub update: super::friends::FriendsFriendshipUpdate5Update,
}

#[derive(Clone, Debug)]
pub enum FriendsFriendshipUpdate5Update {
    Add(super::friends::FriendsFriendshipUpdate5UpdateAdd),
    Remove(super::friends::FriendsFriendshipUpdate5UpdateRemove),
    Modify(super::friends::FriendsFriendshipUpdate5UpdateModify),
}
impl superiority_core::bsn::FromBsn for FriendsFriendshipUpdate5Update {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for FriendsFriendshipUpdate5Update, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Add(<super::friends::FriendsFriendshipUpdate5UpdateAdd as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Remove(<super::friends::FriendsFriendshipUpdate5UpdateRemove as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            2i128 => Ok(Self::Modify(<super::friends::FriendsFriendshipUpdate5UpdateModify as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a FriendsFriendshipUpdate5Update variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendshipUpdate5UpdateAdd {
    #[bsn(name = "m_container")]
    pub container: super::friends::FriendsFriendContainer5,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendshipUpdate5UpdateModify {
    #[bsn(name = "m_container")]
    pub container: super::friends::FriendsFriendContainer5,
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsFriendshipUpdate5UpdateRemove {
    #[bsn(name = "m_id")]
    pub id: super::friends::FriendsPriorFriendId,
}

#[derive(Clone, Debug)]
pub enum FriendsPriorFriendId {
    AccountId(u32),
    ToonHandle(super::toon::ToonHandle),
}
impl superiority_core::bsn::FromBsn for FriendsPriorFriendId {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for FriendsPriorFriendId, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::AccountId(
                <u32 as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            1i128 => Ok(Self::ToonHandle(
                <super::toon::ToonHandle as superiority_core::bsn::FromBsn>::from_bsn(inner)?,
            )),
            other => Err(superiority_core::Error::BsnWire(format!(
                "{other} is not a FriendsPriorFriendId variant"
            ))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsToonBlockContainer {
    #[bsn(name = "m_toonName")]
    pub toon_name: super::toon::ToonFullName,
    #[bsn(name = "m_update")]
    pub update: super::friends::FriendsToonBlockContainerUpdate,
}

#[derive(Clone, Debug)]
pub enum FriendsToonBlockContainerUpdate {
    Add(super::friends::FriendsToonBlockContainerUpdateAdd),
    Remove(super::friends::FriendsToonBlockContainerUpdateRemove),
}
impl superiority_core::bsn::FromBsn for FriendsToonBlockContainerUpdate {
    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {
        let (index, inner) = match value {
            superiority_core::bsn::value::BsnValue::Choice { index, value } => {
                (*index, value.as_ref())
            }
            other => {
                return Err(superiority_core::Error::BsnWire(format!(
                    "expected a choice for FriendsToonBlockContainerUpdate, found {other:?}"
                )));
            }
        };
        match index {
            0i128 => Ok(Self::Add(<super::friends::FriendsToonBlockContainerUpdateAdd as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            1i128 => Ok(Self::Remove(<super::friends::FriendsToonBlockContainerUpdateRemove as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),
            other => Err(superiority_core::Error::BsnWire(format!("{other} is not a FriendsToonBlockContainerUpdate variant"))),
        }
    }
}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsToonBlockContainerUpdateAdd {}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsToonBlockContainerUpdateRemove {}

#[derive(Clone, Debug, FromBsn)]
pub struct FriendsToonOfFriend {
    #[bsn(name = "m_friendAccountId")]
    pub friend_account_id: u32,
    #[bsn(name = "m_toon")]
    pub toon: super::toon::ToonFullName,
    #[bsn(name = "m_profile")]
    pub profile: super::profile::ProfileRecordAddress,
}
