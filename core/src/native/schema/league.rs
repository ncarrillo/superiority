#![allow(dead_code, unused_imports, clippy::all)]

use bsn_derive::FromBsn;
use sc2_core::bsn::{BsnBitArray, Bytes, FourCc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeagueLeagueEnum {
    BRONZE,
    SILVER,
    GOLD,
    PLATINUM,
    DIAMOND,
    MASTER,
    GRANDMASTER,
    TOTALCOUNT,
}
impl sc2_core::bsn::FromBsn for LeagueLeagueEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::BRONZE),
            1i128 => Ok(Self::SILVER),
            2i128 => Ok(Self::GOLD),
            3i128 => Ok(Self::PLATINUM),
            4i128 => Ok(Self::DIAMOND),
            5i128 => Ok(Self::MASTER),
            6i128 => Ok(Self::GRANDMASTER),
            7i128 => Ok(Self::TOTALCOUNT),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a valid LeagueLeagueEnum"))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeagueTeamTypeEnum {
    ARRANGED,
    RANDOM,
    TOTALCOUNT,
}
impl sc2_core::bsn::FromBsn for LeagueTeamTypeEnum {
    fn from_bsn(value: &sc2_core::bsn::value::BsnValue) -> sc2_core::Result<Self> {
        match sc2_core::bsn::FromBsn::from_bsn(value)? {
            0i128 => Ok(Self::ARRANGED),
            1i128 => Ok(Self::RANDOM),
            2i128 => Ok(Self::TOTALCOUNT),
            other => Err(sc2_core::Error::BsnWire(format!("{other} is not a valid LeagueTeamTypeEnum"))),
        }
    }
}

