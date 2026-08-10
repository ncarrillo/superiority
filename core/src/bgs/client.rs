use std::time::Duration;

use prost::Message;
use url::Url;

use crate::{
    Error, Result,
    bgs::{
        generated::bgs::protocol::{
            EntityId,
            authentication::v1::{
                GameAccountSelectedRequest, GenerateWebCredentialsRequest,
                GenerateWebCredentialsResponse, LogonResult, LogonUpdateRequest,
                VerifyWebCredentialsRequest,
            },
            challenge::v1::{ChallengeExternalRequest, ChallengeExternalResult},
            connection::v1::{ConnectRequest, ConnectResponse, EchoRequest, EchoResponse},
        },
        model::{
            LogonSession, NativeHandoff, SecretBytes, build_front_request, challenge_url,
            default_logon_request, fourcc,
        },
    },
    wire::{
        protobuf::{RpcFrame, RpcHeader},
        websocket::RpcSocket,
    },
};

const RESPONSE_SERVICE_ID: u32 = 0xfe;
const CONNECTION_SERVICE_HASH: u32 = fnv1a("bnet.protocol.connection.ConnectionService");
const AUTHENTICATION_SERVICE_HASH: u32 = fnv1a("bnet.protocol.authentication.AuthenticationServer");
const AUTHENTICATION_LISTENER_HASH: u32 =
    fnv1a("bnet.protocol.authentication.AuthenticationClient");
const CHALLENGE_LISTENER_HASH: u32 = fnv1a("bnet.protocol.challenge.ChallengeNotify");
const GAME_UTILITIES_SERVICE_HASH: u32 = fnv1a("bnet.protocol.game_utilities.GameUtilities");

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
    next_token: u32,
    connected: bool,
    selected_game_account: Option<EntityId>,
}

impl Client {
    pub fn open(endpoint: &Endpoint) -> Result<Self> {
        Ok(Self {
            socket: RpcSocket::connect(&endpoint.host, endpoint.port, endpoint.timeout)?,
            next_token: 1,
            connected: false,
            selected_game_account: None,
        })
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
            &default_logon_request(cached_web_credentials),
        )?;
        let mut verification_token = None;
        let mut logon_response_seen = false;
        let mut verification_response_seen = false;
        let mut result = None;

        while !(logon_response_seen
            && result.is_some()
            && (verification_token.is_none() || verification_response_seen))
        {
            let frame = self.receive_application_frame()?;
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
                    let credential = challenge_handler.complete(&url)?;
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
        self.require_connection("GenerateWebCredentials")?;
        let token = self.request(
            AUTHENTICATION_SERVICE_HASH,
            8,
            &GenerateWebCredentialsRequest {
                program: Some(fourcc("S2")),
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
        self.require_connection("ProcessClientRequest")?;
        let request = build_front_request(session)?;
        let token = self.request(GAME_UTILITIES_SERVICE_HASH, 1, &request)?;
        let frame = self.await_response(token, "GameUtilities.ProcessClientRequest")?;
        NativeHandoff::decode(&frame.body)
    }

    pub fn close(&mut self) -> Result<()> {
        self.connected = false;
        self.socket.close()
    }

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
}
