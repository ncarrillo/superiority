use std::{
    collections::HashMap,
    fmt::Write as _,
    fs::File,
    io::{BufRead, BufReader},
    net::IpAddr,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use hmac::{Hmac, Mac};
use sc2_core::{
    Error as ProtocolError,
    bgs::NativeHandoff,
    bsn::codec::WireLayoutSupport,
    bsn::value::{BsnStruct, BsnValue},
    native::{
        Payload, Protocol, Rc4State, derive_session_auth_key, derive_transport_rc4_keys,
        inspect::{
            Direction, Record, decode_reflected_outgoing, inspect_native_record,
            read_routing_header,
        },
        protocol::{
            AUTH_PROOF_COMMAND, AUTH_RESUME_COMMAND, AUTHENTICATION_SLOT,
            CONNECTION_ENABLE_ENCRYPTION_COMMAND, CONNECTION_SLOT,
        },
    },
};
use serde::Deserialize;
use sha2::Sha256;

use crate::{Error, Result, packet::TcpDirection, packet::TcpPacket};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FlowKey {
    pub client: (IpAddr, u16),
    pub server: (IpAddr, u16),
}

impl std::fmt::Display for FlowKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{} ⇄ {}:{}",
            self.client.0, self.client.1, self.server.0, self.server.1
        )
    }
}

#[derive(Clone, Debug)]
pub struct UnknownPacket {
    pub flow: FlowKey,
    pub direction: Direction,
    pub stream_offset: usize,
    pub route: Option<(Option<u8>, u8)>,
    pub reason: String,
    pub plaintext: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl std::fmt::Display for UnknownPacket {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let route = self.route.map_or_else(
            || "unreadable".to_owned(),
            |(slot, command)| match slot {
                Some(slot) => format!("slot={slot} command={command}"),
                None => format!("service-less command={command}"),
            },
        );
        writeln!(formatter, "UNKNOWN BSN PACKET — SWEEP HALTED")?;
        writeln!(formatter, "flow: {}", self.flow)?;
        writeln!(formatter, "direction: {}", direction_arrow(self.direction))?;
        writeln!(formatter, "stream offset: {}", self.stream_offset)?;
        writeln!(formatter, "route: {route}")?;
        writeln!(formatter, "reason: {}", self.reason)?;
        writeln!(
            formatter,
            "buffered plaintext ({} bytes; record boundary unknown):",
            self.plaintext.len()
        )?;
        write_hex_dump(formatter, &self.plaintext)?;
        writeln!(
            formatter,
            "buffered ciphertext ({} bytes; record boundary unknown):",
            self.ciphertext.len()
        )?;
        write_hex_dump(formatter, &self.ciphertext)
    }
}

#[derive(Clone, Debug)]
pub enum SweepUpdate {
    Status(String),
    Activated(FlowKey),
    Record(Record),
}

#[derive(Default)]
struct Flow {
    outgoing: TcpDirection,
    incoming: TcpDirection,
}

struct Activation {
    client_nonce: [u8; 16],
    client_proof: [u8; 32],
    server_nonce: [u8; 16],
    outgoing_offset: usize,
    incoming_offset: usize,
}

struct ProtectedDirection {
    cipher: Rc4State,
    cipher_consumed: usize,
    stream_offset: usize,
    plaintext: Vec<u8>,
    ciphertext: Vec<u8>,
}

impl ProtectedDirection {
    fn new(key: &[u8], offset: usize) -> Result<Self> {
        Ok(Self {
            cipher: Rc4State::new(key)?,
            cipher_consumed: offset,
            stream_offset: offset,
            plaintext: Vec::new(),
            ciphertext: Vec::new(),
        })
    }

    fn consume(
        &mut self,
        protocol: &Protocol,
        flow: &FlowKey,
        direction: Direction,
        stream: &[u8],
        next_sequence: &mut u64,
    ) -> Result<Vec<SweepUpdate>> {
        if stream.len() < self.cipher_consumed {
            return Err(Error::Capture(format!(
                "{} TCP stream shrank from {} to {} bytes",
                direction.label(),
                self.cipher_consumed,
                stream.len()
            )));
        }
        let encrypted = &stream[self.cipher_consumed..];
        self.cipher_consumed = stream.len();
        self.ciphertext.extend_from_slice(encrypted);
        self.plaintext.extend(self.cipher.apply(encrypted));

        let mut updates = Vec::new();
        loop {
            if self.plaintext.is_empty() {
                break;
            }
            match inspect_native_record(protocol, direction, &self.plaintext) {
                Ok(mut record) => {
                    let consumed = record.bytes.len();
                    if consumed == 0 || consumed > self.plaintext.len() {
                        return Err(Error::Capture(
                            "native decoder returned an invalid record length".to_owned(),
                        ));
                    }
                    let candidate = candidate_layout(protocol, &record);
                    if candidate {
                        let remaining = &self.plaintext[consumed..];
                        if remaining.len() < 2 {
                            break;
                        }
                        if let Err(error) = inspect_native_record(protocol, direction, remaining)
                            && !matches!(error, ProtocolError::IncompleteFrame(_))
                        {
                            let route = read_routing_header(&self.plaintext)
                                .ok()
                                .map(|(header, _)| (header.service_slot, header.command_id));
                            return Err(Error::UnknownPacket(Box::new(UnknownPacket {
                                flow: flow.clone(),
                                direction,
                                stream_offset: self.stream_offset,
                                route,
                                reason: format!(
                                    "candidate wire layout {} consumed {consumed} bytes, but the following stream does not begin with a decodable record: {error}",
                                    record.type_name
                                ),
                                plaintext: self.plaintext.clone(),
                                ciphertext: self.ciphertext.clone(),
                            })));
                        }
                    }
                    record.sequence = *next_sequence;
                    record.captured_at_millis = now_millis();
                    *next_sequence = next_sequence.wrapping_add(1);
                    self.plaintext.drain(..consumed);
                    self.ciphertext.drain(..consumed);
                    self.stream_offset += consumed;
                    for audit in record.fields.iter().filter(|field| {
                        field.path.starts_with("audit.")
                            && !(candidate && field.path == "audit.wire_layout")
                    }) {
                        updates.push(SweepUpdate::Status(format!(
                            "parser audit: {} — {}",
                            audit.kind, audit.value
                        )));
                    }
                    if candidate {
                        updates.push(SweepUpdate::Status(format!(
                            "candidate wire layout accepted for {}; its {consumed}-byte boundary is consistent with the following record",
                            record.type_name
                        )));
                    }
                    updates.push(SweepUpdate::Record(record));
                }
                Err(ProtocolError::IncompleteFrame(_)) => break,
                Err(error) => {
                    let route = read_routing_header(&self.plaintext)
                        .ok()
                        .map(|(header, _)| (header.service_slot, header.command_id));
                    return Err(Error::UnknownPacket(Box::new(UnknownPacket {
                        flow: flow.clone(),
                        direction,
                        stream_offset: self.stream_offset,
                        route,
                        reason: error.to_string(),
                        plaintext: self.plaintext.clone(),
                        ciphertext: self.ciphertext.clone(),
                    })));
                }
            }
        }
        Ok(updates)
    }

    fn finish(
        &self,
        protocol: &Protocol,
        flow: &FlowKey,
        direction: Direction,
    ) -> Result<Option<SweepUpdate>> {
        if self.plaintext.is_empty() {
            return Ok(None);
        }
        let route = read_routing_header(&self.plaintext)
            .ok()
            .map(|(header, _)| (header.service_slot, header.command_id));
        match inspect_native_record(protocol, direction, &self.plaintext) {
            Ok(record) if candidate_layout(protocol, &record) => {
                return Ok(Some(SweepUpdate::Status(format!(
                    "capture ended after candidate wire layout {} decoded {} bytes; a following record is needed to corroborate its boundary",
                    record.type_name,
                    record.bytes.len()
                ))));
            }
            Err(ProtocolError::IncompleteFrame(_)) => {
                let route = route.map_or_else(
                    || "an unreadable route".to_owned(),
                    |(slot, command)| match slot {
                        Some(slot) => format!("slot {slot}, command {command}"),
                        None => format!("service-less command {command}"),
                    },
                );
                return Ok(Some(SweepUpdate::Status(format!(
                    "capture ended partway through {route}; {} decrypted bytes remain buffered",
                    self.plaintext.len()
                ))));
            }
            Ok(record) => {
                let reason = format!(
                    "capture ended with a complete {} record still buffered unexpectedly",
                    record.type_name
                );
                return Err(Error::UnknownPacket(Box::new(UnknownPacket {
                    flow: flow.clone(),
                    direction,
                    stream_offset: self.stream_offset,
                    route,
                    reason,
                    plaintext: self.plaintext.clone(),
                    ciphertext: self.ciphertext.clone(),
                })));
            }
            Err(error) => {
                let reason = error.to_string();
                return Err(Error::UnknownPacket(Box::new(UnknownPacket {
                    flow: flow.clone(),
                    direction,
                    stream_offset: self.stream_offset,
                    route,
                    reason,
                    plaintext: self.plaintext.clone(),
                    ciphertext: self.ciphertext.clone(),
                })));
            }
        }
    }
}

fn candidate_layout(protocol: &Protocol, record: &Record) -> bool {
    protocol
        .codec()
        .schema()
        .unique_type_id(&record.type_name)
        .and_then(|type_id| protocol.codec().wire_layout_support(type_id))
        .is_ok_and(|support| support == WireLayoutSupport::Candidate)
}

struct ActiveFlow {
    key: FlowKey,
    outgoing: ProtectedDirection,
    incoming: ProtectedDirection,
}

pub struct Sweep {
    protocol: Protocol,
    bootstrap: Option<NativeHandoff>,
    flows: HashMap<FlowKey, Flow>,
    active: Option<ActiveFlow>,
    next_sequence: u64,
    incoming_packets: usize,
    outgoing_packets: usize,
}

impl Sweep {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            bootstrap: None,
            flows: HashMap::new(),
            active: None,
            next_sequence: 1,
            incoming_packets: 0,
            outgoing_packets: 0,
        }
    }

    pub fn set_bootstrap(&mut self, response: &[u8]) -> Result<Vec<SweepUpdate>> {
        self.bootstrap = Some(NativeHandoff::decode(response)?);
        let mut updates = vec![SweepUpdate::Status(
            "GameUtilities handoff captured; searching for native BSN startup".to_owned(),
        )];
        updates.extend(self.try_activate()?);
        Ok(updates)
    }

    pub fn reset(&mut self) {
        self.bootstrap = None;
        self.flows.clear();
        self.active = None;
        self.next_sequence = 1;
        self.incoming_packets = 0;
        self.outgoing_packets = 0;
    }

    pub fn ingest(&mut self, packet: &TcpPacket) -> Result<Vec<SweepUpdate>> {
        if packet.payload.is_empty() {
            return Ok(Vec::new());
        }
        let (key, direction) = packet_flow(packet)?;
        let flow = self.flows.entry(key.clone()).or_default();
        match direction {
            Direction::Outgoing => {
                self.outgoing_packets = self.outgoing_packets.saturating_add(1);
                flow.outgoing.add(packet.sequence, &packet.payload)?;
            }
            Direction::Incoming => {
                self.incoming_packets = self.incoming_packets.saturating_add(1);
                flow.incoming.add(packet.sequence, &packet.payload)?;
            }
        }
        if self.incoming_packets >= 128 && self.outgoing_packets == 0 {
            return Err(Error::Capture(
                "packet capture is receiving only server-to-client traffic; PKTAP did not expose the outbound side"
                    .to_owned(),
            ));
        }

        let mut updates = if self.active.is_none() {
            self.try_activate()?
        } else {
            Vec::new()
        };
        updates.extend(self.consume_active()?);
        Ok(updates)
    }

    pub fn finish(&self) -> Result<Vec<SweepUpdate>> {
        let Some(active) = self.active.as_ref() else {
            return Ok(Vec::new());
        };
        let mut updates = Vec::new();
        if let Some(update) =
            active
                .outgoing
                .finish(&self.protocol, &active.key, Direction::Outgoing)?
        {
            updates.push(update);
        }
        if let Some(update) =
            active
                .incoming
                .finish(&self.protocol, &active.key, Direction::Incoming)?
        {
            updates.push(update);
        }
        Ok(updates)
    }

    fn try_activate(&mut self) -> Result<Vec<SweepUpdate>> {
        let Some(bootstrap) = self.bootstrap.as_ref() else {
            return Ok(Vec::new());
        };
        if self.active.is_some() {
            return Ok(Vec::new());
        }

        let candidate = self.flows.iter().find_map(|(key, flow)| {
            let outgoing = flow.outgoing.bytes();
            let incoming = flow.incoming.bytes();
            if outgoing.starts_with(b"\x16\x03") || outgoing.starts_with(b"GET ") {
                return None;
            }
            parse_activation(&self.protocol, outgoing, incoming)
                .ok()
                .map(|activation| (key.clone(), activation))
        });
        let Some((key, activation)) = candidate else {
            return Ok(Vec::new());
        };

        let protected_secret = derive_session_auth_key(
            bootstrap.session_key.expose(),
            &activation.client_nonce,
            &activation.server_nonce,
        )?;
        let mut verifier = HmacSha256::new_from_slice(&protected_secret)
            .map_err(|_| Error::Bootstrap("could not initialize proof HMAC".to_owned()))?;
        verifier.update(&[0]);
        verifier.update(&activation.client_nonce);
        verifier.update(&activation.server_nonce);
        verifier
            .verify_slice(&activation.client_proof)
            .map_err(|_| {
                Error::Bootstrap("derived key does not validate the client proof".into())
            })?;
        let (incoming_key, outgoing_key) = derive_transport_rc4_keys(&protected_secret)?;

        let outgoing = ProtectedDirection::new(&outgoing_key, activation.outgoing_offset)?;
        let incoming = ProtectedDirection::new(&incoming_key, activation.incoming_offset)?;
        self.active = Some(ActiveFlow {
            key: key.clone(),
            outgoing,
            incoming,
        });

        let mut updates = vec![SweepUpdate::Activated(key.clone())];
        let flow = self
            .flows
            .get(&key)
            .expect("the activated flow came from the flow map");
        for (direction, stream, end) in [
            (
                Direction::Outgoing,
                flow.outgoing.bytes(),
                activation.outgoing_offset,
            ),
            (
                Direction::Incoming,
                flow.incoming.bytes(),
                activation.incoming_offset,
            ),
        ] {
            let mut offset = 0;
            while offset < end {
                let mut record =
                    inspect_native_record(&self.protocol, direction, &stream[offset..end])?;
                offset += record.bytes.len();
                record.sequence = self.next_sequence;
                record.captured_at_millis = now_millis();
                self.next_sequence = self.next_sequence.wrapping_add(1);
                updates.push(SweepUpdate::Record(record));
            }
        }
        Ok(updates)
    }

    fn consume_active(&mut self) -> Result<Vec<SweepUpdate>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(Vec::new());
        };
        let flow = self
            .flows
            .get(&active.key)
            .expect("the active flow remains in the flow map");
        let mut updates = Vec::new();
        updates.extend(active.outgoing.consume(
            &self.protocol,
            &active.key,
            Direction::Outgoing,
            flow.outgoing.bytes(),
            &mut self.next_sequence,
        )?);
        updates.extend(active.incoming.consume(
            &self.protocol,
            &active.key,
            Direction::Incoming,
            flow.incoming.bytes(),
            &mut self.next_sequence,
        )?);
        Ok(updates)
    }
}

pub fn load_bootstrap(path: &Path) -> Result<Vec<u8>> {
    #[derive(Deserialize)]
    struct LogRecord {
        #[serde(rename = "type")]
        kind: Option<String>,
        hex: Option<String>,
    }

    let file = BufReader::new(File::open(path)?);
    for line in file.lines() {
        let line = line?;
        let Ok(record) = serde_json::from_str::<LogRecord>(&line) else {
            continue;
        };
        if record.kind.as_deref() == Some("game_utilities_client_response") {
            let encoded = record.hex.ok_or_else(|| {
                Error::Bootstrap("GameUtilities ClientResponse has no hex payload".to_owned())
            })?;
            return hex::decode(encoded).map_err(|error| {
                Error::Bootstrap(format!("GameUtilities payload is not valid hex: {error}"))
            });
        }
    }
    Err(Error::Bootstrap(
        "GameUtilities log has no ClientResponse".to_owned(),
    ))
}

fn parse_activation(protocol: &Protocol, outgoing: &[u8], incoming: &[u8]) -> Result<Activation> {
    let resume = decode_reflected_outgoing(protocol, outgoing)?;
    require_route(
        resume.header.service_slot,
        resume.header.command_id,
        AUTHENTICATION_SLOT,
        AUTH_RESUME_COMMAND,
        "client startup",
    )?;
    let mut outgoing_offset = resume.byte_count;

    let proof = decode_reflected_outgoing(protocol, &outgoing[outgoing_offset..])?;
    require_route(
        proof.header.service_slot,
        proof.header.command_id,
        AUTHENTICATION_SLOT,
        AUTH_PROOF_COMMAND,
        "client proof",
    )?;
    let proof_data = client_proof_data(&proof.value)?;
    let client_nonce = proof_data[1..17]
        .try_into()
        .expect("the client nonce range is fixed");
    let client_proof = proof_data[17..49]
        .try_into()
        .expect("the client proof range is fixed");
    outgoing_offset += proof.byte_count;

    let marker = protocol.enable_encryption()?;
    if outgoing.len() < outgoing_offset + marker.len() {
        return Err(ProtocolError::IncompleteFrame(
            "client encryption marker is incomplete".to_owned(),
        )
        .into());
    }
    if outgoing[outgoing_offset..outgoing_offset + marker.len()] != marker {
        return Err(Error::Bootstrap(
            "client proof is not followed by Connection/EnableEncryption".to_owned(),
        ));
    }
    let (marker_header, _) = read_routing_header(&marker)?;
    require_route(
        marker_header.service_slot,
        marker_header.command_id,
        CONNECTION_SLOT,
        CONNECTION_ENABLE_ENCRYPTION_COMMAND,
        "encryption marker",
    )?;
    outgoing_offset += marker.len();

    let mut incoming_offset = 0;
    let mut server_nonce = None;
    let mut saw_resume = false;
    for _ in 0..64 {
        let record =
            inspect_native_record(protocol, Direction::Incoming, &incoming[incoming_offset..])?;
        let (header, mut reader) = read_routing_header(&record.bytes)?;
        let (_, payload) = protocol.decode_incoming_from(&mut reader, header)?;
        incoming_offset += record.bytes.len();
        let route = (header.service_slot, header.command_id);
        if route == (Some(AUTHENTICATION_SLOT), AUTH_PROOF_COMMAND) {
            let Payload::Reflected(value) = payload else {
                return Err(Error::Bootstrap(
                    "server proof request was not a reflected BSN value".to_owned(),
                ));
            };
            let data = server_proof_data(&value)?;
            server_nonce = Some(
                data[1..17]
                    .try_into()
                    .expect("the server nonce range is fixed"),
            );
        }
        if route == (Some(AUTHENTICATION_SLOT), AUTH_RESUME_COMMAND) {
            saw_resume = true;
            break;
        }
    }
    let server_nonce = server_nonce
        .ok_or_else(|| Error::Bootstrap("server startup has no session proof nonce".to_owned()))?;
    if !saw_resume {
        return Err(Error::Bootstrap(
            "server startup has no Authentication/Resume response".to_owned(),
        ));
    }
    Ok(Activation {
        client_nonce,
        client_proof,
        server_nonce,
        outgoing_offset,
        incoming_offset,
    })
}

fn client_proof_data(value: &BsnValue) -> Result<&[u8]> {
    let root = require_struct(value, "client proof")?;
    let modules = require_array(require_field(root, "m_response")?, "m_response")?;
    modules
        .iter()
        .filter_map(|module| {
            require_struct(module, "client proof module")
                .ok()?
                .get("m_data")
                .and_then(expect_bytes)
        })
        .find(|data| data.len() == 49 && data[0] == 1)
        .ok_or_else(|| Error::Bootstrap("client proof has no 49-byte session module".to_owned()))
}

fn server_proof_data(value: &BsnValue) -> Result<&[u8]> {
    let root = require_struct(value, "server proof")?;
    let modules = require_array(require_field(root, "m_request")?, "m_request")?;
    for module in modules {
        let module = require_struct(module, "server proof module")?;
        let Some(identifier) = module.get("m_id").and_then(expect_bytes) else {
            continue;
        };
        if identifier == sc2_core::native::auth::SESSION_PROOF_MODULE_ID {
            let data = module
                .get("m_data")
                .and_then(expect_bytes)
                .ok_or_else(|| Error::Bootstrap("session proof module has no data".to_owned()))?;
            if data.len() == 17 && data[0] == 0 {
                return Ok(data);
            }
            return Err(Error::Bootstrap(
                "server session proof has an unexpected phase or length".to_owned(),
            ));
        }
    }
    Err(Error::Bootstrap(
        "server proof request has no session module".to_owned(),
    ))
}

fn transparent(value: &BsnValue) -> &BsnValue {
    if let BsnValue::Optional(Some(value)) = value {
        transparent(value)
    } else {
        value
    }
}

fn require_struct<'a>(value: &'a BsnValue, label: &str) -> Result<&'a BsnStruct> {
    transparent(value)
        .as_struct()
        .ok_or_else(|| Error::Bootstrap(format!("{label} is not a struct")))
}

fn require_array<'a>(value: &'a BsnValue, label: &str) -> Result<&'a [BsnValue]> {
    match transparent(value) {
        BsnValue::Array(values) => Ok(values),
        _ => Err(Error::Bootstrap(format!("{label} is not an array"))),
    }
}

fn require_field<'a>(value: &'a BsnStruct, name: &str) -> Result<&'a BsnValue> {
    value
        .get(name)
        .ok_or_else(|| Error::Bootstrap(format!("BSN value has no {name}")))
}

fn expect_bytes(value: &BsnValue) -> Option<&[u8]> {
    if let BsnValue::Bytes(bytes) = transparent(value) {
        Some(bytes)
    } else {
        None
    }
}

fn require_route(
    actual_slot: Option<u8>,
    actual_command: u8,
    expected_slot: u8,
    expected_command: u8,
    label: &str,
) -> Result<()> {
    if actual_slot == Some(expected_slot) && actual_command == expected_command {
        Ok(())
    } else {
        Err(Error::Bootstrap(format!(
            "{label} has route slot={actual_slot:?} command={actual_command}; expected slot={expected_slot} command={expected_command}"
        )))
    }
}

fn packet_flow(packet: &TcpPacket) -> Result<(FlowKey, Direction)> {
    if packet.destination_port == 1119 {
        Ok((
            FlowKey {
                client: (packet.source, packet.source_port),
                server: (packet.destination, packet.destination_port),
            },
            Direction::Outgoing,
        ))
    } else if packet.source_port == 1119 {
        Ok((
            FlowKey {
                client: (packet.destination, packet.destination_port),
                server: (packet.source, packet.source_port),
            },
            Direction::Incoming,
        ))
    } else {
        Err(Error::Capture(
            "packet does not belong to a TCP/1119 flow".to_owned(),
        ))
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn direction_arrow(direction: Direction) -> &'static str {
    match direction {
        Direction::Incoming => "S→C",
        Direction::Outgoing => "C→S",
    }
}

fn write_hex_dump(formatter: &mut std::fmt::Formatter<'_>, bytes: &[u8]) -> std::fmt::Result {
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let mut hex = String::new();
        let mut text = String::new();
        for (index, byte) in chunk.iter().enumerate() {
            if index == 8 {
                hex.push(' ');
            }
            let _ = write!(hex, "{byte:02x} ");
            text.push(if byte.is_ascii_graphic() || *byte == b' ' {
                char::from(*byte)
            } else {
                '·'
            });
        }
        writeln!(formatter, "  {:08x}  {hex:<49} │{text}│", row * 16)?;
    }
    Ok(())
}
