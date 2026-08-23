//! descriptor-shaped JSON messages used by WC3's BGS v2 services.

use std::fmt;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use zeroize::{Zeroize as _, Zeroizing};

use crate::{Result, platform::bgs::SecretBytes};

#[derive(Clone, PartialEq, Eq)]
pub struct Base64Bytes(SecretBytes);

impl Base64Bytes {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        SecretBytes::new(bytes).map(Self)
    }

    #[must_use]
    pub fn from_secret(secret: SecretBytes) -> Self {
        Self(secret)
    }

    #[must_use]
    pub fn expose(&self) -> &[u8] {
        self.0.expose()
    }

    #[must_use]
    pub fn into_secret(self) -> SecretBytes {
        self.0
    }
}

impl fmt::Debug for Base64Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Base64Bytes")
            .field("length", &self.0.len())
            .finish_non_exhaustive()
    }
}

impl Serialize for Base64Bytes {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut encoded = Zeroizing::new(String::new());
        BASE64.encode_string(self.expose(), &mut encoded);
        serializer.serialize_str(&encoded)
    }
}

impl<'de> Deserialize<'de> for Base64Bytes {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut encoded = String::deserialize(deserializer)?;
        let decoded = BASE64
            .decode(encoded.as_bytes())
            .map_err(|_| D::Error::custom("bytes field is not valid base64"));
        encoded.zeroize();
        Self::new(decoded?).map_err(|_| D::Error::custom("bytes field is empty"))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MeteringLevel {
    #[default]
    MeteringLevelLegacy,
    MeteringLevelCategorized,
}

#[derive(Default, Serialize)]
pub struct ConnectRequest {
    pub use_bindless_rpc: Option<bool>,
    pub metering_level: Option<MeteringLevel>,
}

#[derive(Default, Deserialize)]
pub struct ConnectResponse {
    pub ciid: Option<String>,
    pub connected_region: Option<u32>,
}

#[derive(Default, Serialize)]
pub struct LogonOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<Base64Bytes>,
}

#[derive(Default, Serialize)]
pub struct LogonRequest {
    pub title_id: Option<u32>,
    pub platform: Option<String>,
    pub locale: Option<String>,
    pub application_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logon_options: Option<LogonOptions>,
}

#[derive(Default, Serialize)]
pub struct VerifyAuthTokenRequest {
    pub auth_token: Option<Base64Bytes>,
}

#[derive(Default, Serialize)]
pub struct GenerateAuthTokenRequest {
    pub title_id: Option<u32>,
}

#[derive(Default, Deserialize)]
pub struct GenerateAuthTokenResponse {
    pub auth_token: Option<Base64Bytes>,
}

#[derive(Clone, Default, Deserialize)]
pub struct GameAccountHandle {
    #[serde(default, deserialize_with = "optional_u64")]
    pub id: Option<u64>,
    pub title_id: Option<u32>,
    pub region: Option<u32>,
}

#[derive(Default, Deserialize)]
pub struct LogonRecord {
    #[serde(default, deserialize_with = "optional_u64")]
    pub account_id: Option<u64>,
    #[serde(default)]
    pub game_account: Vec<GameAccountHandle>,
    pub battle_tag: Option<String>,
    pub session_key: Option<Base64Bytes>,
    #[serde(default, deserialize_with = "optional_bool")]
    pub employee_only_mode: Option<bool>,
}

#[derive(Default, Deserialize)]
pub struct LogonCompleteNotification {
    pub error_code: Option<u32>,
    pub record: Option<LogonRecord>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LogonQueueState {
    pub position: Option<u32>,
}

#[derive(Default, Deserialize)]
pub struct LogonQueueUpdateNotification {
    pub state: Option<LogonQueueState>,
}

#[derive(Default, Deserialize)]
pub struct ExternalChallengeNotification {
    pub payload_type: Option<String>,
    pub payload: Option<Base64Bytes>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Variant {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_value: Option<Base64Bytes>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Attribute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Variant>,
}

#[derive(Default, Serialize)]
pub struct ProcessTaskRequest {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attribute: Vec<Attribute>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<Attribute>,
}

#[derive(Default, Deserialize)]
pub struct ProcessTaskResponse {
    #[serde(default)]
    pub result: Vec<Attribute>,
}

fn optional_u64<'de, D>(deserializer: D) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_u64()
            .map(Some)
            .ok_or_else(|| D::Error::custom("uint64 is outside range")),
        Some(serde_json::Value::String(text)) => text
            .parse()
            .map(Some)
            .map_err(|_| D::Error::custom("uint64 string is invalid")),
        Some(_) => Err(D::Error::custom("uint64 has an invalid JSON type")),
    }
}

fn optional_bool<'de, D>(deserializer: D) -> std::result::Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Bool(value)) => Ok(Some(value)),
        Some(serde_json::Value::String(text)) if text.eq_ignore_ascii_case("true") => {
            Ok(Some(true))
        }
        Some(serde_json::Value::String(text)) if text.eq_ignore_ascii_case("false") => {
            Ok(Some(false))
        }
        Some(_) => Err(D::Error::custom("bool has an invalid JSON value")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protobuf_json_bytes_round_trip_without_debug_disclosure() {
        let value = Base64Bytes::new(b"private-token".to_vec()).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: Base64Bytes = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.expose(), b"private-token");
        assert!(!format!("{decoded:?}").contains("private-token"));
    }
}
