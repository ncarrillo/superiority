//! Remastered's account layer.
//!
//! This is **not** [`crate::platform::bgs`]. Both answer at
//! `wss://us.actual.battle.net:1119/`, but the server routes by websocket
//! subprotocol and they are different protocols behind it:
//!
//! | | Aurora (here) | Front ([`crate::platform::bgs`]) |
//! |---|---|---|
//! | subprotocol | `jsonrpc.aurora.v1.30.battle.net` | `v1.rpc.battle.net` |
//! | frames | JSON `[header, body]` pairs | protobuf |
//!
//! Signing in on the protobuf channel and asking it for a classic server does
//! answer — with an endpoint meant for a different kind of client, whose route
//! is not there. The channel a client is given depends on the one it asked on,
//! so Remastered asks here.
//!
//! Ported from `sc1-research`, recovered from build `1.23.10_2e031d5be4`.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json};
use url::Url;

use crate::{
    Error, Result,
    games::scr::{
        GAME_VERSION,
        catalog::aurora as ids,
        handoff::{ClassicHandoff, platform_fourcc},
    },
    platform::{
        bgs::SecretBytes,
        wire::{
            raw::{self as protobuf, Message},
            websocket::{RpcSocket, SocketProfile},
        },
    },
    product::Product,
};

pub const AURORA_HOST: &str = "us.actual.battle.net";
pub const AURORA_PORT: u16 = 1119;
const AURORA_PATH: &str = "/";
const AURORA_SUBPROTOCOL: &str = "jsonrpc.aurora.v1.30.battle.net";

/// the non-secret marker that asks Battle.net for a fresh challenge rather than
/// replaying a credential. Not a key: it is all zeroes by construction.
pub const CHALLENGE_BOOTSTRAP_CREDENTIAL: &[u8] = b"US-00000000000000000000000000000000-0000";

const SESSION_KEY_BYTES: usize = 64;

/// what the logon hands back, and what the classic channel replays.
#[derive(Debug)]
pub struct AuroraSession {
    pub session_key: SecretBytes,
    pub account_high: u64,
    pub account_low: u64,
    pub game_account_high: u64,
    pub game_account_low: u64,
    pub connected_region: u64,
    /// `LogonResult.battle_tag` (field 7), when the server includes it in the
    /// JSON logon result. It is the account-wide name available before any
    /// channel answers, and the only handle the classic session can match its
    /// own roster entry against.
    pub battle_tag: Option<String>,
}

/// resolves a Battle.net web sign-in, returning the credential it produced.
pub trait ChallengeHandler {
    fn resolve(&mut self, url: &Url) -> Result<SecretBytes>;
}

impl<F: FnMut(&Url) -> Result<SecretBytes>> ChallengeHandler for F {
    fn resolve(&mut self, url: &Url) -> Result<SecretBytes> {
        self(url)
    }
}

#[derive(Debug)]
struct AuroraMessage {
    header: Value,
    body: Value,
}

impl AuroraMessage {
    fn integer(&self, name: &str) -> Option<u64> {
        integer(self.header.get(name))
    }

    fn is_response(&self) -> bool {
        boolean(self.header.get("is_response"))
    }

    fn status(&self) -> u64 {
        self.integer("status").unwrap_or(0)
    }
}

/// Aurora sends these as either JSON numbers or strings.
fn integer(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn boolean(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(text)) => text.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

pub struct AuroraClient {
    socket: RpcSocket,
    next_token: u64,
}

impl AuroraClient {
    pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self> {
        let socket = RpcSocket::connect(
            host,
            port,
            timeout,
            SocketProfile {
                path: AURORA_PATH,
                subprotocol: Some(AURORA_SUBPROTOCOL),
                lenient_upgrade: false,
            },
        )?;
        Ok(Self {
            socket,
            next_token: 1,
        })
    }

    pub fn connect_default(timeout: Duration) -> Result<Self> {
        Self::connect(AURORA_HOST, AURORA_PORT, timeout)
    }

    fn send(&mut self, service_hash: u32, method_id: u32, body: &Value) -> Result<u64> {
        let token = self.next_token;
        self.next_token += 1;
        let message = json!([
            {
                "method_id": method_id,
                "service_hash": service_hash,
                "service_id": 0,
                "token": token,
            },
            body,
        ]);
        self.socket.send_text(&message.to_string())?;
        Ok(token)
    }

    fn receive(&mut self) -> Result<AuroraMessage> {
        let text = self.socket.receive_text()?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| aurora_error("Aurora frame is not valid JSON"))?;
        let Value::Array(pair) = value else {
            return Err(aurora_error("Aurora message is not a [header, body] pair"));
        };
        let [header, body] = <[Value; 2]>::try_from(pair)
            .map_err(|_| aurora_error("Aurora message is not a [header, body] pair"))?;
        if !header.is_object() || !body.is_object() {
            return Err(aurora_error("Aurora message is not a [header, body] pair"));
        }
        Ok(AuroraMessage { header, body })
    }

    fn await_response(&mut self, token: u64) -> Result<AuroraMessage> {
        loop {
            let message = self.receive()?;
            if message.integer("token") != Some(token) || !message.is_response() {
                continue;
            }
            let status = message.status();
            if status != 0 {
                return Err(aurora_error(format!(
                    "Aurora request token {token} failed with status {status}"
                )));
            }
            return Ok(message);
        }
    }

    /// signs in and asks where the classic server is.
    ///
    /// `credential` is a cached web credential, or
    /// [`CHALLENGE_BOOTSTRAP_CREDENTIAL`] to ask for a fresh challenge.
    pub fn bootstrap(
        &mut self,
        credential: &SecretBytes,
        challenge: &mut impl ChallengeHandler,
        mut validate_account: impl FnMut(u64, Option<&str>) -> Result<()>,
    ) -> Result<(ClassicHandoff, AuroraSession)> {
        let token = self.send(
            ids::CONNECTION,
            ids::CONNECT,
            &json!({"use_bindless_rpc": true}),
        )?;
        self.await_response(token)?;

        let session = self.logon(credential, challenge)?;
        // bind the account before asking for a product handoff. A valid cached
        // token can belong to a different Battle.net account; letting it reach
        // the classic edge first makes entitlement failure indistinguishable
        // from a bad handoff.
        validate_account(session.account_low, session.battle_tag.as_deref())?;
        let token = self.send(
            ids::GAME_UTILITIES,
            ids::PROCESS_CLIENT_REQUEST,
            &process_client_request(),
        )?;
        let response = self.await_response(token)?;
        Ok((parse_endpoint(&response.body)?, session))
    }

    /// Aurora interleaves the logon response, the challenge exchange, and
    /// `OnLogonComplete`, so all three are tracked at once.
    fn logon(
        &mut self,
        credential: &SecretBytes,
        challenge: &mut impl ChallengeHandler,
    ) -> Result<AuroraSession> {
        let logon_token = self.send(ids::AUTHENTICATION, ids::LOGON, &logon_request(credential))?;
        let mut logon_answered = false;
        let mut session = None;
        let mut verification_token = None;
        let mut verification_answered = false;

        while session.is_none()
            || !logon_answered
            || (verification_token.is_some() && !verification_answered)
        {
            let message = self.receive()?;
            let token = message.integer("token");
            let service_hash = message.integer("service_hash");
            let method_id = message.integer("method_id");

            if message.is_response() {
                if token == Some(logon_token) {
                    check_status(&message, "Aurora Logon")?;
                    logon_answered = true;
                } else if verification_token.is_some() && token == verification_token {
                    // guarded on is_some: a response carrying no token would
                    // match an unissued verification
                    check_status(&message, "Aurora web credential verification")?;
                    verification_answered = true;
                }
                continue;
            }

            if service_hash == Some(u64::from(ids::AUTHENTICATION_LISTENER))
                && method_id == Some(u64::from(ids::ON_LOGON_COMPLETE))
            {
                session = Some(parse_logon_result(&message.body)?);
            } else if service_hash == Some(u64::from(ids::CHALLENGE_LISTENER)) {
                match method_id.map(|id| u32::try_from(id).unwrap_or(0)) {
                    Some(ids::ON_EXTERNAL_CHALLENGE) => {
                        if verification_token.is_some() {
                            return Err(aurora_error(
                                "Aurora issued more than one external challenge",
                            ));
                        }
                        let credential = Self::resolve_challenge(&message.body, challenge)?;
                        verification_token = Some(self.send(
                            ids::AUTHENTICATION,
                            ids::VERIFY_WEB_CREDENTIALS,
                            &json!({"web_credentials": BASE64.encode(credential.expose())}),
                        )?);
                    }
                    Some(ids::ON_EXTERNAL_CHALLENGE_RESULT)
                        if !boolean(message.body.get("passed")) =>
                    {
                        return Err(aurora_error("Aurora external challenge failed"));
                    }
                    _ => {}
                }
            }
        }
        session.ok_or_else(|| aurora_error("Aurora never delivered a logon result"))
    }

    fn resolve_challenge(
        body: &Value,
        challenge: &mut impl ChallengeHandler,
    ) -> Result<SecretBytes> {
        let payload_type = body.get("payload_type").and_then(Value::as_str);
        let payload = body.get("payload").and_then(Value::as_str);
        let (Some(payload_type), Some(payload)) = (payload_type, payload) else {
            return Err(aurora_error("Aurora external challenge is malformed"));
        };
        if payload_type != "web_auth_url" {
            return Err(aurora_error(format!(
                "unsupported Aurora challenge type {payload_type:?}"
            )));
        }
        challenge.resolve(&decode_web_auth_url(payload)?)
    }

    pub fn close(&mut self) -> Result<()> {
        self.socket.close()
    }
}

fn check_status(message: &AuroraMessage, what: &str) -> Result<()> {
    match message.status() {
        0 => Ok(()),
        status => Err(aurora_error(format!("{what} failed with status {status}"))),
    }
}

fn logon_request(credential: &SecretBytes) -> Value {
    let profile = Product::Remastered
        .logon()
        .expect("Remastered has a traced logon profile");
    json!({
        "allow_logon_queue_notifications": true,
        "application_version": profile.application_version,
        "cached_web_credentials": BASE64.encode(credential.expose()),
        "locale": profile.locale,
        // the account layer spells the platform out; the classic channel sends
        // the `Mc64` FourCC for the same machine
        "platform": "Mac",
        "program": Product::Remastered.code(),
        "version": profile.application_version.to_string(),
        "web_client_verification": true,
    })
}

fn process_client_request() -> Value {
    let request = Message::new()
        .varint(2, u64::from(Product::Remastered.fourcc()))
        .bytes(3, GAME_VERSION.as_bytes())
        .varint(4, u64::from(platform_fourcc()))
        .into_vec();
    json!({
        "attribute": [
            {
                "name": "client_request",
                "value": {"string_value": "classic.protocol.v1.aurora.ConnectToServerRequest"},
            },
            {"name": "protobuf", "value": {"string_value": BASE64.encode(request)}},
            {"name": "server_instance", "value": {"string_value": "Release"}},
        ]
    })
}

fn parse_logon_result(body: &Value) -> Result<AuroraSession> {
    if let Some(code) = integer(body.get("error_code")).filter(|code| *code != 0) {
        return Err(aurora_error(format!(
            "Aurora logon failed with error {code}"
        )));
    }
    let session_key = body
        .get("session_key")
        .and_then(Value::as_str)
        .ok_or_else(|| aurora_error("Aurora logon result has no session key"))?;
    let account = body
        .get("account_id")
        .ok_or_else(|| aurora_error("Aurora logon result has no account ID"))?;
    let game_account = body
        .get("game_account_id")
        .and_then(|value| value.get(0))
        .ok_or_else(|| aurora_error("Aurora logon result has no game account"))?;

    let part = |value: &Value, name: &str| -> Result<u64> {
        integer(value.get(name))
            .ok_or_else(|| aurora_error(format!("Aurora account ID has no {name}")))
    };
    Ok(AuroraSession {
        session_key: parse_session_key(session_key)?,
        account_high: part(account, "high")?,
        account_low: part(account, "low")?,
        game_account_high: part(game_account, "high")?,
        game_account_low: part(game_account, "low")?,
        connected_region: integer(body.get("connected_region"))
            .ok_or_else(|| aurora_error("Aurora logon result has no connected region"))?,
        // optional on the wire and optional here: a logon without it still
        // signs in, it just cannot say who you are until a roster does
        battle_tag: body
            .get("battle_tag")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn parse_session_key(encoded: &str) -> Result<SecretBytes> {
    let key = BASE64
        .decode(encoded)
        .map_err(|_| aurora_error("Aurora session key is not valid base64"))?;
    if key.len() != SESSION_KEY_BYTES {
        return Err(aurora_error(format!(
            "Aurora session key is {} bytes; expected {SESSION_KEY_BYTES}",
            key.len()
        )));
    }
    SecretBytes::new(key)
}

fn parse_endpoint(body: &Value) -> Result<ClassicHandoff> {
    let attributes = body
        .get("attribute")
        .and_then(Value::as_array)
        .ok_or_else(|| aurora_error("ConnectToServer response has no attributes"))?;

    let attribute = |wanted: &str| -> Option<&str> {
        attributes.iter().find_map(|attribute| {
            (attribute.get("name")?.as_str()? == wanted).then_some(())?;
            let value = attribute.get("value")?;
            value
                .get("blob_value")
                .or_else(|| value.get("string_value"))?
                .as_str()
        })
    };

    let response_type = attribute("client_response");
    if response_type != Some("classic.protocol.v1.aurora.ConnectToServerResponse") {
        return Err(aurora_error(format!(
            "unexpected classic response type {response_type:?}"
        )));
    }
    let payload = attribute("protobuf")
        .ok_or_else(|| aurora_error("ConnectToServer response has no protobuf payload"))?;
    let raw = BASE64
        .decode(payload)
        .map_err(|_| aurora_error("ConnectToServer payload is not valid base64"))?;
    let url = protobuf::first_bytes(&raw, 1)
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .ok_or_else(|| aurora_error("ConnectToServer response has no endpoint URL"))?;
    let ticket = protobuf::first_bytes(&raw, 2)
        .ok_or_else(|| aurora_error("ConnectToServer response has no ticket"))?;
    let mut handoff = ClassicHandoff::from_url(&url, SecretBytes::new(ticket.to_vec())?)?;
    handoff.shape = describe(&raw);
    Ok(handoff)
}

/// every field in the payload, as number, wire type, and size.
///
/// Only fields 1 and 2 are read. Anything else the response carries is dropped
/// silently, which is exactly the kind of thing that hides in a payload for
/// years, so the shape is kept and traced rather than guessed at.
fn describe(raw: &[u8]) -> String {
    protobuf::fields(raw)
        .map(|field| match field {
            Err(_) => "<malformed>".to_owned(),
            Ok(field) => {
                let value = match field.value {
                    protobuf::Value::Varint(value) => format!("varint {value}"),
                    protobuf::Value::Fixed64(value) => format!("fixed64 {value}"),
                    protobuf::Value::Fixed32(value) => format!("fixed32 {value}"),
                    protobuf::Value::Bytes(bytes) => match std::str::from_utf8(bytes) {
                        Ok(text) if text.chars().all(|c| !c.is_control()) => {
                            format!("text {text:?}")
                        }
                        _ => format!("{} bytes", bytes.len()),
                    },
                };
                format!("{}={value}", field.number)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// the challenge carries the sign-in URL as base64 text. Only Battle.net's own
/// hosts are accepted: this URL is followed with a browser.
pub fn decode_web_auth_url(payload: &str) -> Result<Url> {
    let decoded = BASE64
        .decode(payload)
        .map_err(|_| aurora_error("Aurora web auth URL is not valid base64"))?;
    let text = String::from_utf8(decoded)
        .map_err(|_| aurora_error("Aurora web auth URL is not valid UTF-8"))?;
    let url = Url::parse(&text)
        .map_err(|_| aurora_error(format!("invalid Aurora web auth URL {text:?}")))?;
    match url.host_str() {
        Some(host) if host == "battle.net" || host.ends_with(".battle.net") => Ok(url),
        _ => Err(aurora_error(format!(
            "Aurora web auth URL is not a Battle.net host: {text:?}"
        ))),
    }
}

fn aurora_error(message: impl Into<String>) -> Error {
    Error::Authentication(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_layer_is_not_the_front_channel() {
        // the whole reason this module exists: same host, same port, different
        // protocol. signing in on the protobuf channel and asking it for a
        // classic server answers with an endpoint whose route is not there.
        assert_eq!(AURORA_SUBPROTOCOL, "jsonrpc.aurora.v1.30.battle.net");
        assert_ne!(
            AURORA_SUBPROTOCOL,
            crate::platform::wire::websocket::SocketProfile::BGS
                .subprotocol
                .expect("the Front channel names one")
        );
    }

    #[test]
    fn the_bootstrap_marker_carries_no_secret() {
        // it is sent in place of a credential to ask for a fresh challenge; if
        // it ever stopped being all-zeroes it would be one
        let text = std::str::from_utf8(CHALLENGE_BOOTSTRAP_CREDENTIAL).expect("ascii");
        assert!(text.starts_with("US-"));
        assert!(
            text.trim_start_matches("US-")
                .chars()
                .all(|character| character == '0' || character == '-')
        );
    }

    #[test]
    fn the_logon_names_remastered_and_spells_out_its_platform() {
        let credential = SecretBytes::new(b"credential".to_vec()).expect("credential");
        let request = logon_request(&credential);
        assert_eq!(request["program"], "S1");
        // the account layer spells it; the classic channel sends the FourCC
        assert_eq!(request["platform"], "Mac");
        assert_eq!(request["locale"], "enUS");
        assert_eq!(request["application_version"], 65559);
        assert_eq!(
            request["cached_web_credentials"],
            BASE64.encode(b"credential")
        );
    }

    #[test]
    fn only_battle_nets_own_hosts_are_followed() {
        // this URL is opened in a browser, so an attacker-chosen host would be
        // a redirect to somewhere the user's session gets handed over
        let ours = BASE64.encode("https://us.battle.net/login/en/?ref=x");
        assert!(decode_web_auth_url(&ours).is_ok());

        for elsewhere in [
            "https://battle.net.evil.example/login",
            "https://notbattle.net/login",
            "http://example.invalid/",
        ] {
            let encoded = BASE64.encode(elsewhere);
            assert!(
                decode_web_auth_url(&encoded).is_err(),
                "followed {elsewhere}"
            );
        }
    }

    #[test]
    fn integers_arrive_as_numbers_or_as_their_text() {
        // aurora sends these both ways, in the same field, across builds
        assert_eq!(integer(Some(&json!(7))), Some(7));
        assert_eq!(integer(Some(&json!("7"))), Some(7));
        assert_eq!(integer(Some(&json!("not a number"))), None);
        assert!(boolean(Some(&json!(true))));
        assert!(boolean(Some(&json!("TRUE"))));
        assert!(!boolean(Some(&json!("no"))));
    }

    #[test]
    fn a_session_key_must_be_sixty_four_bytes() {
        assert!(parse_session_key(&BASE64.encode([0u8; 64])).is_ok());
        assert!(parse_session_key(&BASE64.encode([0u8; 32])).is_err());
        assert!(parse_session_key("not base64!").is_err());
    }

    #[test]
    fn a_logon_result_is_read_whichever_way_its_numbers_are_spelled() {
        let body = json!({
            "session_key": BASE64.encode([0x5A; 64]),
            "account_id": {"high": 1, "low": "2"},
            "game_account_id": [{"high": "3", "low": 4}],
            "connected_region": 5,
        });
        let session = parse_logon_result(&body).expect("logon result");
        assert_eq!(session.account_high, 1);
        assert_eq!(session.account_low, 2);
        assert_eq!(session.game_account_high, 3);
        assert_eq!(session.game_account_low, 4);
        assert_eq!(session.connected_region, 5);

        // an error code is a failure even when the rest looks complete
        let mut failed = body.clone();
        failed["error_code"] = json!(3);
        assert!(parse_logon_result(&failed).is_err());
    }

    #[test]
    fn the_whole_payload_shape_is_kept_even_though_two_fields_are_read() {
        // a field we do not read is exactly what would hide a route, a port, or
        // a token for years; the trace says what was there
        let raw = Message::new()
            .bytes(1, b"wss://host.invalid/")
            .bytes(2, b"ticket")
            .varint(3, 7)
            .bytes(4, &[0u8; 16])
            .into_vec();
        let shape = describe(&raw);
        assert!(shape.contains("1=text"), "{shape}");
        assert!(shape.contains("2=text \"ticket\""), "{shape}");
        assert!(shape.contains("3=varint 7"), "{shape}");
        assert!(shape.contains("4=16 bytes"), "{shape}");
    }

    #[test]
    fn the_endpoint_is_read_from_a_blob_or_a_string() {
        let inner = Message::new()
            .bytes(1, b"wss://us1-s1-rclient-ext-lb.classic.blizzard.com/")
            .bytes(2, b"ticket")
            .into_vec();
        let body = json!({
            "attribute": [
                {
                    "name": "client_response",
                    "value": {"string_value": "classic.protocol.v1.aurora.ConnectToServerResponse"},
                },
                {"name": "protobuf", "value": {"blob_value": BASE64.encode(&inner)}},
            ]
        });
        let handoff = parse_endpoint(&body).expect("endpoint");
        assert_eq!(handoff.host, "us1-s1-rclient-ext-lb.classic.blizzard.com");
        assert_eq!(handoff.path, crate::games::scr::CLASSIC_RPC_PATH);
        assert_eq!(handoff.ticket.expose(), b"ticket");
    }
}
