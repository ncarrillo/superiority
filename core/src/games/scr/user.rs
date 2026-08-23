//! account-wide player status carried on the classic RPC connection.
//!
//! The SDK calls this service `AuroraUser`, but it is not the JSON Aurora
//! bootstrap socket. Its generated protobuf request is sent beside LegacyChat
//! on the classic channel.

use crate::platform::wire::raw::{self as protobuf, Message};

/// the three values accepted by `AuroraUserV1Impl::SetPlayerStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStatus {
    Online,
    Away,
    Busy,
}

/// builds `aurora_user.UpdatePlayerStatusRequest`.
///
/// The SDK maps its public status values 0, 1, and 2 to three mutually
/// exclusive booleans. It explicitly marks all three fields present, including
/// the two false values, so this encoder does the same.
#[must_use]
pub fn update_player_status_request(status: PlayerStatus) -> Vec<u8> {
    let (online, away, busy) = match status {
        PlayerStatus::Online => (true, false, false),
        PlayerStatus::Away => (false, true, false),
        PlayerStatus::Busy => (false, false, true),
    };
    Message::new()
        .varint(1, u64::from(online))
        .varint(2, u64::from(away))
        .varint(3, u64::from(busy))
        .into_vec()
}

/// decodes the same message when it arrives in `PlayerStatusUpdated`.
#[must_use]
pub fn parse_player_status(body: &[u8]) -> Option<PlayerStatus> {
    let online = protobuf::first_varint(body, 1).is_some_and(|value| value != 0);
    let away = protobuf::first_varint(body, 2).is_some_and(|value| value != 0);
    let busy = protobuf::first_varint(body, 3).is_some_and(|value| value != 0);
    match (online, away, busy) {
        (true, false, false) => Some(PlayerStatus::Online),
        (false, true, false) => Some(PlayerStatus::Away),
        (false, false, true) => Some(PlayerStatus::Busy),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_all_three_presence_booleans_exactly_like_the_sdk() {
        assert_eq!(
            update_player_status_request(PlayerStatus::Online),
            [0x08, 0x01, 0x10, 0x00, 0x18, 0x00]
        );
        assert_eq!(
            update_player_status_request(PlayerStatus::Away),
            [0x08, 0x00, 0x10, 0x01, 0x18, 0x00]
        );
        assert_eq!(
            update_player_status_request(PlayerStatus::Busy),
            [0x08, 0x00, 0x10, 0x00, 0x18, 0x01]
        );
    }

    #[test]
    fn decodes_only_a_single_selected_status() {
        for status in [PlayerStatus::Online, PlayerStatus::Away, PlayerStatus::Busy] {
            assert_eq!(
                parse_player_status(&update_player_status_request(status)),
                Some(status)
            );
        }
        assert_eq!(parse_player_status(&[]), None);
        assert_eq!(parse_player_status(&[0x08, 1, 0x10, 1]), None);
    }
}
