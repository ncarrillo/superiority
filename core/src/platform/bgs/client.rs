use std::time::{Duration, Instant};

use prost::Message;
use url::Url;

use crate::{
    Error, Result,
    bgs::{
        generated::bgs::protocol::{
            EntityId,
            account::v1::{
                AccountFieldOptions, GameAccountFieldOptions, GetAccountStateRequest,
                GetAccountStateResponse, GetGameAccountStateRequest, GetGameAccountStateResponse,
            },
            authentication::v1::{
                GameAccountSelectedRequest, GenerateWebCredentialsRequest,
                GenerateWebCredentialsResponse, LogonResult, LogonUpdateRequest,
                VerifyWebCredentialsRequest,
            },
            challenge::v1::{ChallengeExternalRequest, ChallengeExternalResult},
            connection::v1::{ConnectRequest, ConnectResponse, EchoRequest, EchoResponse},
            game_utilities::v1::ClientRequest,
        },
        model::{
            AccountCatalog, GameProgram, LogonSession, NativeHandoff, SecretBytes,
            build_front_request, challenge_url, default_logon_request,
        },
    },
    product::Product,
    wire::{
        protobuf::{RpcFrame, RpcHeader},
        websocket::{RpcSocket, SocketInterrupt, SocketProfile},
    },
};

const RESPONSE_SERVICE_ID: u32 = 0xfe;
const CONNECTION_SERVICE_HASH: u32 = fnv1a("bnet.protocol.connection.ConnectionService");
const AUTHENTICATION_SERVICE_HASH: u32 = fnv1a("bnet.protocol.authentication.AuthenticationServer");
const AUTHENTICATION_LISTENER_HASH: u32 =
    fnv1a("bnet.protocol.authentication.AuthenticationClient");
const CHALLENGE_LISTENER_HASH: u32 = fnv1a("bnet.protocol.challenge.ChallengeNotify");
const GAME_UTILITIES_SERVICE_HASH: u32 = fnv1a("bnet.protocol.game_utilities.GameUtilities");
const ACCOUNT_SERVICE_HASH: u32 = fnv1a("bnet.protocol.account.AccountService");
/// `AccountService.GetAccountState`. The method numbers are not in the
/// generated code — prost drops service definitions — so this one is taken from
/// the reference implementation in `research/.analysis/gophercraft-core`, whose
/// service hash for the same name matches ours exactly.
const ACCOUNT_GET_STATE_METHOD: u32 = 30;
/// `AccountService.GetGameAccountState`, from the same reference.
const ACCOUNT_GET_GAME_STATE_METHOD: u32 = 31;

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub timeout: Duration,
}

impl Default for Endpoint {
    fn default() -> Self {
        Self {
            host: "us.actual.battle.net".into(),
            port: 1119,
            timeout: Duration::from_secs(30),
        }
    }
}

pub trait ChallengeHandler {
    fn complete(&mut self, url: &Url) -> Result<SecretBytes>;
}

impl<F> ChallengeHandler for F
where
    F: FnMut(&Url) -> Result<SecretBytes>,
{
    fn complete(&mut self, url: &Url) -> Result<SecretBytes> {
        self(url)
    }
}

pub struct Client {
    socket: RpcSocket,
    /// which game this connection is signing in as. every call that names a
    /// program reads it from here.
    product: Product,
    next_token: u32,
    connected: bool,
    selected_game_account: Option<EntityId>,
    /// how long the logon may go without the service saying anything. the
    /// socket's own read timeout does not cover this: the service keeps the
    /// connection alive while it answers nothing, and every keepalive resets a
    /// per-read clock. this one is not reset by traffic, only by progress.
    logon_patience: Duration,
}

impl Client {
    pub fn open(endpoint: &Endpoint, product: Product) -> Result<Self> {
        Ok(Self {
            socket: RpcSocket::connect(
                &endpoint.host,
                endpoint.port,
                endpoint.timeout,
                SocketProfile::BGS,
            )?,
            product,
            next_token: 1,
            connected: false,
            selected_game_account: None,
            logon_patience: endpoint.timeout,
        })
    }

    /// a handle another thread can use to end a sign-in that is not going to
    /// finish. see `RpcSocket::interrupt`.
    pub fn interrupt(&self) -> Result<SocketInterrupt> {
        self.socket.interrupt()
    }

    pub fn establish(&mut self) -> Result<ConnectResponse> {
        let request = ConnectRequest {
            use_bindless_rpc: Some(true),
            ..ConnectRequest::default()
        };
        self.send(
            &RpcHeader {
                service_id: 0,
                method_id: Some(1),
                token: 0,
                service_hash: Some(CONNECTION_SERVICE_HASH),
                ..RpcHeader::default()
            },
            &request,
        )?;
        let frame = self.receive()?;
        validate_response(&frame.header, 0, "ConnectionService.Connect")?;
        let response = ConnectResponse::decode(frame.body.as_slice())?;
        self.connected = true;
        Ok(response)
    }

    pub fn authenticate(
        &mut self,
        cached_web_credentials: Option<&[u8]>,
        challenge_handler: &mut impl ChallengeHandler,
    ) -> Result<LogonSession> {
        self.require_connection("authenticate")?;
        let logon_token = self.request(
            AUTHENTICATION_SERVICE_HASH,
            1,
            &default_logon_request(self.product, cached_web_credentials)?,
        )?;
        let mut verification_token = None;
        let mut logon_response_seen = false;
        let mut verification_response_seen = false;
        let mut result = None;
        // the deadline is refreshed by progress, not by traffic — a service
        // that holds the socket open and says nothing has to end the attempt
        // rather than leave the sign-in spinning forever
        let mut deadline = Instant::now() + self.logon_patience;

        while !(logon_response_seen
            && result.is_some()
            && (verification_token.is_none() || verification_response_seen))
        {
            if Instant::now() >= deadline {
                return Err(server_error(
                    "Battle.net accepted the connection but never answered the sign-in",
                ));
            }
            let frame = self.receive_application_frame()?;
            deadline = Instant::now() + self.logon_patience;
            let header = &frame.header;
            if header.service_id == RESPONSE_SERVICE_ID {
                ensure_status_ok(header, "authentication RPC")?;
                if header.token == logon_token {
                    logon_response_seen = true;
                } else if Some(header.token) == verification_token {
                    verification_response_seen = true;
                }
                continue;
            }

            match (header.service_hash, header.method_id) {
                (Some(AUTHENTICATION_LISTENER_HASH), Some(5)) => {
                    let response = LogonResult::decode(frame.body.as_slice())?;
                    result = Some(LogonSession::try_from(response)?);
                }
                (Some(AUTHENTICATION_LISTENER_HASH), Some(10)) => {
                    let update = LogonUpdateRequest::decode(frame.body.as_slice())?;
                    if update.error_code != 0 {
                        return Err(server_error(format!(
                            "Battle.net logon update failed: {}",
                            update.error_code
                        )));
                    }
                }
                (Some(AUTHENTICATION_LISTENER_HASH), Some(14)) => {
                    let selection = GameAccountSelectedRequest::decode(frame.body.as_slice())?;
                    if selection.result != 0 {
                        return Err(server_error(format!(
                            "Battle.net game-account selection failed: {}",
                            selection.result
                        )));
                    }
                    self.selected_game_account = selection.game_account_id;
                }
                (Some(AUTHENTICATION_LISTENER_HASH), Some(11..=13)) => {}
                (Some(CHALLENGE_LISTENER_HASH), Some(3)) => {
                    if verification_token.is_some() {
                        return Err(server_error(
                            "Battle.net issued more than one web challenge",
                        ));
                    }
                    let challenge = ChallengeExternalRequest::decode(frame.body.as_slice())?;
                    let url = challenge_url(challenge)?;
                    // a person is reading a login page now, which takes as long
                    // as it takes; the clock starts again when they are done
                    let credential = challenge_handler.complete(&url)?;
                    deadline = Instant::now() + self.logon_patience;
                    verification_token = Some(self.request(
                        AUTHENTICATION_SERVICE_HASH,
                        7,
                        &VerifyWebCredentialsRequest {
                            web_credentials: Some(credential.expose().to_vec()),
                        },
                    )?);
                }
                (Some(CHALLENGE_LISTENER_HASH), Some(4)) => {
                    let result = ChallengeExternalResult::decode(frame.body.as_slice())?;
                    if !result.passed.unwrap_or(true) {
                        return Err(server_error("Battle.net rejected the external challenge"));
                    }
                }
                _ => {
                    return Err(server_error(format!(
                        "unexpected authentication callback service={:?} method={:?}",
                        header.service_hash, header.method_id
                    )));
                }
            }
        }
        result.ok_or_else(|| server_error("Battle.net returned no LogonResult"))
    }

    pub fn generate_web_credentials(&mut self) -> Result<SecretBytes> {
        self.generate_web_credentials_for(self.product)
    }

    /// Mints a credential for an explicitly named product from this already
    /// authenticated Battle.net session. The account session is authoritative;
    /// `program` only scopes the ticket to the protocol that will consume it.
    pub fn generate_web_credentials_for(&mut self, product: Product) -> Result<SecretBytes> {
        self.require_connection("GenerateWebCredentials")?;
        let token = self.request(
            AUTHENTICATION_SERVICE_HASH,
            8,
            &GenerateWebCredentialsRequest {
                program: Some(product.fourcc()),
            },
        )?;
        let frame = self.await_response(token, "AuthenticationService.GenerateWebCredentials")?;
        let response = GenerateWebCredentialsResponse::decode(frame.body.as_slice())?;
        SecretBytes::new(
            response
                .web_credentials
                .ok_or_else(|| wire_error("GenerateWebCredentials returned no credential"))?,
        )
    }

    pub fn process_client_request(&mut self, session: &LogonSession) -> Result<NativeHandoff> {
        let body = self.game_utilities_request(&build_front_request(session)?)?;
        NativeHandoff::decode(&body)
    }

    /// `GameUtilities.ProcessClientRequest`, with the response handed back raw.
    ///
    /// The call is shared — every product asks this same service where its
    /// server is — but the attributes inside are the product's own, and so is
    /// what comes back: `StarCraft II` is told a host and port, Remastered a
    /// websocket URL. Neither shape belongs in here, so neither is built or
    /// read here.
    pub fn game_utilities_request(&mut self, request: &ClientRequest) -> Result<Vec<u8>> {
        self.require_connection("ProcessClientRequest")?;
        let token = self.request(GAME_UTILITIES_SERVICE_HASH, 1, request)?;
        let frame = self.await_response(token, "GameUtilities.ProcessClientRequest")?;
        Ok(frame.body)
    }

    /// The complete account inputs consumed by Battle.net Desktop's signed
    /// product rules: account-level license ids and game-account handles from
    /// one `GetAccountState` response.
    ///
    /// This deliberately does not reduce the answer to "has any license for
    /// this FourCC". The launcher distinguishes editions by license id; WC3's
    /// beta license creates a W3 handle but does not grant the retail product.
    pub fn account_catalog(&mut self, session: &LogonSession) -> Result<AccountCatalog> {
        self.require_connection("GetAccountState")?;
        let request = GetAccountStateRequest {
            entity_id: session.account_id,
            options: Some(AccountFieldOptions {
                field_account_level_info: Some(true),
                field_game_level_info: Some(true),
                field_game_accounts: Some(true),
                ..AccountFieldOptions::default()
            }),
            ..GetAccountStateRequest::default()
        };
        let token = self.request(ACCOUNT_SERVICE_HASH, ACCOUNT_GET_STATE_METHOD, &request)?;
        let frame = self.await_response(token, "AccountService.GetAccountState")?;
        let response = GetAccountStateResponse::decode(frame.body.as_slice())?;
        let state = response
            .state
            .as_ref()
            .ok_or_else(|| wire_error("GetAccountState returned no account state"))?;
        Ok(AccountCatalog::from_account_state(Some(state)))
    }

    /// What one game account is, asked for by the id the logon handed back.
    /// `GetAccountState` answers with handles and no names; this is the call
    /// that carries a name, if any call does.
    pub fn game_account(&mut self, game_account: EntityId) -> Result<Option<GameProgram>> {
        self.require_connection("GetGameAccountState")?;
        let request = GetGameAccountStateRequest {
            game_account_id: Some(game_account),
            options: Some(GameAccountFieldOptions {
                field_game_level_info: Some(true),
                field_game_status: Some(true),
                ..GameAccountFieldOptions::default()
            }),
            ..GetGameAccountStateRequest::default()
        };
        let token = self.request(
            ACCOUNT_SERVICE_HASH,
            ACCOUNT_GET_GAME_STATE_METHOD,
            &request,
        )?;
        let frame = self.await_response(token, "AccountService.GetGameAccountState")?;
        let response = GetGameAccountStateResponse::decode(frame.body.as_slice())?;
        Ok(response
            .state
            .and_then(|state| state.game_level_info)
            .map(|level| GameProgram {
                program: level.program.unwrap_or_default(),
                name: level.name,
                is_trial: level.is_trial.unwrap_or(false),
                is_restricted: level.is_restricted.unwrap_or(false),
                accounts: 1,
            }))
    }

    pub fn close(&mut self) -> Result<()> {
        self.connected = false;
        self.socket.close()
    }

    /// Sends a request and returns its token.
    ///
    /// `body` is the message itself, **not** its encoded bytes: this encodes it.
    /// Passing `&something.encode_to_vec()` still compiles — prost counts
    /// `Vec<u8>` as a `Message` — and encodes it a second time, which the server
    /// answers by rejecting the call. If a caller already holds bytes, it wants
    /// a different function, not this one.
    fn request<M: Message>(&mut self, service: u32, method: u32, body: &M) -> Result<u32> {
        let token = self.next_token;
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or_else(|| wire_error("BGS request token exhausted"))?;
        self.send(
            &RpcHeader {
                service_id: 0,
                method_id: Some(method),
                token,
                service_hash: Some(service),
                ..RpcHeader::default()
            },
            body,
        )?;
        Ok(token)
    }

    fn send<M: Message>(&mut self, header: &RpcHeader, body: &M) -> Result<()> {
        self.socket.send(header, &body.encode_to_vec())
    }

    fn respond<M: Message>(&mut self, token: u32, body: &M) -> Result<()> {
        self.send(
            &RpcHeader {
                service_id: RESPONSE_SERVICE_ID,
                token,
                status: Some(0),
                ..RpcHeader::default()
            },
            body,
        )
    }

    fn receive(&mut self) -> Result<RpcFrame> {
        self.socket.receive()
    }

    fn receive_application_frame(&mut self) -> Result<RpcFrame> {
        loop {
            let frame = self.receive()?;
            if frame.header.service_hash == Some(CONNECTION_SERVICE_HASH)
                && frame.header.method_id == Some(3)
                && frame.header.service_id != RESPONSE_SERVICE_ID
            {
                let request = EchoRequest::decode(frame.body.as_slice())?;
                self.respond(
                    frame.header.token,
                    &EchoResponse {
                        time: request.time,
                        payload: request.payload,
                    },
                )?;
                continue;
            }
            return Ok(frame);
        }
    }

    fn await_response(&mut self, token: u32, operation: &str) -> Result<RpcFrame> {
        let frame = self.receive_application_frame()?;
        if frame.header.service_id != RESPONSE_SERVICE_ID {
            return Err(server_error(format!(
                "unexpected callback during {operation}: service={:?} method={:?}",
                frame.header.service_hash, frame.header.method_id
            )));
        }
        if frame.header.token != token {
            return Err(server_error(format!(
                "unexpected response token during {operation}: {}",
                frame.header.token
            )));
        }
        ensure_status_ok(&frame.header, operation)?;
        Ok(frame)
    }

    fn require_connection(&self, operation: &str) -> Result<()> {
        if self.connected {
            Ok(())
        } else {
            Err(server_error(format!(
                "establish must succeed before {operation}"
            )))
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.connected {
            let _ = self.socket.close();
        }
    }
}

fn validate_response(header: &RpcHeader, token: u32, operation: &str) -> Result<()> {
    if header.service_id != RESPONSE_SERVICE_ID || header.token != token {
        return Err(server_error(format!("server did not answer {operation}")));
    }
    ensure_status_ok(header, operation)
}

fn ensure_status_ok(header: &RpcHeader, operation: &str) -> Result<()> {
    let status = header.status.unwrap_or(0);
    if status == 0 {
        return Ok(());
    }
    let label = match status {
        1 => "INTERNAL",
        2 => "TIMED_OUT",
        3 => "DENIED",
        7 => "INVALID_ARGS",
        10 => "NO_AUTH",
        3010 => "RPC_INVALID_SERVICE",
        3011 => "RPC_INVALID_METHOD",
        3013 => "RPC_MALFORMED_REQUEST",
        3015 => "RPC_NOT_IMPLEMENTED",
        34_200 => "GAME_UTILITY_SERVER_NO_SERVER",
        _ => "UNKNOWN",
    };
    let nested = header
        .error
        .iter()
        .map(|error| {
            format!(
                "status={} service=0x{:08x} method={}",
                error.status, error.service_hash, error.method_id
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if nested.is_empty() {
        String::new()
    } else {
        format!("; nested errors: {nested}")
    };
    Err(server_error(format!(
        "{operation} failed: {status} ({label}){suffix}"
    )))
}

const fn fnv1a(value: &str) -> u32 {
    let bytes = value.as_bytes();
    let mut hash = 0x811c_9dc5_u32;
    let mut index = 0;
    while index < bytes.len() {
        hash = (hash ^ bytes[index] as u32).wrapping_mul(0x0100_0193);
        index += 1;
    }
    hash
}

fn wire_error(message: impl Into<String>) -> Error {
    Error::BgsWire(message.into())
}

fn server_error(message: impl Into<String>) -> Error {
    Error::Server(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_hashes_match_retail_values() {
        assert_eq!(CONNECTION_SERVICE_HASH, 0x6544_6991);
        assert_eq!(AUTHENTICATION_SERVICE_HASH, 0x0dec_fc01);
        assert_eq!(GAME_UTILITIES_SERVICE_HASH, 0x3fc1_274d);
    }

    #[test]
    fn catalog_request_asks_for_every_launcher_rule_input() {
        let session = LogonSession {
            account_id: Some(EntityId { high: 1, low: 2 }),
            battle_tag: None,
            game_account_ids: Vec::new(),
            available_regions: Vec::new(),
            connected_region: Some(1),
            restricted_mode: false,
            session_key: SecretBytes::new(vec![7; 64]).expect("test key"),
        };
        let request = GetAccountStateRequest {
            entity_id: session.account_id,
            options: Some(AccountFieldOptions {
                field_account_level_info: Some(true),
                field_game_level_info: Some(true),
                field_game_accounts: Some(true),
                ..AccountFieldOptions::default()
            }),
            ..GetAccountStateRequest::default()
        };

        assert_eq!(request.entity_id, session.account_id);
        let options = request.options.expect("account field options");
        assert_eq!(options.field_account_level_info, Some(true));
        assert_eq!(options.field_game_level_info, Some(true));
        assert_eq!(options.field_game_accounts, Some(true));
    }
}
