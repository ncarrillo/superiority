use std::{
    fmt,
    net::{Shutdown, TcpStream, ToSocketAddrs},
    time::Duration,
};

use zeroize::Zeroizing;

use crate::games::sc2::native::errors::native_error;
use crate::{
    Error, Result,
    bgs::NativeHandoff,
    bsn::value::{BsnStruct, BsnValue},
    native::{
        auth::{answer_proof_request, native_server_error, validate_resume_server_proof},
        crypto::thumbprint_context_for_peer,
        protocol::{
            AUTH_CONFIGURATION_COMMAND, AUTH_PROOF_COMMAND, AUTH_RESUME_COMMAND,
            AUTHENTICATION_SLOT, CONNECTION_BOOM_COMMAND, CONNECTION_REGULATOR_COMMAND,
            CONNECTION_SLOT, Protocol, Record,
        },
        stream::RecordStream,
    },
};

pub struct Session {
    records: RecordStream<TcpStream>,
    transport_key: Zeroizing<Vec<u8>>,
    pub use_s3_depot: bool,
}

impl Session {
    #[must_use]
    pub const fn records(&self) -> &RecordStream<TcpStream> {
        &self.records
    }

    pub const fn records_mut(&mut self) -> &mut RecordStream<TcpStream> {
        &mut self.records
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.records.get_ref().set_read_timeout(timeout)?;
        Ok(())
    }

    pub fn close(self) -> Result<()> {
        self.records.get_ref().shutdown(Shutdown::Both)?;
        Ok(())
    }

    /// wrap an already-handshaken, protection-enabled record stream as a session.
    /// used by the loopback harness, which performs the native handshake against a
    /// local Sunken server (verifying the thumbprint with *our* modulus) instead of
    /// going through [`Connector::authenticate`], which pins Blizzard's modulus.
    #[must_use]
    pub fn from_protected(records: RecordStream<TcpStream>, transport_key: Vec<u8>) -> Self {
        Self {
            records,
            transport_key: Zeroizing::new(transport_key),
            use_s3_depot: false,
        }
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("use_s3_depot", &self.use_s3_depot)
            .field("protection_enabled", &self.records.protection_enabled())
            .field("transport_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.records.get_ref().shutdown(Shutdown::Both);
        self.transport_key.fill(0);
    }
}

#[derive(Clone, Debug)]
pub struct Connector {
    protocol: Protocol,
    timeout: Duration,
}

impl Connector {
    #[must_use]
    pub const fn new(protocol: Protocol, timeout: Duration) -> Self {
        Self { protocol, timeout }
    }

    pub fn authenticate(&self, bootstrap: &NativeHandoff) -> Result<Session> {
        self.authenticate_with_handoff(bootstrap, || Ok(()))
    }

    pub fn authenticate_with_handoff<F>(
        &self,
        bootstrap: &NativeHandoff,
        before_resume: F,
    ) -> Result<Session>
    where
        F: FnOnce() -> Result<()>,
    {
        if bootstrap.session_key.len() != 64 {
            return Err(native_error("GameUtilities session seed is not 64 bytes"));
        }
        let logon = self.protocol.decode_logon_parameters(bootstrap)?;
        let (host, port) = bootstrap.endpoint(1119)?;
        let stream = connect(&host, port, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        stream.set_nodelay(true)?;
        let peer_address = stream.peer_addr()?.ip().to_string();
        let mut records = RecordStream::new(stream, self.protocol.clone());

        before_resume()?;
        records.send(
            &self
                .protocol
                .resume_request(bootstrap, Some(logon.game_account_region))?,
        )?;

        let configuration = records.receive()?;
        raise_if_boom(&configuration)?;
        expect_route(
            &configuration,
            AUTHENTICATION_SLOT,
            AUTH_CONFIGURATION_COMMAND,
        )?;
        let configuration = record_struct(&configuration, "configuration response")?;
        let use_s3_depot = required_bool(configuration, "m_useS3Depot")?;

        let proof_request = records.receive()?;
        raise_if_boom(&proof_request)?;
        expect_route(&proof_request, AUTHENTICATION_SLOT, AUTH_PROOF_COMMAND)?;
        let proof_request = record_struct(&proof_request, "proof request")?;
        let answer = answer_proof_request(
            proof_request,
            bootstrap.session_key.expose(),
            &thumbprint_context_for_peer(&peer_address)?,
        )?;
        let outputs = answer.outputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
        records.send(&self.protocol.proof_response(&outputs)?)?;

        let mut resume = records.receive()?;
        raise_if_boom(&resume)?;
        if route(&resume) == (Some(CONNECTION_SLOT), CONNECTION_REGULATOR_COMMAND) {
            resume = records.receive()?;
            raise_if_boom(&resume)?;
        }
        expect_route(&resume, AUTHENTICATION_SLOT, AUTH_RESUME_COMMAND)?;
        let resume_value = record_struct(&resume, "resume response")?;
        validate_resume_server_proof(resume_value, &answer.session)?;

        records.send(&self.protocol.enable_encryption()?)?;
        records.enable_protection(answer.session.transport_key())?;
        Ok(Session {
            records,
            transport_key: Zeroizing::new(answer.session.transport_key().to_vec()),
            use_s3_depot,
        })
    }
}

fn connect(host: &str, port: u16, timeout: Duration) -> Result<TcpStream> {
    let addresses = (host, port).to_socket_addrs()?.collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(Error::Transport(format!(
            "SC2 native endpoint {host}:{port} resolved to no addresses"
        )));
    }
    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .expect("at least one endpoint was attempted")
        .into())
}

fn route(record: &Record) -> (Option<u8>, u8) {
    (record.header.service_slot, record.header.command_id)
}

fn expect_route(record: &Record, slot: u8, command: u8) -> Result<()> {
    if route(record) == (Some(slot), command) {
        Ok(())
    } else {
        Err(native_error(format!(
            "unexpected native record route slot={:?} command={}",
            record.header.service_slot, record.header.command_id
        )))
    }
}

fn raise_if_boom(record: &Record) -> Result<()> {
    if route(record) != (Some(CONNECTION_SLOT), CONNECTION_BOOM_COMMAND) {
        return Ok(());
    }
    let value = record_struct(record, "Connection/Boom")?;
    Err(native_server_error(value)?)
}

fn record_struct<'a>(record: &'a Record, label: &str) -> Result<&'a BsnStruct> {
    match record.value.reflected() {
        Some(BsnValue::Struct(value)) => Ok(value),
        _ => Err(native_error(format!("{label} is not a struct"))),
    }
}

fn required_bool(value: &BsnStruct, field: &str) -> Result<bool> {
    match value.get(field) {
        Some(BsnValue::Bool(value)) => Ok(*value),
        _ => Err(native_error(format!(
            "native struct field {field} is not a bool"
        ))),
    }
}
