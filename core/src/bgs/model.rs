use std::{fmt, net::IpAddr};

use prost::Message;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    Error, Result,
    bgs::generated::bgs::protocol::{
        Attribute, EntityId, Variant,
        authentication::v1::{LogonRequest, LogonResult},
        challenge::v1::ChallengeExternalRequest,
        game_utilities::v1::{ClientRequest, ClientResponse},
    },
};

pub const SC2_BGS_SDK_VERSION: &str =
    "Battle.net Game Service SDK v1.48.2 \"cf68e241e0\"/104 (Jul 14 2026 19:45:54)";

#[derive(Clone, Eq, PartialEq)]
pub struct SecretBytes(Zeroizing<Vec<u8>>);

impl SecretBytes {
    pub fn new(value: Vec<u8>) -> Result<Self> {
        if value.is_empty() {
            return Err(model_error("secret byte string cannot be empty"));
        }
        Ok(Self(Zeroizing::new(value)))
    }

    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretBytes")
            .field("length", &self.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct LogonSession {
    pub account_id: Option<EntityId>,
    pub game_account_ids: Vec<EntityId>,
    pub available_regions: Vec<u32>,
    pub connected_region: Option<u32>,
    pub restricted_mode: bool,
    pub session_key: SecretBytes,
}

impl fmt::Debug for LogonSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LogonSession")
            .field("game_accounts", &self.game_account_ids.len())
            .field("available_regions", &self.available_regions)
            .field("connected_region", &self.connected_region)
            .field("restricted_mode", &self.restricted_mode)
            .field("session_key", &self.session_key)
            .finish_non_exhaustive()
    }
}

impl TryFrom<LogonResult> for LogonSession {
    type Error = Error;

    fn try_from(result: LogonResult) -> Result<Self> {
        if result.error_code != 0 {
            return Err(Error::Server(format!(
                "Battle.net authentication failed: {}",
                result.error_code
            )));
        }
        let session_key = result
            .session_key
            .ok_or_else(|| model_error("LogonResult has no session key"))?;
        Ok(Self {
            account_id: result.account_id,
            game_account_ids: result.game_account_id,
            available_regions: result.available_region,
            connected_region: result.connected_region,
            restricted_mode: result.restricted_mode.unwrap_or(false),
            session_key: SecretBytes::new(session_key)?,
        })
    }
}

#[derive(Clone)]
pub struct NativeHandoff {
    address: String,
    pub session_key: SecretBytes,
    pub account_region: u8,
    game_account_name: String,
    account_mail: String,
    logon_response: SecretBytes,
}

impl fmt::Debug for NativeHandoff {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeHandoff")
            .field("session_key", &self.session_key)
            .field("account_region", &self.account_region)
            .field("logon_response", &self.logon_response)
            .finish_non_exhaustive()
    }
}

impl NativeHandoff {
    pub fn decode(body: &[u8]) -> Result<Self> {
        let response = ClientResponse::decode(body)?;
        let mut address = None;
        let mut session_key = None;
        let mut account_region = None;
        let mut game_account_name = None;
        let mut account_mail = None;
        let mut logon_response = None;

        for attribute in response.attribute {
            let destination = match attribute.name.as_str() {
                "address" => &mut address,
                "session_key" => &mut session_key,
                "account_region" => &mut account_region,
                "game_account_name" => &mut game_account_name,
                "account_mail" => &mut account_mail,
                "logon_response" => &mut logon_response,
                _ => continue,
            };
            if destination.is_some() {
                return Err(model_error(format!(
                    "GameUtilities returned duplicate {:?}",
                    attribute.name
                )));
            }
            *destination = Some(VariantValue::decode(attribute.value)?);
        }

        let address = expect_string(address, "address")?;
        let session_key = expect_bytes(session_key, "session_key")?;
        let region = expect_unsigned(account_region, "account_region")?;
        let account_region = u8::try_from(region)
            .map_err(|_| model_error("SC2 account region is outside byte range"))?;
        let game_account_name = expect_string(game_account_name, "game_account_name")?;
        let account_mail = expect_string(account_mail, "account_mail")?;
        let logon_response = expect_bytes(logon_response, "logon_response")?;

        if address.is_empty() || address.len() > 255 {
            return Err(model_error("SC2 native endpoint has an invalid length"));
        }
        if session_key.len() != 64 {
            return Err(model_error("SC2 native session key is not 64 bytes"));
        }
        if game_account_name.is_empty() || game_account_name.len() > 32 {
            return Err(model_error("SC2 game-account name has an invalid length"));
        }
        if account_mail.is_empty() || account_mail.len() > 320 {
            return Err(model_error("SC2 account mail has an invalid length"));
        }

        Ok(Self {
            address,
            session_key: SecretBytes::new(session_key)?,
            account_region,
            game_account_name,
            account_mail,
            logon_response: SecretBytes::new(logon_response)?,
        })
    }

    /// override the native endpoint (`host:port`). used to point a native client
    /// at a loopback Sunken server instead of Blizzard's real native address.
    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }

    pub fn endpoint(&self, default_port: u16) -> Result<(String, u16)> {
        let value = self.address.trim();
        let (host, port) = if let Some(value) = value.strip_prefix('[') {
            let (host, suffix) = value
                .split_once(']')
                .ok_or_else(|| model_error("SC2 native endpoint has invalid IPv6 syntax"))?;
            let port =
                if suffix.is_empty() {
                    default_port
                } else {
                    parse_port(suffix.strip_prefix(':').ok_or_else(|| {
                        model_error("SC2 native endpoint has invalid IPv6 syntax")
                    })?)?
                };
            (host.to_owned(), port)
        } else if value.matches(':').count() == 1 {
            let (host, port) = value
                .rsplit_once(':')
                .ok_or_else(|| model_error("SC2 native endpoint is malformed"))?;
            (host.to_owned(), parse_port(port)?)
        } else {
            (value.to_owned(), default_port)
        };
        if host.is_empty() || host.chars().any(char::is_whitespace) {
            return Err(model_error("SC2 native endpoint has an invalid host"));
        }
        if host.contains(':') {
            host.parse::<IpAddr>()
                .map_err(|_| model_error("SC2 native endpoint has invalid IPv6 syntax"))?;
        }
        Ok((host, port))
    }

    #[must_use]
    pub fn account_mail(&self) -> &str {
        &self.account_mail
    }

    #[must_use]
    pub fn game_account_name(&self) -> &str {
        &self.game_account_name
    }

    #[must_use]
    pub fn logon_response(&self) -> &[u8] {
        self.logon_response.expose()
    }
}

pub fn default_logon_request(cached_web_credentials: Option<&[u8]>) -> LogonRequest {
    LogonRequest {
        program: Some("S2".into()),
        platform: Some("Mc64".into()),
        locale: Some("enUS".into()),
        version: Some(SC2_BGS_SDK_VERSION.into()),
        application_version: Some(0),
        allow_logon_queue_notifications: Some(true),
        cached_web_credentials: cached_web_credentials.map(<[u8]>::to_vec),
        ..LogonRequest::default()
    }
}

pub fn build_front_request(session: &LogonSession) -> Result<ClientRequest> {
    let game_account_id = session
        .game_account_ids
        .first()
        .copied()
        .ok_or_else(|| model_error("LogonResult has no selected SC2 game account"))?;
    Ok(ClientRequest {
        attribute: vec![
            attribute_string("LogonTokenRequest", "0.0.1"),
            attribute_string("environment", "US"),
            attribute_blob("session_key", session.session_key.expose()),
            attribute_string("locale", "enUS"),
        ],
        game_account_id: Some(game_account_id),
        ..ClientRequest::default()
    })
}

pub fn challenge_url(challenge: ChallengeExternalRequest) -> Result<Url> {
    if challenge.payload_type.as_deref() != Some("web_auth_url") {
        return Err(Error::Authentication(format!(
            "unsupported external challenge {:?}",
            challenge.payload_type
        )));
    }
    let payload = challenge
        .payload
        .ok_or_else(|| model_error("external challenge has no payload"))?;
    let text = std::str::from_utf8(&payload)
        .map_err(|_| model_error("web authentication URL is not UTF-8"))?;
    let url = Url::parse(text)?;
    let allowed_host = url
        .host_str()
        .is_some_and(|host| host.ends_with(".account.battle.net"));
    if url.scheme() != "https" || !allowed_host {
        return Err(Error::Authentication(
            "Battle.net returned an unexpected authentication URL".into(),
        ));
    }
    Ok(url)
}

#[must_use]
pub fn fourcc(value: &str) -> u32 {
    assert!(
        (1..=4).contains(&value.len()),
        "FourCC must be one to four bytes"
    );
    value
        .as_bytes()
        .iter()
        .fold(0_u32, |output, byte| (output << 8) | u32::from(*byte))
}

enum VariantValue {
    Bool(bool),
    Signed(i64),
    Float(f64),
    String(String),
    Blob(Vec<u8>),
    Message(Vec<u8>),
    FourCc(String),
    Unsigned(u64),
    EntityId(EntityId),
}

impl VariantValue {
    fn decode(value: Variant) -> Result<Self> {
        let variants = [
            value.bool_value.is_some(),
            value.int_value.is_some(),
            value.float_value.is_some(),
            value.string_value.is_some(),
            value.blob_value.is_some(),
            value.message_value.is_some(),
            value.fourcc_value.is_some(),
            value.uint_value.is_some(),
            value.entity_id_value.is_some(),
        ];
        if variants.into_iter().filter(|present| *present).count() != 1 {
            return Err(model_error(
                "GameUtilities Variant must contain exactly one typed value",
            ));
        }
        if let Some(value) = value.bool_value {
            Ok(Self::Bool(value))
        } else if let Some(value) = value.int_value {
            Ok(Self::Signed(value))
        } else if let Some(value) = value.float_value {
            Ok(Self::Float(value))
        } else if let Some(value) = value.string_value {
            Ok(Self::String(value))
        } else if let Some(value) = value.blob_value {
            Ok(Self::Blob(value))
        } else if let Some(value) = value.message_value {
            Ok(Self::Message(value))
        } else if let Some(value) = value.fourcc_value {
            Ok(Self::FourCc(value))
        } else if let Some(value) = value.uint_value {
            Ok(Self::Unsigned(value))
        } else if let Some(value) = value.entity_id_value {
            Ok(Self::EntityId(value))
        } else {
            unreachable!("exactly one variant was checked")
        }
    }
}

impl fmt::Debug for VariantValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => formatter.debug_tuple("Bool").field(value).finish(),
            Self::Signed(value) => formatter.debug_tuple("Signed").field(value).finish(),
            Self::Float(value) => formatter.debug_tuple("Float").field(value).finish(),
            Self::String(_) => formatter.write_str("String(<suppressed>)"),
            Self::Blob(value) => formatter
                .debug_struct("Blob")
                .field("length", &value.len())
                .finish(),
            Self::Message(value) => formatter
                .debug_struct("Message")
                .field("length", &value.len())
                .finish(),
            Self::FourCc(value) => formatter.debug_tuple("FourCc").field(value).finish(),
            Self::Unsigned(value) => formatter.debug_tuple("Unsigned").field(value).finish(),
            Self::EntityId(value) => {
                let _ = value;
                formatter.write_str("EntityId(<suppressed>)")
            }
        }
    }
}

fn attribute_string(name: &str, value: &str) -> Attribute {
    Attribute {
        name: name.into(),
        value: Variant {
            string_value: Some(value.into()),
            ..Variant::default()
        },
    }
}

fn attribute_blob(name: &str, value: &[u8]) -> Attribute {
    Attribute {
        name: name.into(),
        value: Variant {
            blob_value: Some(value.to_vec()),
            ..Variant::default()
        },
    }
}

fn expect_string(value: Option<VariantValue>, name: &str) -> Result<String> {
    match value {
        Some(VariantValue::String(value)) => Ok(value),
        Some(value) => Err(model_error(format!(
            "GameUtilities attribute {name:?} is {value:?}, expected string"
        ))),
        None => Err(model_error(format!(
            "GameUtilities response has no {name:?} attribute"
        ))),
    }
}

fn expect_bytes(value: Option<VariantValue>, name: &str) -> Result<Vec<u8>> {
    match value {
        Some(VariantValue::Blob(value)) => Ok(value),
        Some(value) => Err(model_error(format!(
            "GameUtilities attribute {name:?} is {value:?}, expected blob"
        ))),
        None => Err(model_error(format!(
            "GameUtilities response has no {name:?} attribute"
        ))),
    }
}

fn expect_unsigned(value: Option<VariantValue>, name: &str) -> Result<u64> {
    match value {
        Some(VariantValue::Unsigned(value)) => Ok(value),
        Some(value) => Err(model_error(format!(
            "GameUtilities attribute {name:?} is {value:?}, expected uint"
        ))),
        None => Err(model_error(format!(
            "GameUtilities response has no {name:?} attribute"
        ))),
    }
}

fn parse_port(value: &str) -> Result<u16> {
    value
        .parse()
        .map_err(|_| model_error("SC2 native endpoint port is not numeric"))
}

fn model_error(message: impl Into<String>) -> Error {
    Error::BgsWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_request_matches_recovered_sparse_shape() {
        let session = LogonSession {
            account_id: Some(EntityId { high: 1, low: 2 }),
            game_account_ids: vec![EntityId { high: 3, low: 4 }],
            available_regions: vec![1],
            connected_region: Some(1),
            restricted_mode: false,
            session_key: SecretBytes::new(vec![0x55; 64]).unwrap(),
        };
        let request = build_front_request(&session).unwrap();
        assert_eq!(request.attribute.len(), 4);
        assert_eq!(
            request
                .attribute
                .iter()
                .map(|attribute| attribute.name.as_str())
                .collect::<Vec<_>>(),
            ["LogonTokenRequest", "environment", "session_key", "locale"]
        );
        assert!(request.host.is_none());
        assert!(request.account_id.is_none());
        assert_eq!(
            request.game_account_id,
            session.game_account_ids.first().copied()
        );
        assert!(request.program.is_none());
        assert!(request.client_info.is_none());
    }

    #[test]
    fn native_endpoint_parses_host_and_port() {
        let bootstrap = NativeHandoff {
            address: "127.0.0.1:1119".into(),
            session_key: SecretBytes::new(vec![1; 64]).unwrap(),
            account_region: 1,
            game_account_name: "toon".into(),
            account_mail: "account@example.invalid".into(),
            logon_response: SecretBytes::new(vec![1]).unwrap(),
        };
        assert_eq!(
            bootstrap.endpoint(1119).unwrap(),
            ("127.0.0.1".into(), 1119)
        );
    }
}
