//! The small gateway population snapshot shown when SC:R chat comes online.
//!
//! Retail requests this separately from `LegacyChat.Connect`; the welcome and
//! help prose around it is client-owned UI text. The request uses the retail
//! catalogue ID (11 for U.S. East). Only the player and game counts are used;
//! the middle response field is server-owned metadata and is not stable.

use crate::{
    Error, Result,
    platform::wire::raw::{self as protobuf, Message},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayStats {
    pub players_online: u64,
    pub games_being_played: u64,
}

pub fn request(catalog_id: u64) -> Result<Vec<u8>> {
    if catalog_id == 0 {
        return Err(gateway_error("gateway catalogue ID must be positive"));
    }
    Ok(Message::new().varint(1, catalog_id).into_vec())
}

pub fn parse_response(body: &[u8]) -> Result<GatewayStats> {
    let players_online = protobuf::first_varint(body, 1)
        .ok_or_else(|| gateway_error("GetGatewayStats returned no player count"))?;
    let games_being_played = protobuf::first_varint(body, 3)
        .ok_or_else(|| gateway_error("GetGatewayStats returned no game count"))?;
    Ok(GatewayStats {
        players_online,
        games_being_played,
    })
}

fn gateway_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::scr::session::DEFAULT_GATEWAY_CATALOG_ID;

    #[test]
    fn requests_the_retail_catalogue_id() {
        let body = request(DEFAULT_GATEWAY_CATALOG_ID).expect("valid gateway");
        assert_eq!(protobuf::first_varint(&body, 1), Some(11));
    }

    #[test]
    fn decodes_the_live_gateway_population_shape() {
        // Captured live on U.S. East. Field 2 has been observed as both 2 and
        // 0, so it must not be interpreted as a gateway identity check.
        let body = Message::new()
            .varint(1, 47)
            .varint(2, 0)
            .varint(3, 32)
            .into_vec();
        assert_eq!(
            parse_response(&body).expect("live response"),
            GatewayStats {
                players_online: 47,
                games_being_played: 32,
            }
        );
    }

    #[test]
    fn does_not_require_the_unstable_middle_field() {
        let body = Message::new().varint(1, 48).varint(3, 28).into_vec();
        assert_eq!(
            parse_response(&body).expect("population counts"),
            GatewayStats {
                players_online: 48,
                games_being_played: 28,
            }
        );
    }

    #[test]
    fn rejects_the_empty_response_returned_for_a_wrong_request_id() {
        assert!(parse_response(&[]).is_err());
    }
}
