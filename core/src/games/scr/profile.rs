//! SC:R's ToonProfile avatar lookup.
//!
//! LegacyChat identifies a roster member by toon name, while ToonProfile maps
//! `(program, gateway, toon)` to the member's selected avatar. The response
//! carries both a CDN URL and the stable catalogue id; consumers should prefer
//! the id when they bundle the catalogue locally.

use crate::{
    Error, Result,
    platform::wire::raw::{self as protobuf, Message},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Avatar {
    pub image_url: Option<String>,
    pub id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AvatarLookup {
    pub program_id: u32,
    pub gateway: u64,
    pub toon: String,
    pub avatar: Option<Avatar>,
}

pub(crate) fn get_avatar_request(program_id: u32, gateway: u64, toon: &str) -> Result<Vec<u8>> {
    let toon = toon.trim();
    if toon.is_empty() {
        return Err(Error::ClassicWire(
            "cannot resolve an avatar without a toon name".into(),
        ));
    }
    Ok(Message::new()
        .varint(1, u64::from(program_id))
        .varint(2, gateway)
        .bytes(3, toon.as_bytes())
        .into_vec())
}

pub(crate) fn parse_avatar_response(data: &[u8]) -> Result<AvatarLookup> {
    let mut program_id = None;
    let mut gateway = None;
    let mut toon = None;
    let mut image_url = None;
    let mut avatar_id = None;
    for field in protobuf::fields(data).flatten() {
        match field.number {
            1 => program_id = field.varint().and_then(|value| u32::try_from(value).ok()),
            2 => gateway = field.varint(),
            3 => toon = field.bytes().and_then(text),
            4 => image_url = field.bytes().and_then(non_empty_text),
            5 => avatar_id = field.bytes().and_then(non_empty_text),
            _ => {}
        }
    }
    Ok(AvatarLookup {
        program_id: program_id.ok_or_else(|| malformed("program id"))?,
        gateway: gateway.ok_or_else(|| malformed("gateway"))?,
        toon: toon.ok_or_else(|| malformed("toon name"))?,
        avatar: (image_url.is_some() || avatar_id.is_some()).then_some(Avatar {
            image_url,
            id: avatar_id,
        }),
    })
}

fn text(value: &[u8]) -> Option<String> {
    std::str::from_utf8(value).ok().map(str::to_owned)
}

fn non_empty_text(value: &[u8]) -> Option<String> {
    text(value).filter(|value| !value.is_empty())
}

fn malformed(field: &str) -> Error {
    Error::ClassicWire(format!("ToonProfile.GetAvatar omitted its {field}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bgs::fourcc, games::scr::session::DEFAULT_GATEWAY_CATALOG_ID};

    #[test]
    fn avatar_request_names_the_product_gateway_and_toon() {
        let request = get_avatar_request(fourcc("S1"), DEFAULT_GATEWAY_CATALOG_ID, "ncarrillo1")
            .expect("request");
        assert_eq!(protobuf::first_varint(&request, 1), Some(0x5331));
        assert_eq!(protobuf::first_varint(&request, 2), Some(11));
        assert_eq!(
            protobuf::fields(&request)
                .flatten()
                .find(|field| field.number == 3)
                .and_then(|field| field.bytes()),
            Some("ncarrillo1".as_bytes())
        );
    }

    #[test]
    fn avatar_response_keeps_the_catalogue_id_and_cdn_url() {
        let response = hex::decode(
            "08b1a601100b1a0a6e63617272696c6c6f31\
             225b68747470733a2f2f7363726173736574732e636c61737369632e626c697a7a6172642e636f6d\
             2f6176617461722d69636f6e732f53312f336461626466633230316566666265333263653534653239\
             65626666663535312e706e67\
             2a1a6176617461725f70726f746f73735f61647669736f722e6a7067",
        )
        .expect("captured response");
        let parsed = parse_avatar_response(&response).expect("avatar response");
        assert_eq!(parsed.program_id, fourcc("S1"));
        assert_eq!(parsed.gateway, 11);
        assert_eq!(parsed.toon, "ncarrillo1");
        assert_eq!(
            parsed.avatar,
            Some(Avatar {
                image_url: Some(
                    "https://scrassets.classic.blizzard.com/avatar-icons/S1/3dabdfc201effbe32ce54e29ebfff551.png"
                        .into()
                ),
                id: Some("avatar_protoss_advisor.jpg".into()),
            })
        );
    }

    #[test]
    fn an_account_without_a_selected_avatar_is_a_valid_miss() {
        let response = hex::decode("08b1a601100b1a064461726b6f3222002a00").expect("response");
        assert_eq!(
            parse_avatar_response(&response)
                .expect("avatar response")
                .avatar,
            None
        );
    }
}
