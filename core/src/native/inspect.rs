use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    Result,
    bsn::bits::{BitReader, RoutingHeader},
    bsn::codec::DecodedField,
    bsn::value::BsnValue,
};

use super::{
    Payload, Protocol,
    protocol::{
        AUTH_LOGON_COMMAND, AUTH_PROOF_COMMAND, AUTH_RESUME_COMMAND, AUTH_SINGLE_SIGN_ON_COMMAND,
        AUTHENTICATION_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND, CACHE_SLOT,
        CHAT_CHANNEL_LIST_REQUEST_COMMAND, CHAT_ENUM_CONFERENCES_COMMAND,
        CHAT_INVITE_ACCEPT_COMMAND, CHAT_INVITE_DECLINE_COMMAND, CHAT_JOIN_REQUEST_COMMAND,
        CHAT_LEAVE_REQUEST_COMMAND, CHAT_MESSAGE_COMMAND, CHAT_SLOT, CHAT_STATUS_CHANGE_COMMAND,
        CHAT_WHISPER_SEND_COMMAND, CONNECTION_ENABLE_ENCRYPTION_COMMAND,
        CONNECTION_MESSAGE_FRAME_COMMAND, CONNECTION_PING_COMMAND, CONNECTION_SLOT, FRIENDS_SLOT,
        FRIENDS_TOONS_COMMAND, PRESENCE_SLOT, PRESENCE_STATISTICS_SUBSCRIBE_COMMAND,
        PRESENCE_TEMPORARY_COMMAND, PROFILE_ADDRESS_QUERY_COMMAND, PROFILE_READ_COMMAND,
        PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND, PROFILE_SLOT,
        S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND,
        S2_MULTIPLAYER_INVITE_ACTION_COMMAND, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND,
        S2_MULTIPLAYER_SLOT, TOON_SELECT_COMMAND, TOON_SLOT,
    },
};

const CAPTURE_LIMIT: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldRole {
    Route,
    Control,
    Payload,
    Padding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Incoming,
    Outgoing,
}

impl Direction {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
        }
    }

    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Incoming => "←",
            Self::Outgoing => "→",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub path: String,
    pub kind: &'static str,
    pub value: String,
    pub start_bit: usize,
    pub end_bit: usize,
    pub depth: usize,
    pub role: FieldRole,
    pub exact_range: bool,
}

impl Field {
    fn leaf(
        path: impl Into<String>,
        kind: &'static str,
        value: impl Into<String>,
        start_bit: usize,
        end_bit: usize,
        depth: usize,
        role: FieldRole,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            value: value.into(),
            start_bit,
            end_bit,
            depth,
            role,
            exact_range: true,
        }
    }

    fn decoded(
        path: impl Into<String>,
        kind: &'static str,
        value: impl Into<String>,
        start_bit: usize,
        end_bit: usize,
        depth: usize,
    ) -> Self {
        let mut field = Self::leaf(
            path,
            kind,
            value,
            start_bit,
            end_bit,
            depth,
            FieldRole::Payload,
        );
        field.exact_range = false;
        field
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub sequence: u64,
    pub captured_at_millis: u64,
    pub direction: Direction,
    pub service: String,
    pub command: String,
    pub type_name: String,
    pub service_slot: u8,
    pub command_id: u8,
    pub bytes: Vec<u8>,
    pub logical_bits: usize,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Capture {
    pub records: Vec<Record>,
}

#[derive(Default)]
struct CaptureState {
    next_sequence: u64,
    protocol: Option<Protocol>,
    paused: bool,
    records: VecDeque<RawRecord>,
}

#[derive(Clone)]
struct RawRecord {
    sequence: u64,
    captured_at_millis: u64,
    direction: Direction,
    header: RoutingHeader,
    bytes: Vec<u8>,
    logical_bits: usize,
}

static CAPTURE: OnceLock<Mutex<CaptureState>> = OnceLock::new();

#[must_use]
pub fn live_capture_after(sequence: Option<u64>) -> Capture {
    let (protocol, records) = {
        let state = capture_state()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let records = state
            .records
            .iter()
            .filter(|record| sequence.is_none_or(|sequence| record.sequence > sequence))
            .cloned()
            .collect::<Vec<_>>();
        (state.protocol.clone(), records)
    };
    Capture {
        records: protocol.map_or_else(Vec::new, |protocol| {
            records
                .iter()
                .filter_map(|raw| {
                    let mut record = match raw.direction {
                        Direction::Incoming => {
                            let mut reader = BitReader::new(&raw.bytes, None).ok()?;
                            reader.set_position(raw.header.bit_count).ok()?;
                            let decoded = protocol
                                .decode_incoming_with_provenance_from(&mut reader, raw.header)
                                .ok()?;
                            inspect_decoded(
                                &protocol,
                                DecodedRecord {
                                    direction: Direction::Incoming,
                                    header: raw.header,
                                    type_id: decoded.type_id,
                                    payload: &decoded.payload,
                                    provenance: &decoded.provenance,
                                    bytes: &raw.bytes,
                                    logical_bits: raw.logical_bits,
                                },
                            )?
                        }
                        Direction::Outgoing => inspect_outgoing(&protocol, &raw.bytes).ok()?,
                    };
                    record.sequence = raw.sequence;
                    record.captured_at_millis = raw.captured_at_millis;
                    Some(record)
                })
                .collect()
        }),
    }
}

#[must_use]
pub fn capture_paused() -> bool {
    capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .paused
}

pub fn set_capture_paused(paused: bool) {
    capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .paused = paused;
}

pub fn clear_capture() {
    capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .records
        .clear();
}

pub(crate) fn capture_incoming(
    protocol: &Protocol,
    header: RoutingHeader,
    bytes: &[u8],
    logical_bits: usize,
) {
    capture_record(protocol, Direction::Incoming, header, bytes, logical_bits);
}

pub(crate) fn capture_outgoing(protocol: &Protocol, bytes: &[u8]) {
    let Ok((header, _)) = read_routing_header(bytes) else {
        return;
    };
    capture_record(
        protocol,
        Direction::Outgoing,
        header,
        bytes,
        bytes.len() * 8,
    );
}

fn capture_record(
    protocol: &Protocol,
    direction: Direction,
    header: RoutingHeader,
    bytes: &[u8],
    logical_bits: usize,
) {
    let mut state = capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.paused {
        return;
    }
    state.protocol.get_or_insert_with(|| protocol.clone());
    let sequence = state.next_sequence;
    state.next_sequence = state.next_sequence.wrapping_add(1);
    if state.records.len() == CAPTURE_LIMIT {
        state.records.pop_front();
    }
    state.records.push_back(RawRecord {
        sequence,
        captured_at_millis: now_millis(),
        direction,
        header,
        bytes: bytes.to_vec(),
        logical_bits,
    });
}

fn capture_state() -> &'static Mutex<CaptureState> {
    CAPTURE.get_or_init(|| Mutex::new(CaptureState::default()))
}

#[must_use]
pub fn sample_capture() -> Capture {
    let protocol = Protocol::current().expect("the bundled native schema must load");
    let records = [
        ("General", 0x1020_3040, 2, 0x1122_3344),
        ("Arcade", 0x5060_7080, 3, 0x5566_7788),
        ("Co-op", 0x90a0_b0c0, 4, 0x99aa_bbcc),
    ]
    .into_iter()
    .map(|(name, member, channel, token)| {
        let bytes = sample_chat_join_bytes(name, member, channel, token);
        inspect_chat_join(&protocol, &bytes)
            .expect("the protocol viewer fixture must decode with the bundled schema")
    })
    .collect();
    Capture { records }
}

#[allow(clippy::too_many_lines)]
pub fn inspect_chat_join(protocol: &Protocol, bytes: &[u8]) -> Result<Record> {
    let (header, mut reader) = read_routing_header(bytes)?;
    let decoded = protocol.decode_incoming_with_provenance_from(&mut reader, header)?;
    let logical_bits = reader.position();
    inspect_decoded(
        protocol,
        DecodedRecord {
            direction: Direction::Incoming,
            header,
            type_id: decoded.type_id,
            payload: &decoded.payload,
            provenance: &decoded.provenance,
            bytes,
            logical_bits,
        },
    )
    .ok_or_else(|| crate::Error::Native("chat join has no service slot".to_owned()))
}

fn read_routing_header(bytes: &[u8]) -> Result<(RoutingHeader, BitReader<'_>)> {
    let mut reader = BitReader::new(bytes, None)?;
    let command_id = u8::try_from(reader.read(6)?).expect("six bits fit in u8");
    let service_slot = if reader.read(1)? == 0 {
        None
    } else {
        Some(u8::try_from(reader.read(4)?).expect("four bits fit in u8"))
    };
    Ok((
        RoutingHeader {
            command_id,
            service_slot,
            bit_count: reader.position(),
        },
        reader,
    ))
}

fn inspect_outgoing(protocol: &Protocol, bytes: &[u8]) -> Result<Record> {
    let (header, mut reader) = read_routing_header(bytes)?;
    let service_slot = header.service_slot.ok_or_else(|| {
        crate::Error::Native("native client record has no service slot".to_owned())
    })?;
    let route = (service_slot, header.command_id);
    let reflected_type_name = reflected_outgoing_type(route);

    if let Some(type_name) = reflected_type_name
        && let Ok(type_id) = protocol.codec().schema().unique_type_id(type_name)
        && let Ok(decoded) = protocol.codec().decode_traced_from(&mut reader, type_id)
    {
        let payload = Payload::Reflected(decoded.value);
        let logical_bits = reader.position();
        return inspect_decoded(
            protocol,
            DecodedRecord {
                direction: Direction::Outgoing,
                header,
                type_id,
                payload: &payload,
                provenance: &decoded.fields,
                bytes,
                logical_bits,
            },
        )
        .ok_or_else(|| crate::Error::Native("outgoing record has no service slot".to_owned()));
    }

    reader.set_position(header.bit_count)?;
    let type_name = reflected_type_name
        .or_else(|| manual_outgoing_type(route))
        .map_or_else(
            || {
                format!(
                    "Battlenet::Client::{}::Command{}Request",
                    service_name(service_slot),
                    header.command_id
                )
            },
            str::to_owned,
        );
    let (service, command) = labels(&type_name);
    let mut fields = route_fields(header, service_slot);
    let payload_start = reader.position();
    let payload_index = fields.len();
    fields.push(Field::leaf(
        "payload",
        "outgoing payload",
        command.clone(),
        payload_start,
        payload_start,
        0,
        FieldRole::Payload,
    ));
    let decoded = decode_manual_outgoing(route, &mut reader, &mut fields);
    let logical_bits = if decoded.is_ok() {
        reader.position()
    } else {
        fields.truncate(payload_index + 1);
        bytes.len() * 8
    };
    fields[payload_index].end_bit = logical_bits;
    if decoded.is_err() {
        fields[payload_index].kind = "raw outgoing payload";
        fields[payload_index].value = format!("{} bits", logical_bits - payload_start);
        fields[payload_index].exact_range = false;
    }
    append_padding(&mut fields, logical_bits, bytes.len() * 8);
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: Direction::Outgoing,
        service,
        command,
        type_name,
        service_slot,
        command_id: header.command_id,
        bytes: bytes.to_vec(),
        logical_bits,
        fields,
    })
}

fn reflected_outgoing_type(route: (u8, u8)) -> Option<&'static str> {
    Some(match route {
        (AUTHENTICATION_SLOT, AUTH_LOGON_COMMAND) => {
            "Battlenet::Client::Authentication::LogonRequest3"
        }
        (AUTHENTICATION_SLOT, AUTH_RESUME_COMMAND) => {
            "Battlenet::Client::Authentication::ResumeRequest"
        }
        (AUTHENTICATION_SLOT, AUTH_PROOF_COMMAND) => {
            "Battlenet::Client::Authentication::ProofResponse"
        }
        (AUTHENTICATION_SLOT, AUTH_SINGLE_SIGN_ON_COMMAND) => {
            "Battlenet::Client::Authentication::SingleSignOnRequest3"
        }
        (CONNECTION_SLOT, CONNECTION_ENABLE_ENCRYPTION_COMMAND) => {
            "Battlenet::Client::Connection::EnableEncryption"
        }
        (CONNECTION_SLOT, CONNECTION_PING_COMMAND) => "Battlenet::Client::Connection::Ping",
        (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND) => {
            "Battlenet::Client::Connection::MessageFrame"
        }
        (CHAT_SLOT, CHAT_STATUS_CHANGE_COMMAND) => "Battlenet::Client::Chat::StatusChangeRequest",
        (CHAT_SLOT, CHAT_CHANNEL_LIST_REQUEST_COMMAND) => {
            "Battlenet::Client::Chat::ChannelListRequest"
        }
        (CHAT_SLOT, CHAT_ENUM_CONFERENCES_COMMAND) => {
            "Battlenet::Client::Chat::EnumConferenceDescriptions"
        }
        (FRIENDS_SLOT, FRIENDS_TOONS_COMMAND) => {
            "Battlenet::Client::Friends::ToonsOfFriendsRequest"
        }
        (PRESENCE_SLOT, PRESENCE_STATISTICS_SUBSCRIBE_COMMAND) => {
            "Battlenet::Client::Presence::StatisticsSubscribe"
        }
        (PRESENCE_SLOT, PRESENCE_TEMPORARY_COMMAND) => {
            "Battlenet::Client::Presence::TemporaryPresenceRequest"
        }
        (PROFILE_SLOT, PROFILE_ADDRESS_QUERY_COMMAND) => {
            "Battlenet::Client::Profile::AddressQueryRequest"
        }
        (PROFILE_SLOT, PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND) => {
            "Battlenet::Client::Profile::ResolveToonNameToHandleRequest"
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND) => {
            "Battlenet::Client::Club::SearchClubsRequest"
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND) => {
            "Battlenet::Client::Club::GetClubInfoRequest"
        }
        _ => return None,
    })
}

fn manual_outgoing_type(route: (u8, u8)) -> Option<&'static str> {
    Some(match route {
        (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND) => {
            "Battlenet::Client::Cache::GetStreamItemsRequest"
        }
        (CHAT_SLOT, CHAT_JOIN_REQUEST_COMMAND) => "Battlenet::Client::Chat::JoinRequest",
        (CHAT_SLOT, CHAT_LEAVE_REQUEST_COMMAND) => "Battlenet::Client::Chat::LeaveRequest",
        (CHAT_SLOT, CHAT_INVITE_ACCEPT_COMMAND) => "Battlenet::Client::Chat::InviteAcceptRequest",
        (CHAT_SLOT, CHAT_INVITE_DECLINE_COMMAND) => "Battlenet::Client::Chat::InviteDeclineRequest",
        (CHAT_SLOT, CHAT_MESSAGE_COMMAND) => "Battlenet::Client::Chat::SendMessage",
        (CHAT_SLOT, CHAT_WHISPER_SEND_COMMAND) => "Battlenet::Client::Chat::SendWhisper",
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND) => {
            "Battlenet::Client::Club::GetToonClubsRequest"
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_INVITE_ACTION_COMMAND) => {
            "Battlenet::Client::Club::InvitationActionRequest"
        }
        (TOON_SLOT, TOON_SELECT_COMMAND) => "Battlenet::Client::Toon::SelectRequest",
        (PROFILE_SLOT, PROFILE_READ_COMMAND) => "Battlenet::Client::Profile::ReadRequest",
        _ => return None,
    })
}

fn service_name(slot: u8) -> &'static str {
    match slot {
        AUTHENTICATION_SLOT => "Authentication",
        CONNECTION_SLOT => "Connection",
        FRIENDS_SLOT => "Friends",
        PRESENCE_SLOT => "Presence",
        CHAT_SLOT => "Chat",
        CACHE_SLOT => "Cache",
        S2_MULTIPLAYER_SLOT => "Club",
        PROFILE_SLOT => "Profile",
        TOON_SLOT => "Toon",
        _ => "Native",
    }
}

fn decode_manual_outgoing(
    route: (u8, u8),
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
) -> Result<()> {
    match route {
        (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND) => decode_cache_request(reader, fields),
        (CHAT_SLOT, CHAT_JOIN_REQUEST_COMMAND) => decode_chat_join_request(reader, fields),
        (
            CHAT_SLOT,
            CHAT_LEAVE_REQUEST_COMMAND | CHAT_INVITE_ACCEPT_COMMAND | CHAT_INVITE_DECLINE_COMMAND,
        ) => {
            read_number(reader, fields, "payload.channel_index", "uint3", 3, 1)?;
            Ok(())
        }
        (CHAT_SLOT, CHAT_MESSAGE_COMMAND) => {
            read_utf8(reader, fields, "payload.body", 10, 1)?;
            read_number(reader, fields, "payload.channel_index", "uint3", 3, 1)?;
            Ok(())
        }
        (CHAT_SLOT, CHAT_WHISPER_SEND_COMMAND) => decode_whisper_request(reader, fields),
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND) => {
            read_number(reader, fields, "payload.token", "uint32", 32, 1)?;
            read_number(reader, fields, "payload.toon.program_id", "fourcc", 32, 1)?;
            read_number(reader, fields, "payload.toon.region", "uint8", 8, 1)?;
            read_number(reader, fields, "payload.toon.realm", "uint32", 32, 1)?;
            read_number(reader, fields, "payload.toon.id", "uint64", 64, 1)?;
            Ok(())
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_INVITE_ACTION_COMMAND) => {
            read_number(reader, fields, "payload.action", "uint2", 2, 1)?;
            read_number(reader, fields, "payload.program_id", "fourcc", 32, 1)?;
            read_number(reader, fields, "payload.region", "uint8", 8, 1)?;
            read_number(reader, fields, "payload.realm", "uint32", 32, 1)?;
            read_number(reader, fields, "payload.toon_id", "uint64", 64, 1)?;
            read_number(reader, fields, "payload.club_id", "uint32", 32, 1)?;
            read_number(reader, fields, "payload.reserved", "uint11", 11, 1)?;
            read_number(reader, fields, "payload.trailing", "uint16", 16, 1)?;
            Ok(())
        }
        (TOON_SLOT, TOON_SELECT_COMMAND) => {
            let encoded_length =
                read_number(reader, fields, "payload.name.encoded_length", "uint7", 7, 2)?;
            read_utf8_bytes(
                reader,
                fields,
                "payload.name",
                usize::try_from(encoded_length + 2).unwrap_or(usize::MAX),
                1,
            )?;
            read_number(reader, fields, "payload.checksum", "uint10", 10, 1)?;
            read_number(reader, fields, "payload.realm", "uint32", 32, 1)?;
            Ok(())
        }
        _ => Err(crate::Error::Native(
            "outgoing route has no structured decoder".to_owned(),
        )),
    }
}

fn decode_cache_request(reader: &mut BitReader<'_>, fields: &mut Vec<Field>) -> Result<()> {
    read_number(reader, fields, "payload.token", "uint32", 32, 1)?;
    read_number(reader, fields, "payload.checksum", "uint23", 23, 1)?;
    let maximum = read_number(reader, fields, "payload.maximum_items", "uint6", 6, 1)?;
    fields
        .last_mut()
        .expect("the maximum field was just added")
        .value = (maximum + 1).to_string();
    read_number(reader, fields, "payload.stream_kind", "uint1", 1, 1)?;
    read_fourcc(reader, fields, "payload.channel", 1)?;
    read_fourcc(reader, fields, "payload.item_name", 1)?;
    read_fourcc(reader, fields, "payload.locale", 1)?;
    read_number(reader, fields, "payload.reference_time", "int32", 32, 1)?;
    read_number(reader, fields, "payload.direction", "bool", 1, 1)?;
    Ok(())
}

fn decode_chat_join_request(reader: &mut BitReader<'_>, fields: &mut Vec<Field>) -> Result<()> {
    let kind = read_number(reader, fields, "payload.kind", "choice", 2, 1)?;
    match kind {
        0 => "private",
        2 => "public",
        3 => "club",
        _ => "unknown",
    }
    .clone_into(
        &mut fields
            .last_mut()
            .expect("the kind field was just added")
            .value,
    );
    match kind {
        0 => {
            read_utf8(reader, fields, "payload.name", 7, 1)?;
            read_number(reader, fields, "payload.token", "uint32", 32, 1)?;
        }
        2 => {
            read_fourcc(reader, fields, "payload.locale", 1)?;
            read_number(reader, fields, "payload.channel_name_id", "uint16", 16, 1)?;
            read_number(reader, fields, "payload.token", "uint32", 32, 1)?;
        }
        3 => {
            read_number(reader, fields, "payload.namespace", "uint16", 16, 1)?;
            read_number(reader, fields, "payload.club_id", "uint32", 32, 1)?;
            read_number(reader, fields, "payload.token", "uint32", 32, 1)?;
        }
        _ => {
            return Err(crate::Error::Native(
                "unknown outgoing chat join kind".to_owned(),
            ));
        }
    }
    Ok(())
}

fn decode_whisper_request(reader: &mut BitReader<'_>, fields: &mut Vec<Field>) -> Result<()> {
    let target = read_number(reader, fields, "payload.target.kind", "choice", 3, 1)?;
    match target {
        0 => {
            read_number(
                reader,
                fields,
                "payload.target.presence_id",
                "uint32",
                32,
                2,
            )?;
        }
        1 => {
            read_number(reader, fields, "payload.target.region", "uint8", 8, 2)?;
            read_fourcc(reader, fields, "payload.target.program_id", 2)?;
            read_number(reader, fields, "payload.target.realm", "uint32", 32, 2)?;
            let length = read_number(
                reader,
                fields,
                "payload.target.name.encoded_length",
                "uint7",
                7,
                3,
            )?;
            read_utf8_bytes(
                reader,
                fields,
                "payload.target.name",
                usize::try_from(length + 2).unwrap_or(usize::MAX),
                2,
            )?;
        }
        3 => {
            read_number(reader, fields, "payload.target.account_id", "uint32", 32, 2)?;
        }
        5 => {
            read_fourcc(reader, fields, "payload.target.program_id", 2)?;
            read_number(reader, fields, "payload.target.region", "uint8", 8, 2)?;
            read_number(reader, fields, "payload.target.realm", "uint32", 32, 2)?;
            read_number(reader, fields, "payload.target.id", "uint64", 64, 2)?;
        }
        _ => {
            return Err(crate::Error::Native(
                "unknown outgoing whisper target".to_owned(),
            ));
        }
    }
    read_utf8(reader, fields, "payload.body", 10, 1)?;
    Ok(())
}

fn read_number(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
    path: &str,
    kind: &'static str,
    width: usize,
    depth: usize,
) -> Result<u64> {
    let start = reader.position();
    let value = reader.read(width)?;
    fields.push(Field::leaf(
        path,
        kind,
        value.to_string(),
        start,
        reader.position(),
        depth,
        FieldRole::Payload,
    ));
    Ok(value)
}

fn read_fourcc(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
    path: &str,
    depth: usize,
) -> Result<u64> {
    let value = read_number(reader, fields, path, "fourcc", 32, depth)?;
    let bytes = u32::try_from(value)
        .expect("fourcc is 32 bits")
        .to_be_bytes();
    fields
        .last_mut()
        .expect("the fourcc field was just added")
        .value = String::from_utf8_lossy(&bytes).into_owned();
    Ok(value)
}

fn read_utf8(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
    path: &str,
    length_bits: usize,
    depth: usize,
) -> Result<String> {
    let start = reader.position();
    let length = usize::try_from(reader.read(length_bits)?).map_err(|_| {
        crate::Error::Native("outgoing string length exceeds platform limits".to_owned())
    })?;
    let length_end = reader.position();
    let value = read_utf8_bytes(reader, fields, path, length, depth)?;
    fields.push(Field::leaf(
        format!("{path}.length"),
        "length",
        length.to_string(),
        start,
        length_end,
        depth + 1,
        FieldRole::Payload,
    ));
    Ok(value)
}

fn read_utf8_bytes(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
    path: &str,
    length: usize,
    depth: usize,
) -> Result<String> {
    let start = reader.position();
    let bytes = reader.read_bytes(length, true)?;
    let value = String::from_utf8_lossy(&bytes).into_owned();
    fields.push(Field::leaf(
        path,
        "string",
        value.clone(),
        start,
        reader.position(),
        depth,
        FieldRole::Payload,
    ));
    Ok(value)
}

#[derive(Clone, Copy)]
struct DecodedRecord<'a> {
    direction: Direction,
    header: RoutingHeader,
    type_id: u32,
    payload: &'a Payload,
    provenance: &'a [DecodedField],
    bytes: &'a [u8],
    logical_bits: usize,
}

fn inspect_decoded(protocol: &Protocol, decoded: DecodedRecord<'_>) -> Option<Record> {
    let DecodedRecord {
        direction,
        header,
        type_id,
        payload,
        provenance,
        bytes,
        logical_bits,
    } = decoded;
    let service_slot = header.service_slot?;
    let type_name = protocol
        .codec()
        .schema()
        .type_metadata(type_id)
        .ok()
        .and_then(|metadata| metadata.name)
        .unwrap_or_else(|| format!("type #{type_id}"));
    let (service, command) = labels(&type_name);
    let route_end = header.bit_count;
    let mut fields = route_fields(header, service_slot);
    let traced_fields = append_provenance(&mut fields, provenance);
    if traced_fields == 0 {
        append_decoded_payload(&mut fields, payload, route_end, logical_bits);
    } else if traced_fields == 1 && payload.reflected().is_none() {
        fields.pop();
        append_decoded_payload(&mut fields, payload, route_end, logical_bits);
    }
    append_padding(&mut fields, logical_bits, bytes.len() * 8);
    Some(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction,
        service,
        command,
        type_name,
        service_slot,
        command_id: header.command_id,
        bytes: bytes.to_vec(),
        logical_bits,
        fields,
    })
}

fn route_fields(header: RoutingHeader, service_slot: u8) -> Vec<Field> {
    let route_end = header.bit_count;
    vec![
        Field::leaf(
            "route",
            "routing header",
            format!("slot {service_slot}, command {}", header.command_id),
            0,
            route_end,
            0,
            FieldRole::Route,
        ),
        Field::leaf(
            "route.command_id",
            "uint6",
            header.command_id.to_string(),
            0,
            6,
            1,
            FieldRole::Route,
        ),
        Field::leaf(
            "route.service_present",
            "bool",
            "true",
            6,
            7,
            1,
            FieldRole::Route,
        ),
        Field::leaf(
            "route.service_slot",
            "uint4",
            service_slot.to_string(),
            7,
            route_end,
            1,
            FieldRole::Route,
        ),
    ]
}

fn append_padding(fields: &mut Vec<Field>, logical_bits: usize, total_bits: usize) {
    if logical_bits < total_bits {
        fields.push(Field::leaf(
            "padding",
            "zero bits",
            "0",
            logical_bits,
            total_bits,
            0,
            FieldRole::Padding,
        ));
    }
}

fn append_provenance(fields: &mut Vec<Field>, provenance: &[DecodedField]) -> usize {
    fields.extend(provenance.iter().map(|field| {
        let path = field
            .path
            .strip_prefix("value")
            .map_or(field.path.clone(), |suffix| format!("payload{suffix}"));
        Field::leaf(
            path,
            field.kind,
            field.value.clone(),
            field.start_bit,
            field.end_bit,
            field.depth,
            FieldRole::Payload,
        )
    }));
    provenance.len()
}

fn labels(type_name: &str) -> (String, String) {
    let mut parts = type_name.rsplit("::");
    let command = parts.next().unwrap_or(type_name).to_owned();
    let service = parts.next().unwrap_or("Native").to_owned();
    (service, command)
}

fn payload_name(payload: &Payload) -> String {
    format!("{payload:?}")
        .split(['(', '{'])
        .next()
        .unwrap_or("Payload")
        .to_owned()
}

fn append_decoded_payload(
    fields: &mut Vec<Field>,
    payload: &Payload,
    start_bit: usize,
    end_bit: usize,
) {
    if let Some(value) = payload.reflected() {
        append_bsn_value(fields, "payload", value, start_bit, end_bit, 1);
        return;
    }
    let Ok(value) = serde_json::to_value(payload) else {
        fields.push(Field::leaf(
            "payload",
            "decoded payload",
            payload_name(payload),
            start_bit,
            end_bit,
            0,
            FieldRole::Payload,
        ));
        return;
    };
    let (kind, value) = unwrap_payload_variant(&value);
    append_json_value(fields, "payload", kind, value, start_bit, end_bit, 0, true);
}

#[allow(clippy::too_many_arguments)]
fn append_json_value(
    fields: &mut Vec<Field>,
    path: &str,
    kind: &str,
    value: &serde_json::Value,
    start_bit: usize,
    end_bit: usize,
    depth: usize,
    exact_range: bool,
) {
    let (node_kind, summary) = json_summary(kind, value);
    let mut field = Field::decoded(path, node_kind, summary, start_bit, end_bit, depth);
    field.exact_range = exact_range;
    fields.push(field);
    match value {
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                append_json_value(
                    fields,
                    &format!("{path}[{index}]"),
                    "item",
                    value,
                    start_bit,
                    end_bit,
                    depth + 1,
                    false,
                );
            }
        }
        serde_json::Value::Object(values) => {
            for (name, value) in values {
                append_json_value(
                    fields,
                    &format!("{path}.{name}"),
                    "field",
                    value,
                    start_bit,
                    end_bit,
                    depth + 1,
                    false,
                );
            }
        }
        _ => {}
    }
}

fn unwrap_payload_variant(value: &serde_json::Value) -> (&str, &serde_json::Value) {
    let serde_json::Value::Object(values) = value else {
        return ("payload", value);
    };
    if values.len() != 1 {
        return ("payload", value);
    }
    values
        .iter()
        .next()
        .map_or(("payload", value), |(kind, value)| (kind, value))
}

fn json_summary(declared_kind: &str, value: &serde_json::Value) -> (&'static str, String) {
    match value {
        serde_json::Value::Null => ("null", "null".to_owned()),
        serde_json::Value::Bool(value) => ("bool", value.to_string()),
        serde_json::Value::Number(value) => ("number", value.to_string()),
        serde_json::Value::String(value) => ("string", value.clone()),
        serde_json::Value::Array(values) => (
            "array",
            if values.len() == 1 {
                "1 item".to_owned()
            } else {
                format!("{} items", values.len())
            },
        ),
        serde_json::Value::Object(values) => (
            if declared_kind == "field" || declared_kind == "item" {
                "object"
            } else {
                "payload"
            },
            if declared_kind == "field" || declared_kind == "item" {
                format!("{} fields", values.len())
            } else {
                declared_kind.to_owned()
            },
        ),
    }
}

fn append_bsn_value(
    fields: &mut Vec<Field>,
    path: &str,
    value: &BsnValue,
    start_bit: usize,
    end_bit: usize,
    depth: usize,
) {
    match value {
        BsnValue::Array(values) => {
            fields.push(Field::decoded(
                path,
                "array",
                format!("{} items", values.len()),
                start_bit,
                end_bit,
                depth,
            ));
            for (index, value) in values.iter().enumerate() {
                append_bsn_value(
                    fields,
                    &format!("{path}[{index}]"),
                    value,
                    start_bit,
                    end_bit,
                    depth + 1,
                );
            }
        }
        BsnValue::Optional(value) => {
            fields.push(Field::decoded(
                path,
                "optional",
                if value.is_some() { "present" } else { "none" },
                start_bit,
                end_bit,
                depth,
            ));
            if let Some(value) = value {
                append_bsn_value(fields, path, value, start_bit, end_bit, depth + 1);
            }
        }
        BsnValue::Choice { index, value } => {
            fields.push(Field::decoded(
                path,
                "choice",
                format!("variant {index}"),
                start_bit,
                end_bit,
                depth,
            ));
            append_bsn_value(fields, path, value, start_bit, end_bit, depth + 1);
        }
        BsnValue::Struct(value) => {
            fields.push(Field::decoded(
                path,
                "struct",
                format!("type #{}", value.type_id),
                start_bit,
                end_bit,
                depth,
            ));
            for field in &value.fields {
                let name = field
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("field_{}", field.index));
                append_bsn_value(
                    fields,
                    &format!("{path}.{name}"),
                    &field.value,
                    start_bit,
                    end_bit,
                    depth + 1,
                );
            }
        }
        _ => {
            let (kind, decoded) = bsn_scalar(value);
            fields.push(Field::decoded(
                path, kind, decoded, start_bit, end_bit, depth,
            ));
        }
    }
}

fn bsn_scalar(value: &BsnValue) -> (&'static str, String) {
    match value {
        BsnValue::Void => ("void", "void".to_owned()),
        BsnValue::Bool(value) => ("bool", value.to_string()),
        BsnValue::Integer(value) => ("integer", value.to_string()),
        BsnValue::FourCc(value) => ("fourcc", format!("0x{value:08x}")),
        BsnValue::Float32(value) => ("float32", value.to_string()),
        BsnValue::Float64(value) => ("float64", value.to_string()),
        BsnValue::Bytes(value) => ("bytes", format!("{} bytes", value.len())),
        BsnValue::String(value) => ("string", value.clone()),
        BsnValue::BitArray(value) => ("bit array", format!("{} bits", value.bit_count)),
        BsnValue::Array(_)
        | BsnValue::Optional(_)
        | BsnValue::Choice { .. }
        | BsnValue::Struct(_) => unreachable!(),
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn sample_chat_join_bytes(name: &str, member: u32, channel: u8, token: u32) -> Vec<u8> {
    use crate::bsn::bits::BitWriter;

    let mut writer = BitWriter::new();
    writer.write(27, 6).expect("fixture command fits");
    writer.write(1, 1).expect("fixture service flag fits");
    writer
        .write(u64::from(CHAT_SLOT), 4)
        .expect("fixture service slot fits");
    writer.write(0, 1).expect("fixture success flag fits");
    writer
        .write(u64::from(member), 32)
        .expect("fixture member fits");
    writer
        .write(u64::from(channel), 3)
        .expect("fixture channel fits");
    writer.write(0x1111_1111, 32).expect("fixture id fits");
    writer.write(0x2222_2222, 32).expect("fixture owner fits");
    writer.write(1, 4).expect("fixture channel type fits");
    writer.write(1, 1).expect("fixture name flag fits");
    writer.write(0, 16).expect("fixture region fits");
    writer.write(0, 29).expect("fixture namespace fits");
    writer.write(0, 2).expect("fixture name kind fits");
    writer
        .write(name.len() as u64, 7)
        .expect("fixture name length fits");
    writer.align().expect("fixture alignment fits");
    writer
        .write_bytes(name.as_bytes(), false)
        .expect("fixture name bytes fit");
    writer.write(0, 1).expect("fixture config flag fits");
    writer.write(0, 1).expect("fixture reserved flag fits");
    writer.write(1, 1).expect("fixture token flag fits");
    writer
        .write(u64::from(token), 32)
        .expect("fixture token fits");
    writer.align().expect("fixture final alignment fits");
    writer.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outgoing_cache_requests_are_captured_with_exact_fields() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol
            .cache_get_stream_items(7, "BNET", "CONF", "enUS")
            .unwrap();
        let record = inspect_outgoing(&protocol, &bytes).unwrap();

        assert_eq!(record.direction, Direction::Outgoing);
        assert_eq!(record.service, "Cache");
        assert_eq!(record.command, "GetStreamItemsRequest");
        assert_eq!(record.logical_bits, 202);
        for (path, value) in [
            ("payload.token", "7"),
            ("payload.maximum_items", "1"),
            ("payload.channel", "BNET"),
            ("payload.item_name", "CONF"),
            ("payload.locale", "enUS"),
        ] {
            let field = record
                .fields
                .iter()
                .find(|field| field.path == path)
                .unwrap_or_else(|| panic!("missing {path}"));
            assert_eq!(field.value, value);
            assert!(field.exact_range);
            assert!(field.end_bit > field.start_bit);
        }
    }

    #[test]
    fn outgoing_chat_messages_expose_body_and_channel() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol.chat_message(3, "hello world").unwrap();
        let record = inspect_outgoing(&protocol, &bytes).unwrap();

        assert_eq!(record.direction, Direction::Outgoing);
        assert_eq!(record.service, "Chat");
        assert_eq!(record.command, "SendMessage");
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.body" && field.value == "hello world" && field.exact_range
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.channel_index" && field.value == "3" && field.exact_range
        }));
    }

    #[test]
    fn reflected_outgoing_requests_retain_schema_provenance() {
        let protocol = Protocol::current().unwrap();
        let bytes = protocol.ping(123_456).unwrap();
        let record = inspect_outgoing(&protocol, &bytes).unwrap();

        assert_eq!(record.direction, Direction::Outgoing);
        assert_eq!(record.service, "Connection");
        assert_eq!(record.command, "Ping");
        assert!(record.fields.iter().any(|field| {
            field.path.ends_with("m_timeData.value") && field.value == "123456" && field.exact_range
        }));
    }

    #[test]
    fn inspected_ranges_cover_the_real_chat_join_fixture() {
        let protocol = Protocol::current().unwrap();
        let bytes = sample_chat_join_bytes("General", 0x1020_3040, 2, 0x1122_3344);
        let record = inspect_chat_join(&protocol, &bytes).unwrap();
        let decoded = protocol_test_decode(&protocol, &bytes);

        assert!(decoded);
        assert!(record.logical_bits <= bytes.len() * 8);
        assert_eq!(record.fields[0].start_bit, 0);
        assert_eq!(record.fields[0].end_bit, 11);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.channel_name.literal"
                && field.value == "General"
                && field.start_bit < field.end_bit
        }));
        assert!(
            record
                .fields
                .iter()
                .all(|field| field.start_bit <= field.end_bit && field.end_bit <= bytes.len() * 8)
        );
        assert!(record.fields.iter().any(|field| {
            field.role == FieldRole::Padding
                && field.start_bit == record.logical_bits
                && field.end_bit == bytes.len() * 8
        }));

        let mut reader = BitReader::new(&bytes, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = u8::try_from(reader.read(4).unwrap()).unwrap();
        let header = RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        let decoded = protocol
            .decode_incoming_with_provenance_from(&mut reader, header)
            .unwrap();
        assert!(decoded.provenance.iter().any(|field| {
            field.path == "value.channel_name.literal"
                && field.value == "General"
                && field.start_bit < field.end_bit
        }));
    }

    #[test]
    fn custom_whisper_fields_keep_their_wire_ranges() {
        let protocol = Protocol::current().unwrap();
        let bytes =
            hex::decode("5305414a682e0000000019034e656c736f6e54657374393123313435380100686f6c61")
                .unwrap();
        let mut reader = BitReader::new(&bytes, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = u8::try_from(reader.read(4).unwrap()).unwrap();
        let header = RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        let decoded = protocol
            .decode_incoming_with_provenance_from(&mut reader, header)
            .unwrap();
        let logical_bits = reader.position();

        let record = inspect_decoded(
            &protocol,
            DecodedRecord {
                direction: Direction::Incoming,
                header,
                type_id: decoded.type_id,
                payload: &decoded.payload,
                provenance: &decoded.provenance,
                bytes: &bytes,
                logical_bits,
            },
        )
        .unwrap();

        let body = record
            .fields
            .iter()
            .find(|field| field.path == "payload.body")
            .expect("the decoded body must have a traced node");
        assert!(body.exact_range);
        assert_eq!(body.value, "hola");
        assert!(body.start_bit > header.bit_count);
        assert_eq!(body.end_bit, logical_bits);
        assert!(body.end_bit - body.start_bit < logical_bits - header.bit_count);
    }

    #[test]
    fn friend_toon_fields_keep_their_generated_wire_ranges() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "460301010014cc0200000011004563686f657323323935cafebabe7f1884100000000002fe223701",
        )
        .unwrap();
        let mut reader = BitReader::new(&bytes, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = u8::try_from(reader.read(4).unwrap()).unwrap();
        let header = RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        let decoded = protocol
            .decode_incoming_with_provenance_from(&mut reader, header)
            .unwrap();
        let record = inspect_decoded(
            &protocol,
            DecodedRecord {
                direction: Direction::Incoming,
                header,
                type_id: decoded.type_id,
                payload: &decoded.payload,
                provenance: &decoded.provenance,
                bytes: &bytes,
                logical_bits: reader.position(),
            },
        )
        .unwrap();

        for path in [
            "payload.entries[0].account_id",
            "payload.entries[0].program_id",
            "payload.entries[0].profile.label",
            "payload.entries[0].profile.id",
            "payload.entries[0].toon_name.region",
            "payload.entries[0].toon_name.program_id",
            "payload.entries[0].toon_name.realm",
            "payload.entries[0].toon_name.name",
            "payload.complete",
        ] {
            let field = record
                .fields
                .iter()
                .find(|field| field.path == path)
                .unwrap_or_else(|| panic!("missing {path}"));
            assert!(field.exact_range, "{path} must retain its wire range");
            assert!(field.end_bit > field.start_bit);
        }
        assert!(
            record
                .fields
                .iter()
                .filter(|field| field.role == FieldRole::Payload)
                .all(|field| field.exact_range)
        );
    }

    #[test]
    fn toon_list_wire_fields_reach_the_inspector_with_exact_ranges() {
        let protocol = Protocol::current().unwrap();
        let root_type = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Client::Toon::ToonList")
            .unwrap();
        let display_type = protocol
            .array_element(protocol.member_type(root_type, "m_toonDisplays").unwrap())
            .unwrap();
        let profile_type = protocol.member_type(display_type, "m_profile").unwrap();
        let realm_type = protocol.member_type(display_type, "m_realm").unwrap();
        let profile_type = protocol.peel_alias(profile_type).unwrap();
        let profile_shape = protocol.codec().schema().shape(profile_type).unwrap();
        let profile = BsnValue::Struct(crate::bsn::value::BsnStruct::new(
            profile_type,
            vec![
                crate::bsn::value::BsnField::named(
                    profile_shape.index_values[0],
                    "m_label",
                    BsnValue::Integer(9),
                ),
                crate::bsn::value::BsnField::named(
                    profile_shape.index_values[1],
                    "m_id",
                    BsnValue::Integer(10),
                ),
            ],
        ));
        let mut writer = crate::bsn::bits::BitWriter::new();
        writer
            .write(super::super::protocol::TOON_LIST_COMMAND.into(), 6)
            .unwrap();
        writer.write(1, 1).unwrap();
        writer
            .write(super::super::protocol::TOON_SLOT.into(), 4)
            .unwrap();
        writer.write(1, 6).unwrap();
        writer.write(3, 7).unwrap();
        writer.align().unwrap();
        writer.write_bytes(b"Nova!", false).unwrap();
        writer.write(0x8000_0000, 32).unwrap();
        writer.write(5, 3).unwrap();
        writer.write(7, 32).unwrap();
        protocol
            .codec()
            .encode_reflected_into(&mut writer, profile_type, &profile)
            .unwrap();
        protocol
            .codec()
            .encode_reflected_into(&mut writer, realm_type, &BsnValue::Integer(1))
            .unwrap();
        let logical_bits = writer.position();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes, Some(logical_bits)).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = u8::try_from(reader.read(4).unwrap()).unwrap();
        let header = RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        let decoded = protocol
            .decode_incoming_with_provenance_from(&mut reader, header)
            .unwrap();
        let record = inspect_decoded(
            &protocol,
            DecodedRecord {
                direction: Direction::Incoming,
                header,
                type_id: decoded.type_id,
                payload: &decoded.payload,
                provenance: &decoded.provenance,
                bytes: &bytes,
                logical_bits,
            },
        )
        .unwrap();

        for path in [
            "payload.displays.count",
            "payload.displays[0].name",
            "payload.displays[0].last_online",
            "payload.displays[0].wire_layout_selector",
            "payload.displays[0].flags",
            "payload.displays[0].profile.m_label",
            "payload.displays[0].profile.m_id",
            "payload.displays[0].realm",
        ] {
            let field = record
                .fields
                .iter()
                .find(|field| field.path == path)
                .unwrap_or_else(|| panic!("missing {path}"));
            assert!(field.exact_range, "{path}");
            assert!(field.end_bit > field.start_bit, "{path}");
        }
    }

    #[test]
    fn custom_payloads_form_objects_and_indexed_arrays() {
        let payload = Payload::ClubSummaries(vec![super::super::model::ClubSummary {
            club_id: 535_225,
            name: Some("cecw".to_owned()),
            kind: 1,
            category: 1,
            private: false,
        }]);
        let mut fields = Vec::new();

        append_decoded_payload(&mut fields, &payload, 11, 2304);

        assert_eq!(fields[0].path, "payload");
        assert_eq!(fields[0].kind, "array");
        assert_eq!(fields[0].value, "1 item");
        assert_eq!(fields[1].path, "payload[0]");
        assert_eq!(fields[1].kind, "object");
        assert!(fields.iter().any(|field| {
            field.path == "payload[0].name" && field.value == "cecw" && field.depth == 2
        }));
        assert!(fields.iter().all(|field| field.end_bit > field.start_bit));
        assert!(!fields.iter().any(|field| field.path.contains("item_")));
    }

    #[test]
    fn club_summary_children_keep_exact_wire_ranges() {
        const THREE_CLUB_REPLY: &str = concat!(
            "ee056356e55503000000010082ab09010463656377f72bfaeabe6f45645400000000000000",
            "000100014cb200000000d32963020000000000000000000000602b72aa130000000200415609",
            "010c546573742047726f75702041c12bfaeabe7b81645400000000000000010100014cb20000",
            "0080a1f320020000000000000000000000602b72aa130000000100415514000a4d6964696761",
            "74696f6eee2bfaea7ed34d645400000000000000010200014cb200000040054d4447544e4283",
            "5f0400000000000000000000000003321e32c884d01e000000004d011a00010c000000000920",
            "36e353af92ff53af97100000000c01000299120000090d05090100029912000006090509000000",
            "010000000142074151c9b9030000000002c537e5efc87774cf724ca67d01835cdcab380000000",
            "03400000001c8a9d7c91f0265000101000a6602f39e460c010001"
        );
        let bytes = hex::decode(THREE_CLUB_REPLY).unwrap();
        let protocol = Protocol::current().unwrap();
        let mut reader = BitReader::new(&bytes, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = u8::try_from(reader.read(4).unwrap()).unwrap();
        let header = RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        let decoded = protocol
            .decode_incoming_with_provenance_from(&mut reader, header)
            .unwrap();
        let logical_bits = reader.position();
        let record = inspect_decoded(
            &protocol,
            DecodedRecord {
                direction: Direction::Incoming,
                header,
                type_id: decoded.type_id,
                payload: &decoded.payload,
                provenance: &decoded.provenance,
                bytes: &bytes,
                logical_bits,
            },
        )
        .unwrap();

        for path in [
            "payload[0].club_id",
            "payload[0].name",
            "payload[0].kind",
            "payload[0].category",
            "payload[0].private",
        ] {
            let field = record
                .fields
                .iter()
                .find(|field| field.path == path)
                .unwrap_or_else(|| panic!("missing {path}"));
            assert!(field.exact_range, "{path} must retain its wire range");
            assert!(field.end_bit > field.start_bit);
        }
    }

    fn protocol_test_decode(protocol: &Protocol, bytes: &[u8]) -> bool {
        let mut reader = BitReader::new(bytes, None).unwrap();
        let command_id = u8::try_from(reader.read(6).unwrap()).unwrap();
        assert_eq!(reader.read(1).unwrap(), 1);
        let service_slot = u8::try_from(reader.read(4).unwrap()).unwrap();
        let header = crate::bsn::bits::RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        protocol.decode_incoming_from(&mut reader, header).is_ok()
    }
}
