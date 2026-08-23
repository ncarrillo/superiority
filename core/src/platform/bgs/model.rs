use std::{
    fmt,
    net::IpAddr,
    time::{SystemTime, UNIX_EPOCH},
};

use prost::Message;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    Error, Result,
    bgs::generated::bgs::protocol::{
        Attribute, EntityId, Variant,
        account::v1::{AccountLicense, AccountState},
        authentication::v1::{LogonRequest, LogonResult},
        challenge::v1::ChallengeExternalRequest,
        game_utilities::v1::{ClientRequest, ClientResponse},
    },
    product::Product,
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
    /// the account's `BattleTag`. This is the only name the logon carries that
    /// is meant to be read: the account service's `name` for a game account is
    /// an internal one — `S22` for a `StarCraft II` account — and no call on
    /// this service hands back a product's display name.
    pub battle_tag: Option<String>,
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
            .field("battle_tag", &self.battle_tag)
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
            battle_tag: result.battle_tag,
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

/// the logon a product's own client sends. Which product this is for is the
/// caller's to say — it used to be written in here as `"S2"`, which is why
/// there was no way to sign in as anything else.
pub fn default_logon_request(
    product: Product,
    cached_web_credentials: Option<&[u8]>,
) -> Result<LogonRequest> {
    let profile = product.logon().ok_or_else(|| {
        model_error(format!(
            "no logon profile has been traced for {}",
            product.name()
        ))
    })?;
    Ok(LogonRequest {
        program: Some(product.code().into()),
        platform: Some(profile.platform.into()),
        locale: Some(profile.locale.into()),
        version: profile.sdk_version.map(str::to_owned),
        application_version: Some(profile.application_version.cast_signed()),
        allow_logon_queue_notifications: Some(true),
        cached_web_credentials: cached_web_credentials.map(<[u8]>::to_vec),
        ..LogonRequest::default()
    })
}

/// `StarCraft II`'s `ProcessClientRequest`. The attributes are its own — the
/// game service answers per product, and Remastered's request asks for a
/// classic endpoint with a different set — so this gets a sibling rather than a
/// parameter when that lands.
///
/// Taking the first game account is right because the logon was scoped to one
/// program: every id the result carries belongs to the program we signed in as.
/// Which region it picks is arbitrary, which is a separate question from which
/// product it is.
pub fn build_front_request(session: &LogonSession) -> Result<ClientRequest> {
    let game_account_id = session
        .game_account_ids
        .first()
        .copied()
        .ok_or_else(|| model_error("LogonResult named no game account for this product"))?;
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
/// a product record registered beneath an account, as the account service
/// names it.
///
/// `program` is the FourCC every part of this protocol keys a product by —
/// `S2` is `StarCraft II`, and it is the same value the client sends when it
/// asks for web credentials. Presence here is not an ownership assertion;
/// licenses are returned by a different AccountService RPC.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameProgram {
    pub program: u32,
    /// the service's own name for it, when it gave one.
    pub name: Option<String>,
    pub is_trial: bool,
    pub is_restricted: bool,
    /// how many game accounts sit under it, across every region.
    pub accounts: usize,
}

/// the account inputs Battle.net Desktop evaluates before it adds retail
/// products to the library.
///
/// These are deliberately kept separate. A game-account handle says that a
/// program has an account record; a license id says which edition or access
/// grant that record carries. WC3 is the important counterexample: beta
/// license `50676` creates a W3 record but does not add the retail product.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountCatalog {
    pub licenses: Vec<AccountLicense>,
    pub games: Vec<GameProgram>,
}

impl AccountCatalog {
    #[must_use]
    pub fn from_account_state(state: Option<&AccountState>) -> Self {
        Self {
            licenses: state
                .and_then(|state| state.account_level_info.as_ref())
                .map_or_else(Vec::new, |info| info.licenses.clone()),
            games: GameProgram::from_account_state(state),
        }
    }

    /// applies Battle.net Desktop catalog v30's retail-product rules for the
    /// three products Superiority supports.
    ///
    /// Recovered from the signed `starcraft_ii`, `starcraft_remastered`, and
    /// `warcraft_iii` catalog fragments shipped by Battle.net Desktop
    /// 2.52.9.17709. In particular, W3 beta licenses `50676` and `50696` are
    /// absent from the retail rule and must never reveal Reforged.
    #[must_use]
    pub fn retail_products_at(&self, now: u64, playing_from_igr: bool) -> Vec<Product> {
        let active_license = |wanted: &[u32]| {
            self.licenses.iter().any(|license| {
                wanted.contains(&license.id) && license.expires.is_none_or(|expires| expires > now)
            })
        };
        let has_game_account = |product: Product| {
            self.games
                .iter()
                .any(|game| game.program == product.fourcc() && game.accounts > 0)
        };

        // S2's final run-first rule is an unconditional retail grant tagged
        // `play_for_free`.
        let mut products = vec![Product::StarCraft2];
        // S1 retail is granted by its paid license or by an existing S1 game
        // account, in which case the launcher tags it `play_for_free`.
        if active_license(&[17_019]) || has_game_account(Product::Remastered) {
            products.push(Product::Remastered);
        }
        // W3 retail has no game-account fallback. The catalog names only the
        // retail license and two legacy retail ids, plus IGR access.
        if active_license(&[34_998, 9, 13]) || playing_from_igr {
            products.push(Product::Warcraft3);
        }
        products
    }

    /// evaluates the signed catalog at the current time. IGR access remains an
    /// explicit input because it is a separate launcher condition, not a
    /// conclusion that can be drawn from an ordinary account license.
    #[must_use]
    pub fn retail_products(&self, playing_from_igr: bool) -> Vec<Product> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.retail_products_at(now, playing_from_igr)
    }
}

impl GameProgram {
    /// reads product records out of an account state. Two fields describe them
    /// and they do not always agree: `game_level_info` carries presentation
    /// flags and names, while `game_accounts` carries login handles.
    ///
    /// In practice the live service answers with `game_accounts` only, so the
    /// names come back empty and the trial flags stay false. That costs us
    /// nothing — a card knows its own name — but it is why neither is relied
    /// on for anything.
    #[must_use]
    pub fn from_account_state(state: Option<&AccountState>) -> Vec<Self> {
        let Some(state) = state else {
            return Vec::new();
        };
        let mut games: Vec<Self> = Vec::new();
        for level in &state.game_level_info {
            let Some(program) = level.program else {
                continue;
            };
            games.push(Self {
                program,
                name: level.name.clone(),
                is_trial: level.is_trial.unwrap_or(false),
                is_restricted: level.is_restricted.unwrap_or(false),
                accounts: 0,
            });
        }
        for list in &state.game_accounts {
            for handle in &list.handle {
                if let Some(game) = games.iter_mut().find(|game| game.program == handle.program) {
                    game.accounts += 1;
                } else {
                    games.push(Self {
                        program: handle.program,
                        name: None,
                        is_trial: false,
                        is_restricted: false,
                        accounts: 1,
                    });
                }
            }
        }
        games
    }

    /// the FourCC as it reads on the wire, for tracing and for matching
    /// against a product code.
    #[must_use]
    pub fn code(&self) -> String {
        self.program
            .to_be_bytes()
            .iter()
            .filter(|byte| **byte != 0)
            .map(|byte| char::from(*byte))
            .collect()
    }
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
            battle_tag: None,
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

#[cfg(test)]
mod game_program_tests {
    use super::{AccountCatalog, GameProgram};
    use crate::bgs::generated::bgs::protocol::account::v1::{
        AccountLevelInfo, AccountLicense, AccountState, GameAccountHandle, GameAccountList,
        GameLevelInfo,
    };
    use crate::product::Product;

    fn program(code: &str) -> u32 {
        super::fourcc(code)
    }

    #[test]
    fn the_games_come_from_both_halves_of_the_account_state() {
        // what the account owns carries the names; what it can log in as
        // carries the handles. a program in either one is a game it has.
        let state = AccountState {
            game_level_info: vec![GameLevelInfo {
                program: Some(program("S2")),
                name: Some("StarCraft II".to_owned()),
                is_trial: Some(false),
                ..GameLevelInfo::default()
            }],
            game_accounts: vec![GameAccountList {
                region: Some(1),
                handle: vec![
                    GameAccountHandle {
                        id: 1,
                        program: program("S2"),
                        region: 1,
                    },
                    GameAccountHandle {
                        id: 2,
                        program: program("S1"),
                        region: 1,
                    },
                ],
            }],
            ..AccountState::default()
        };

        let games = GameProgram::from_account_state(Some(&state));
        assert_eq!(games.len(), 2);

        let sc2 = &games[0];
        assert_eq!(sc2.code(), "S2");
        assert_eq!(sc2.name.as_deref(), Some("StarCraft II"));
        assert_eq!(sc2.accounts, 1);

        // a login handle without level info is still a registered product,
        // but this parser deliberately makes no license claim about it.
        let remastered = &games[1];
        assert_eq!(remastered.code(), "S1");
        assert_eq!(remastered.name, None);
        assert_eq!(remastered.accounts, 1);
    }

    #[test]
    fn an_account_state_that_says_nothing_names_no_games() {
        assert!(GameProgram::from_account_state(None).is_empty());
        assert!(GameProgram::from_account_state(Some(&AccountState::default())).is_empty());
    }

    #[test]
    fn the_code_reads_the_way_it_is_written_on_the_wire() {
        // the FourCC is packed big-endian with the short codes left-padded, so
        // reading it back drops the padding rather than the letters
        let game = GameProgram {
            program: program("S2"),
            name: None,
            is_trial: false,
            is_restricted: false,
            accounts: 0,
        };
        assert_eq!(game.code(), "S2");
    }

    fn catalog(licenses: &[(u32, Option<u64>)], programs: &[&str]) -> AccountCatalog {
        AccountCatalog::from_account_state(Some(&AccountState {
            account_level_info: Some(AccountLevelInfo {
                licenses: licenses
                    .iter()
                    .map(|(id, expires)| AccountLicense {
                        id: *id,
                        expires: *expires,
                    })
                    .collect(),
                ..AccountLevelInfo::default()
            }),
            game_accounts: vec![GameAccountList {
                region: Some(1),
                handle: programs
                    .iter()
                    .enumerate()
                    .map(|(index, code)| GameAccountHandle {
                        id: index as u32 + 1,
                        program: program(code),
                        region: 1,
                    })
                    .collect(),
            }],
            ..AccountState::default()
        }))
    }

    #[test]
    fn wc3_beta_record_does_not_grant_retail() {
        let products = catalog(&[(50_676, None)], &["S2", "W3"]).retail_products_at(100, false);

        assert_eq!(products, vec![Product::StarCraft2]);
    }

    #[test]
    fn captured_non_owner_catalog_hides_wc3_while_owner_catalog_reveals_it() {
        // exact AccountLevelInfo distinction observed from Battle.net Desktop:
        // account 3693 carried beta-only 50676; account 991651410 carried
        // retail 34998. Both had W3 game-account records.
        let non_owner = catalog(
            &[
                (150, None),
                (236, None),
                (260, None),
                (17_019, None),
                (50_676, None),
            ],
            &["S2", "S1", "W3"],
        )
        .retail_products_at(100, false);
        let owner = catalog(&[(34_998, None), (36_198, None)], &["S2", "S1", "W3"])
            .retail_products_at(100, false);

        assert_eq!(non_owner, vec![Product::StarCraft2, Product::Remastered]);
        assert_eq!(
            owner,
            vec![Product::StarCraft2, Product::Remastered, Product::Warcraft3]
        );
    }

    #[test]
    fn wc3_retail_and_legacy_license_ids_grant_retail() {
        for id in [34_998, 9, 13] {
            let products = catalog(&[(id, None)], &["W3"]).retail_products_at(100, false);
            assert!(products.contains(&Product::Warcraft3), "license {id}");
        }
    }

    #[test]
    fn expired_wc3_license_does_not_grant_retail() {
        let products = catalog(&[(34_998, Some(99))], &["W3"]).retail_products_at(100, false);
        assert!(!products.contains(&Product::Warcraft3));
    }

    #[test]
    fn scr_game_account_and_igr_follow_their_explicit_catalog_branches() {
        let scr_products = catalog(&[], &["S1"]).retail_products_at(100, false);
        assert!(scr_products.contains(&Product::Remastered));

        let igr_products = catalog(&[], &[]).retail_products_at(100, true);
        assert!(igr_products.contains(&Product::Warcraft3));
    }

    #[test]
    fn sc2_is_the_catalogs_unconditional_free_product() {
        assert_eq!(
            catalog(&[], &[]).retail_products_at(100, false),
            vec![Product::StarCraft2]
        );
    }
}
