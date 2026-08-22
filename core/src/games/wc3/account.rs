use std::{fmt, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Serialize;
use serde_json::Value;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    Error, Result,
    platform::{
        bgs::SecretBytes,
        wire::{
            raw::{self as protobuf, Message},
            websocket::{RpcSocket, SocketProfile},
        },
    },
};

use super::{
    classic::ClassicEndpoint,
    protocol::{
        self, APPLICATION_VERSION, AUTHENTICATION_LISTENER, AUTHENTICATION_SERVICE,
        BGS_SUBPROTOCOL, CONNECTION_SERVICE, GAME_UTILITIES_SERVICE, GAME_VERSION, LOCALE,
        PLATFORM, PLATFORM_FOURCC, TITLE_ID, authentication, authentication_listener,
    },
    schema::{
        Attribute, Base64Bytes, ConnectRequest, ConnectResponse, ExternalChallengeNotification,
        GameAccountHandle as WireGameAccount, GenerateAuthTokenRequest, GenerateAuthTokenResponse,
        LogonCompleteNotification, LogonOptions, LogonQueueState, LogonQueueUpdateNotification,
        LogonRecord, LogonRequest, MeteringLevel, ProcessTaskRequest, ProcessTaskResponse, Variant,
        VerifyAuthTokenRequest,
    },
};

pub trait ChallengeHandler {
    fn resolve(&mut self, url: &Url) -> Result<SecretBytes>;
}

impl<F> ChallengeHandler for F
where
    F: FnMut(&Url) -> Result<SecretBytes>,
{
    fn resolve(&mut self, url: &Url) -> Result<SecretBytes> {
        self(url)
    }
}

#[derive(Clone)]
pub struct ConnectionSession {
    pub ciid: SecretBytes,
    pub connected_region: Option<u32>,
}

impl fmt::Debug for ConnectionSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionSession")
            .field("ciid", &self.ciid)
            .field("connected_region", &self.connected_region)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct GameAccount {
    pub id: u64,
    pub title_id: u32,
    pub region: u32,
}

#[derive(Clone)]
pub struct AuthSession {
    pub account_id: u64,
    pub game_accounts: Vec<GameAccount>,
    pub session_key: SecretBytes,
    pub battle_tag: Option<String>,
    pub employee_only_mode: bool,
}

impl AuthSession {
    pub fn wc3_game_account(&self, connected_region: Option<u32>) -> Result<&GameAccount> {
        let candidates = self
            .game_accounts
            .iter()
            .filter(|account| account.title_id == TITLE_ID)
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => Err(authentication_error("logon named no WC3 game account")),
            [account] => Ok(account),
            many => connected_region
                .and_then(|region| {
                    let matching = many
                        .iter()
                        .copied()
                        .filter(|account| account.region == region)
                        .collect::<Vec<_>>();
                    (matching.len() == 1).then_some(matching[0])
                })
                .ok_or_else(|| authentication_error("logon named multiple WC3 game accounts")),
        }
    }
}

impl fmt::Debug for AuthSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthSession")
            .field("battle_tag", &self.battle_tag)
            .field("game_accounts", &self.game_accounts.len())
            .field("session_key", &self.session_key)
            .field("employee_only_mode", &self.employee_only_mode)
            .finish_non_exhaustive()
    }
}

pub struct Authentication {
    pub session: AuthSession,
    pub browser_used: bool,
    pub queue_updates: Vec<LogonQueueState>,
}

#[derive(Serialize)]
struct RequestHeader {
    method_id: u32,
    service_hash: u32,
    service_id: u32,
    token: u64,
}

struct BgsMessage {
    header: Value,
    body: Value,
}

impl BgsMessage {
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

pub struct AccountClient {
    socket: RpcSocket,
    next_token: u64,
    connection: Option<ConnectionSession>,
    authenticated: bool,
}

impl AccountClient {
    pub fn connect(timeout: Duration) -> Result<Self> {
        let socket = RpcSocket::connect(
            protocol::HOST,
            protocol::PORT,
            timeout,
            SocketProfile {
                path: "/",
                subprotocol: Some(BGS_SUBPROTOCOL),
                lenient_upgrade: false,
            },
        )?;
        Ok(Self {
            socket,
            next_token: 1,
            connection: None,
            authenticated: false,
        })
    }

    pub fn establish(&mut self) -> Result<ConnectionSession> {
        let token = self.request(
            CONNECTION_SERVICE,
            1,
            &ConnectRequest {
                use_bindless_rpc: Some(true),
                metering_level: Some(MeteringLevel::MeteringLevelCategorized),
            },
        )?;
        let response = self.await_response(token, "ConnectionService.Connect")?;
        let response: ConnectResponse = serde_json::from_value(response.body)?;
        let connection = ConnectionSession {
            ciid: SecretBytes::new(
                response
                    .ciid
                    .ok_or_else(|| protocol_error("Connect returned no ciid"))?
                    .into_bytes(),
            )?,
            connected_region: response.connected_region,
        };
        self.connection = Some(connection.clone());
        Ok(connection)
    }

    pub fn authenticate(
        &mut self,
        cached_token: Option<&SecretBytes>,
        challenge: &mut impl ChallengeHandler,
    ) -> Result<Authentication> {
        if self.connection.is_none() {
            return Err(protocol_error("Connect must complete before logon"));
        }
        let logon_options = cached_token.cloned().map(|token| LogonOptions {
            auth_token: Some(Base64Bytes::from_secret(token)),
        });
        let logon_token = self.request(
            AUTHENTICATION_SERVICE,
            authentication::LOGON,
            &LogonRequest {
                title_id: Some(TITLE_ID),
                platform: Some(PLATFORM.into()),
                locale: Some(LOCALE.into()),
                application_version: Some(APPLICATION_VERSION),
                logon_options,
            },
        )?;

        let mut logon_answered = false;
        let mut verification_token = None;
        let mut verification_answered = false;
        let mut session = None;
        let mut browser_used = false;
        let mut queue_updates = Vec::new();

        while session.is_none()
            || !logon_answered
            || (verification_token.is_some() && !verification_answered)
        {
            let message = self.receive_message()?;
            if message.is_response() {
                let token = message.integer("token");
                if token == Some(logon_token) {
                    check_status(&message, "Logon")?;
                    logon_answered = true;
                } else if verification_token.is_some() && token == verification_token {
                    check_status(&message, "VerifyAuthToken")?;
                    verification_answered = true;
                }
                continue;
            }
            if message.integer("service_hash") != Some(u64::from(AUTHENTICATION_LISTENER)) {
                continue;
            }
            let method = message
                .integer("method_id")
                .and_then(|value| u32::try_from(value).ok());
            match method {
                Some(authentication_listener::ON_LOGON_COMPLETE) => {
                    let notification: LogonCompleteNotification =
                        serde_json::from_value(message.body)?;
                    session = Some(parse_logon_complete(notification)?);
                }
                Some(authentication_listener::ON_LOGON_QUEUE_UPDATE) => {
                    let notification: LogonQueueUpdateNotification =
                        serde_json::from_value(message.body)?;
                    queue_updates.extend(notification.state);
                }
                Some(authentication_listener::ON_LOGON_QUEUE_END) => {}
                Some(authentication_listener::ON_EXTERNAL_CHALLENGE) => {
                    if verification_token.is_some() {
                        return Err(authentication_error(
                            "Battle.net issued more than one WC3 challenge",
                        ));
                    }
                    let notification: ExternalChallengeNotification =
                        serde_json::from_value(message.body)?;
                    browser_used = true;
                    let credential = challenge.resolve(&challenge_url(notification)?)?;
                    verification_token = Some(self.request(
                        AUTHENTICATION_SERVICE,
                        authentication::VERIFY_AUTH_TOKEN,
                        &VerifyAuthTokenRequest {
                            auth_token: Some(Base64Bytes::from_secret(credential)),
                        },
                    )?);
                }
                Some(other) => {
                    return Err(protocol_error(format!(
                        "unexpected AuthenticationListener method {other}"
                    )));
                }
                None => return Err(protocol_error("AuthenticationListener has no method")),
            }
        }
        self.authenticated = true;
        Ok(Authentication {
            session: session.expect("loop requires a session"),
            browser_used,
            queue_updates,
        })
    }

    pub fn request_classic_endpoint(&mut self, session: &AuthSession) -> Result<ClassicEndpoint> {
        if !self.authenticated {
            return Err(protocol_error("ProcessTask requires a completed logon"));
        }
        let account = session.wc3_game_account(
            self.connection
                .as_ref()
                .and_then(|connection| connection.connected_region),
        )?;
        let embedded = Message::new()
            .varint(2, u64::from(TITLE_ID))
            .bytes(3, GAME_VERSION.as_bytes())
            .varint(4, u64::from(PLATFORM_FOURCC))
            .varint(5, session.account_id)
            .varint(6, account.id)
            .varint(7, 0)
            .bytes(8, &[])
            .into_vec();
        let request = ProcessTaskRequest {
            attribute: vec![
                string_attribute(
                    "client_request",
                    "classic.protocol.v1.aurora.ConnectToServerRequest",
                ),
                string_attribute("protobuf", &BASE64.encode(embedded)),
                string_attribute("server_instance", "Release"),
            ],
            payload: Vec::new(),
        };
        let token = self.request(GAME_UTILITIES_SERVICE, 1, &request)?;
        let response = self.await_response(token, "GameUtilities.ProcessTask")?;
        parse_process_task(&serde_json::from_value(response.body)?)
    }

    pub fn generate_auth_token(&mut self) -> Result<SecretBytes> {
        let token = self.request(
            AUTHENTICATION_SERVICE,
            authentication::GENERATE_AUTH_TOKEN,
            &GenerateAuthTokenRequest {
                title_id: Some(TITLE_ID),
            },
        )?;
        let response = self.await_response(token, "GenerateAuthToken")?;
        let response: GenerateAuthTokenResponse = serde_json::from_value(response.body)?;
        response
            .auth_token
            .map(Base64Bytes::into_secret)
            .ok_or_else(|| authentication_error("GenerateAuthToken returned no token"))
    }

    pub fn close(&mut self) -> Result<()> {
        self.socket.close()
    }

    fn request<M: Serialize>(
        &mut self,
        service_hash: u32,
        method_id: u32,
        body: &M,
    ) -> Result<u64> {
        let token = self.next_token;
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or_else(|| protocol_error("BGS request token exhausted"))?;
        let header = RequestHeader {
            method_id,
            service_hash,
            service_id: 0,
            token,
        };
        let wire = Zeroizing::new(serde_json::to_string(&(header, body))?);
        self.socket.send_text(&wire)?;
        Ok(token)
    }

    fn receive_message(&mut self) -> Result<BgsMessage> {
        let value: Value = serde_json::from_str(&self.socket.receive_text()?)?;
        let Value::Array(pair) = value else {
            return Err(protocol_error("BGS message is not a header/body pair"));
        };
        let [header, body] = <[Value; 2]>::try_from(pair)
            .map_err(|_| protocol_error("BGS message is not a header/body pair"))?;
        if !header.is_object() || !body.is_object() {
            return Err(protocol_error("BGS message is not a header/body pair"));
        }
        Ok(BgsMessage { header, body })
    }

    fn await_response(&mut self, token: u64, label: &str) -> Result<BgsMessage> {
        loop {
            let message = self.receive_message()?;
            if message.is_response() && message.integer("token") == Some(token) {
                check_status(&message, label)?;
                return Ok(message);
            }
        }
    }
}

fn string_attribute(name: &str, value: &str) -> Attribute {
    Attribute {
        name: Some(name.into()),
        value: Some(Variant {
            string_value: Some(value.into()),
            blob_value: None,
        }),
    }
}

fn parse_process_task(response: &ProcessTaskResponse) -> Result<ClassicEndpoint> {
    let response_type = unique_attribute(&response.result, "client_response")?
        .string_value
        .as_deref();
    if response_type != Some("classic.protocol.v1.aurora.ConnectToServerResponse") {
        return Err(protocol_error(
            "ProcessTask returned an unexpected response type",
        ));
    }
    let payload = unique_attribute(&response.result, "protobuf")?
        .blob_value
        .as_ref()
        .ok_or_else(|| protocol_error("ProcessTask protobuf result is not bytes"))?;
    let url = protobuf::first_bytes(payload.expose(), 1)
        .and_then(|value| std::str::from_utf8(value).ok())
        .ok_or_else(|| protocol_error("ProcessTask returned no endpoint URL"))?;
    let ticket = protobuf::first_bytes(payload.expose(), 2)
        .ok_or_else(|| protocol_error("ProcessTask returned no ticket"))?;
    ClassicEndpoint::from_url(url, SecretBytes::new(ticket.to_vec())?)
}

fn unique_attribute<'a>(attributes: &'a [Attribute], name: &str) -> Result<&'a Variant> {
    let mut matches = attributes
        .iter()
        .filter(|attribute| attribute.name.as_deref() == Some(name));
    let attribute = matches
        .next()
        .ok_or_else(|| protocol_error(format!("ProcessTask has no {name} attribute")))?;
    if matches.next().is_some() {
        return Err(protocol_error(format!(
            "ProcessTask has multiple {name} attributes"
        )));
    }
    attribute
        .value
        .as_ref()
        .ok_or_else(|| protocol_error(format!("ProcessTask {name} has no value")))
}

fn parse_logon_complete(notification: LogonCompleteNotification) -> Result<AuthSession> {
    if notification.error_code.unwrap_or(0) != 0 {
        return Err(authentication_error(format!(
            "WC3 logon failed with error {}",
            notification.error_code.unwrap_or_default()
        )));
    }
    parse_logon_record(
        notification
            .record
            .ok_or_else(|| authentication_error("logon completed without a record"))?,
    )
}

fn parse_logon_record(record: LogonRecord) -> Result<AuthSession> {
    let session_key = record
        .session_key
        .map(Base64Bytes::into_secret)
        .ok_or_else(|| authentication_error("logon record has no session key"))?;
    if session_key.len() != 64 {
        return Err(authentication_error("WC3 session key is not 64 bytes"));
    }
    let game_accounts = record
        .game_account
        .iter()
        .map(parse_game_account)
        .collect::<Result<Vec<_>>>()?;
    if game_accounts.is_empty() {
        return Err(authentication_error("logon record has no game accounts"));
    }
    Ok(AuthSession {
        account_id: record
            .account_id
            .ok_or_else(|| authentication_error("logon record has no account id"))?,
        game_accounts,
        session_key,
        battle_tag: record.battle_tag,
        employee_only_mode: record.employee_only_mode.unwrap_or(false),
    })
}

fn parse_game_account(account: &WireGameAccount) -> Result<GameAccount> {
    Ok(GameAccount {
        id: account
            .id
            .ok_or_else(|| authentication_error("game account has no id"))?,
        title_id: account
            .title_id
            .ok_or_else(|| authentication_error("game account has no title id"))?,
        region: account
            .region
            .ok_or_else(|| authentication_error("game account has no region"))?,
    })
}

fn challenge_url(notification: ExternalChallengeNotification) -> Result<Url> {
    if notification.payload_type.as_deref() != Some("web_auth_url") {
        return Err(authentication_error("unsupported WC3 challenge type"));
    }
    let payload = notification
        .payload
        .ok_or_else(|| authentication_error("WC3 challenge has no payload"))?;
    let text = std::str::from_utf8(payload.expose())
        .map_err(|_| authentication_error("WC3 challenge URL is not UTF-8"))?;
    let url = Url::parse(text).map_err(|_| authentication_error("WC3 challenge URL is invalid"))?;
    if url.scheme() != "https"
        || url
            .host_str()
            .is_none_or(|host| host != "battle.net" && !host.ends_with(".battle.net"))
    {
        return Err(authentication_error("WC3 challenge URL is not trusted"));
    }
    Ok(url)
}

fn check_status(message: &BgsMessage, label: &str) -> Result<()> {
    match message.status() {
        0 => Ok(()),
        status => Err(protocol_error(format!(
            "{label} failed with status {status}"
        ))),
    }
}

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

fn protocol_error(message: impl Into<String>) -> Error {
    Error::BgsWire(message.into())
}

fn authentication_error(message: impl Into<String>) -> Error {
    Error::Authentication(message.into())
}
