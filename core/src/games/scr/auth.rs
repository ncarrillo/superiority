//! `classic.protocol.v1.authentication.Authentication.AuthSession`, the first
//! message on the classic websocket. It replays the one-time ticket the game
//! service issued, along with the session key, the account ids, and the install
//! identity.
//!
//! The account half comes from Remastered's own account layer
//! ([`crate::games::scr::aurora`]), not from the protobuf Front channel: the
//! two are different protocols behind the same host, and only Aurora issues a
//! ticket the classic edge will take.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::{
    Error, Result,
    games::scr::{
        CLIENT_CAPABILITIES, GAME_VERSION, SESSION_TYPE, aurora::AuroraSession,
        handoff::ClassicHandoff, handoff::platform_fourcc,
    },
    platform::wire::raw::{self as protobuf, Message},
    product::Product,
};

/// both proof lengths the live edge returns.
const PROOF_LENGTHS: [usize; 2] = [48, 64];

/// `enUS`, packed the way this message wants it.
fn locale_fourcc() -> u32 {
    let locale = Product::Remastered
        .logon()
        .expect("Remastered has a traced logon profile")
        .locale
        .as_bytes();
    locale
        .iter()
        .fold(0u32, |value, byte| (value << 8) | u32::from(*byte))
}

fn account_info(session: &AuroraSession) -> Vec<u8> {
    Message::new()
        .varint(1, session.account_high)
        .varint(2, session.account_low)
        .varint(3, session.game_account_high)
        .varint(4, session.game_account_low)
        .into_vec()
}

fn session_info(session: &AuroraSession) -> Vec<u8> {
    let application_version = Product::Remastered
        .logon()
        .expect("Remastered has a traced logon profile")
        .application_version;
    Message::new()
        .bytes(1, session.session_key.expose())
        .varint(2, u64::from(application_version))
        .varint(3, u64::from(locale_fourcc()))
        .varint(4, u64::from(platform_fourcc()))
        .bytes(6, &account_info(session))
        .varint(7, u64::from(SESSION_TYPE))
        .into_vec()
}

/// `client_identity` is the install identity as its base64 text; the decoded
/// value must be 20 bytes.
pub fn request(
    handoff: &ClassicHandoff,
    session: &AuroraSession,
    client_identity: &[u8],
) -> Result<Vec<u8>> {
    let decoded = BASE64
        .decode(client_identity)
        .map_err(|_| auth_error("classic client identity is not valid base64"))?;
    if decoded.len() != 20 {
        return Err(auth_error(format!(
            "classic client identity decodes to {} bytes; expected 20",
            decoded.len()
        )));
    }
    Ok(Message::new()
        .bytes(1, handoff.ticket.expose())
        .bytes(2, &session_info(session))
        .varint(3, u64::from(Product::Remastered.fourcc()))
        .bytes(4, GAME_VERSION.as_bytes())
        .bytes(5, client_identity)
        .varint(6, u64::from(CLIENT_CAPABILITIES))
        .varint(7, u64::from(platform_fourcc()))
        .varint(8, 1)
        .into_vec())
}

pub fn parse_response(body: &[u8]) -> Result<Vec<u8>> {
    let proof = protobuf::first_bytes(body, 3)
        .ok_or_else(|| auth_error("classic AuthSession response has no server proof"))?;
    if !PROOF_LENGTHS.contains(&proof.len()) {
        return Err(auth_error(format!(
            "classic AuthSession proof is {} bytes; expected one of {PROOF_LENGTHS:?}",
            proof.len()
        )));
    }
    Ok(proof.to_vec())
}

fn auth_error(message: impl Into<String>) -> Error {
    Error::Authentication(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{games::scr::CLIENT_IDENTITY, platform::bgs::SecretBytes};

    fn session() -> AuroraSession {
        AuroraSession {
            session_key: SecretBytes::new(vec![0x5A; 64]).expect("key"),
            account_high: 1,
            account_low: 2,
            game_account_high: 3,
            game_account_low: 4,
            connected_region: 5,
            battle_tag: None,
        }
    }

    fn handoff() -> ClassicHandoff {
        ClassicHandoff::from_url(
            "wss://classic.example.invalid/",
            SecretBytes::new(b"ticket".to_vec()).expect("ticket"),
        )
        .expect("valid url")
    }

    #[test]
    fn builds_the_captured_request_shape() {
        let body = request(&handoff(), &session(), CLIENT_IDENTITY).expect("valid identity");
        let numbers: Vec<u32> = protobuf::fields(&body)
            .map(|field| field.expect("valid body").number)
            .collect();
        assert_eq!(numbers, [1, 2, 3, 4, 5, 6, 7, 8]);

        // field 5 carries the 28-character base64 text, not the decoded bytes
        assert_eq!(protobuf::first_bytes(&body, 5), Some(CLIENT_IDENTITY));

        let info = protobuf::first_bytes(&body, 2).expect("session info");
        assert_eq!(protobuf::first_bytes(info, 1), Some([0x5A; 64].as_slice()));
        let accounts = protobuf::first_bytes(info, 6).expect("account info");
        assert_eq!(protobuf::first_varint(accounts, 4), Some(4));
    }

    #[test]
    fn the_account_ids_come_from_the_aurora_logon() {
        let body = request(&handoff(), &session(), CLIENT_IDENTITY).expect("valid");
        let info = protobuf::first_bytes(&body, 2).expect("session info");
        let accounts = protobuf::first_bytes(info, 6).expect("account info");
        // account high/low then game account high/low, in that order
        assert_eq!(protobuf::first_varint(accounts, 1), Some(1));
        assert_eq!(protobuf::first_varint(accounts, 2), Some(2));
        assert_eq!(protobuf::first_varint(accounts, 3), Some(3));

        assert_eq!(protobuf::first_varint(accounts, 4), Some(4));
    }

    #[test]
    fn rejects_identities_that_are_not_twenty_bytes() {
        assert!(request(&handoff(), &session(), b"not base64!").is_err());
        assert!(request(&handoff(), &session(), BASE64.encode([0u8; 16]).as_bytes()).is_err());
    }

    #[test]
    fn accepts_only_the_observed_proof_lengths() {
        for length in PROOF_LENGTHS {
            let body = Message::new().bytes(3, &vec![7u8; length]).into_vec();
            assert_eq!(parse_response(&body).expect("proof").len(), length);
        }
        assert!(parse_response(&Message::new().varint(1, 0).into_vec()).is_err());
        assert!(parse_response(&Message::new().bytes(3, &[0u8; 32]).into_vec()).is_err());
    }
}
