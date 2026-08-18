use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    Result,
    bsn::bits::{BitReader, RoutingHeader},
    bsn::codec::DecodedField,
    bsn::value::BsnValue,
    wire::protobuf::RpcHeader,
};

use super::{
    Payload, Protocol,
    protocol::{
        ACHIEVEMENT_LISTEN_COMMAND, ACHIEVEMENT_SLOT, AUTH_GENERATE_WEB_TOKEN_COMMAND,
        AUTH_LOGON_COMMAND, AUTH_PROOF_COMMAND, AUTH_RESUME_COMMAND, AUTH_SINGLE_SIGN_ON_COMMAND,
        AUTHENTICATION_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND, CACHE_SLOT,
        CHAT_CHANNEL_LIST_REQUEST_COMMAND, CHAT_CREATE_AND_INVITE_COMMAND,
        CHAT_DATAGRAM_CONNECTION_UPDATE_COMMAND, CHAT_ENUM_CATEGORIES_COMMAND,
        CHAT_ENUM_CONFERENCES_COMMAND, CHAT_INVITE_ACCEPT_COMMAND, CHAT_INVITE_DECLINE_COMMAND,
        CHAT_JOIN_REQUEST_COMMAND, CHAT_LEAVE_REQUEST_COMMAND, CHAT_MESSAGE_COMMAND,
        CHAT_MODIFY_CHANNEL_LIST_COMMAND, CHAT_SLOT, CHAT_STATUS_CHANGE_COMMAND,
        CHAT_WHISPER_SEND_COMMAND, CONNECTION_ENABLE_ENCRYPTION_COMMAND, CONNECTION_LOGOUT_COMMAND,
        CONNECTION_MESSAGE_FRAME_COMMAND, CONNECTION_PING_COMMAND, CONNECTION_PONG_COMMAND,
        CONNECTION_SLOT, FRIENDS_SLOT, FRIENDS_TOONS_COMMAND, PARTY_BEGIN_READY_PROCESS_COMMAND,
        PARTY_MODIFY_MAP_OPTIONS_COMMAND, PARTY_MODIFY_NON_LOBBY_ATTRIBUTE_LIST_COMMAND,
        PARTY_READY_PROCESS_UPDATE_COMMAND, PARTY_SLOT, PRESENCE_SLOT,
        PRESENCE_STATISTICS_SUBSCRIBE_COMMAND, PRESENCE_TEMPORARY_COMMAND, PRESENCE_UPDATE_COMMAND,
        PROFILE_ADDRESS_QUERY_COMMAND, PROFILE_CHANGE_SETTINGS_COMMAND, PROFILE_READ_COMMAND,
        PROFILE_RESOLVE_TOON_HANDLE_REQUEST_COMMAND, PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND,
        PROFILE_SEND_STATS_UI_EVENTS_COMMAND, PROFILE_SLOT, S2_MAP_GAME_GROUP_SUBSCRIBE_COMMAND,
        S2_MAP_GAME_GROUP_UPDATE_COMMAND, S2_MAP_LIST_FAVORITES_COMMAND,
        S2_MASTER_CURRENT_SEASON_COMMAND, S2_MASTER_MMQ_GET_INFO_COMMAND,
        S2_MASTER_MMQ_GET_LIST_COMMAND, S2_MASTER_MMQ_SUBSCRIBE_COMMAND,
        S2_MASTER_SITE_LATENCY_INFO_COMMAND, S2_MASTER_SLOT, S2_MULTIPLAYER_CLUB_SUBSCRIBE_COMMAND,
        S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND, S2_MULTIPLAYER_GET_TOON_CLUBS_COMMAND,
        S2_MULTIPLAYER_INVITE_ACTION_COMMAND, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND,
        S2_MULTIPLAYER_SLOT, TOON_BILLING_UPDATE_COMMAND, TOON_CAIS_TIME_UPDATE_COMMAND,
        TOON_CREATE_CANCEL_COMMAND, TOON_CREATE_FINAL_COMMAND, TOON_CREATE_INIT_COMMAND,
        TOON_SELECT_COMMAND, TOON_SLOT,
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct ReflectedOutgoingRecord {
    pub header: RoutingHeader,
    pub type_id: u32,
    pub value: BsnValue,
    pub logical_bits: usize,
    pub byte_count: usize,
}

#[derive(Default)]
struct CaptureState {
    next_sequence: u64,
    protocol: Option<Protocol>,
    paused: bool,
    records: VecDeque<RawRecord>,
    bgs_routes: HashMap<(Direction, u32), BgsRoute>,
}

#[derive(Clone)]
struct RawRecord {
    sequence: u64,
    captured_at_millis: u64,
    message: RawMessage,
}

#[derive(Clone)]
enum RawMessage {
    Native(RawNativeRecord),
    Bgs(RawBgsRecord),
    Http(RawHttpRecord),
}

#[derive(Clone)]
struct RawNativeRecord {
    direction: Direction,
    header: RoutingHeader,
    bytes: Vec<u8>,
    logical_bits: usize,
}

#[derive(Clone, Copy)]
struct BgsRoute {
    service_hash: u32,
    method_id: u32,
}

#[derive(Clone)]
struct RawBgsRecord {
    direction: Direction,
    header: Option<RpcHeader>,
    route: Option<BgsRoute>,
    bytes: Vec<u8>,
    error: Option<String>,
}

#[derive(Clone)]
struct RawHttpRecord {
    direction: Direction,
    method: String,
    url: String,
    status: Option<u16>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
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
        records: records
            .iter()
            .filter_map(|raw| {
                let mut record = match &raw.message {
                    RawMessage::Native(native) => {
                        let protocol = protocol.as_ref()?;
                        match native.direction {
                            Direction::Incoming => {
                                let mut reader = BitReader::new(&native.bytes, None).ok()?;
                                reader.set_position(native.header.bit_count).ok()?;
                                let decoded = protocol
                                    .decode_incoming_with_provenance_from(
                                        &mut reader,
                                        native.header,
                                    )
                                    .ok()?;
                                inspect_decoded(
                                    protocol,
                                    DecodedRecord {
                                        direction: Direction::Incoming,
                                        header: native.header,
                                        type_id: decoded.type_id,
                                        payload: &decoded.payload,
                                        provenance: &decoded.provenance,
                                        bytes: &native.bytes,
                                        logical_bits: native.logical_bits,
                                    },
                                )?
                            }
                            Direction::Outgoing => {
                                inspect_outgoing(protocol, &native.bytes).ok()?
                            }
                        }
                    }
                    RawMessage::Bgs(bgs) => inspect_bgs(bgs),
                    RawMessage::Http(http) => inspect_http(http),
                };
                record.sequence = raw.sequence;
                record.captured_at_millis = raw.captured_at_millis;
                Some(record)
            })
            .collect(),
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
    let mut state = capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.records.clear();
    state.bgs_routes.clear();
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
    push_record(
        &mut state,
        RawMessage::Native(RawNativeRecord {
            direction,
            header,
            bytes: bytes.to_vec(),
            logical_bits,
        }),
    );
}

pub(crate) fn capture_bgs(direction: Direction, header: &RpcHeader, body: &[u8], bytes: &[u8]) {
    let mut state = capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.paused {
        return;
    }
    let response = header.service_id == 0xfe || header.is_response == Some(true);
    let direct_route =
        header
            .service_hash
            .zip(header.method_id)
            .map(|(service_hash, method_id)| BgsRoute {
                service_hash,
                method_id,
            });
    let route = if response {
        state
            .bgs_routes
            .remove(&(direction, header.token))
            .or(direct_route)
    } else {
        if let Some(route) = direct_route {
            let response_direction = match direction {
                Direction::Incoming => Direction::Outgoing,
                Direction::Outgoing => Direction::Incoming,
            };
            state
                .bgs_routes
                .insert((response_direction, header.token), route);
        }
        direct_route
    };
    let expected_size = header.size.map_or(body.len(), |size| size as usize);
    let error = (expected_size != body.len()).then(|| {
        format!(
            "header declares {expected_size} body bytes, received {}",
            body.len()
        )
    });
    push_record(
        &mut state,
        RawMessage::Bgs(RawBgsRecord {
            direction,
            header: Some(header.clone()),
            route,
            bytes: bytes.to_vec(),
            error,
        }),
    );
}

pub(crate) fn capture_invalid_bgs(direction: Direction, bytes: &[u8], error: &str) {
    let mut state = capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.paused {
        return;
    }
    push_record(
        &mut state,
        RawMessage::Bgs(RawBgsRecord {
            direction,
            header: None,
            route: None,
            bytes: bytes.to_vec(),
            error: Some(error.to_owned()),
        }),
    );
}

pub fn capture_http_request(method: &str, url: &str, headers: &[(String, String)], body: &[u8]) {
    capture_http(RawHttpRecord {
        direction: Direction::Outgoing,
        method: method.to_owned(),
        url: url.to_owned(),
        status: None,
        headers: sanitized_headers(headers),
        body: body.to_vec(),
    });
}

pub fn capture_http_response(
    method: &str,
    url: &str,
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) {
    capture_http(RawHttpRecord {
        direction: Direction::Incoming,
        method: method.to_owned(),
        url: url.to_owned(),
        status: Some(status),
        headers: sanitized_headers(headers),
        body: body.to_vec(),
    });
}

fn capture_http(http: RawHttpRecord) {
    let mut state = capture_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.paused {
        return;
    }
    push_record(&mut state, RawMessage::Http(http));
}

fn sanitized_headers(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let sensitive = [
                "authorization",
                "cookie",
                "proxy-authorization",
                "set-cookie",
                "x-api-key",
            ]
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate));
            (
                name.clone(),
                if sensitive {
                    "<redacted>".to_owned()
                } else {
                    value.clone()
                },
            )
        })
        .collect()
}

fn push_record(state: &mut CaptureState, message: RawMessage) {
    let sequence = state.next_sequence;
    state.next_sequence = state.next_sequence.wrapping_add(1);
    if state.records.len() == CAPTURE_LIMIT {
        state.records.pop_front();
    }
    state.records.push_back(RawRecord {
        sequence,
        captured_at_millis: now_millis(),
        message,
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

pub fn read_routing_header(bytes: &[u8]) -> Result<(RoutingHeader, BitReader<'_>)> {
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

pub fn decode_reflected_outgoing(
    protocol: &Protocol,
    bytes: &[u8],
) -> Result<ReflectedOutgoingRecord> {
    let (header, mut reader) = read_routing_header(bytes)?;
    let service_slot = header.service_slot.ok_or_else(|| {
        crate::Error::Native("native client record has no service slot".to_owned())
    })?;
    let type_name = reflected_outgoing_type((service_slot, header.command_id)).ok_or(
        crate::Error::UnmappedNativeRoute {
            slot: service_slot,
            command: header.command_id,
        },
    )?;
    let type_id = protocol.codec().schema().unique_type_id(type_name)?;
    let value = protocol.codec().decode_from(&mut reader, type_id)?;
    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    Ok(ReflectedOutgoingRecord {
        header,
        type_id,
        value,
        logical_bits,
        byte_count,
    })
}

pub fn inspect_native_record(
    protocol: &Protocol,
    direction: Direction,
    bytes: &[u8],
) -> Result<Record> {
    match direction {
        Direction::Incoming => inspect_incoming_strict(protocol, bytes),
        Direction::Outgoing => inspect_outgoing_strict(protocol, bytes),
    }
}

fn inspect_incoming_strict(protocol: &Protocol, bytes: &[u8]) -> Result<Record> {
    let (header, mut reader) = read_routing_header(bytes)?;
    let Some(service_slot) = header.service_slot else {
        return inspect_command_response(header, &mut reader, Direction::Incoming, bytes);
    };
    if (service_slot, header.command_id) == (S2_MASTER_SLOT, S2_MASTER_MMQ_GET_INFO_COMMAND) {
        return inspect_mmq_get_info_response(header, &mut reader, bytes);
    }
    if (service_slot, header.command_id) == (S2_MULTIPLAYER_SLOT, S2_MAP_GAME_GROUP_UPDATE_COMMAND)
    {
        return inspect_game_group_update(protocol, header, &mut reader, bytes);
    }
    let decoded = protocol.decode_incoming_with_provenance_from(&mut reader, header)?;
    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    inspect_decoded(
        protocol,
        DecodedRecord {
            direction: Direction::Incoming,
            header,
            type_id: decoded.type_id,
            payload: &decoded.payload,
            provenance: &decoded.provenance,
            bytes: &bytes[..byte_count],
            logical_bits,
        },
    )
    .ok_or_else(|| {
        crate::Error::Native(format!(
            "native incoming record has no service slot after decoding slot {service_slot}"
        ))
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "the fields stay in physical wire order for protocol inspection"
)]
fn inspect_mmq_get_info_response(
    header: RoutingHeader,
    reader: &mut BitReader<'_>,
    bytes: &[u8],
) -> Result<Record> {
    const TYPE_NAME: &str = "Battlenet::Client::S2Master::MMQGetInfoResponse";

    let mut fields = route_fields(header, S2_MASTER_SLOT);
    let payload_start = reader.position();
    let payload_index = fields.len();
    fields.push(Field::leaf(
        "payload",
        "incoming payload",
        "MMQGetInfoResponse",
        payload_start,
        payload_start,
        0,
        FieldRole::Payload,
    ));
    read_number(reader, &mut fields, "payload.generated_flag", "bool", 1, 1)?;
    read_number(reader, &mut fields, "payload.token", "uint32", 32, 1)?;
    let present = read_number(
        reader,
        &mut fields,
        "payload.queue_info.present",
        "optional flag",
        1,
        1,
    )?;
    if present != 0 {
        let alignment_start = reader.position();
        let alignment = reader.align()?;
        if alignment != 0 {
            fields.push(Field::leaf(
                "payload.queue_info.alignment",
                "padding",
                format!("{alignment} ignored bits"),
                alignment_start,
                reader.position(),
                2,
                FieldRole::Padding,
            ));
        }
        let data_start = reader.position();
        let data = reader.read_bytes(40, false)?;
        fields.push(Field::leaf(
            "payload.queue_info.per_game_data",
            "generated byte block",
            byte_summary(&data),
            data_start,
            reader.position(),
            2,
            FieldRole::Payload,
        ));
        read_number(
            reader,
            &mut fields,
            "payload.queue_info.static_info.id",
            "uint32",
            32,
            2,
        )?;
        read_number(
            reader,
            &mut fields,
            "payload.queue_info.static_info.flags",
            "uint64",
            64,
            2,
        )?;
        read_number(
            reader,
            &mut fields,
            "payload.queue_info.handle.region",
            "uint8",
            8,
            2,
        )?;
        read_fourcc(
            reader,
            &mut fields,
            "payload.queue_info.handle.program_id",
            2,
        )?;
        read_number(
            reader,
            &mut fields,
            "payload.queue_info.handle.id",
            "uint32",
            32,
            2,
        )?;
        read_number(
            reader,
            &mut fields,
            "payload.queue_info.handle.version",
            "uint16",
            16,
            2,
        )?;
    }

    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    fields[payload_index].end_bit = logical_bits;
    append_padding(&mut fields, logical_bits, byte_count * 8);
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: Direction::Incoming,
        service: "S2Master".to_owned(),
        command: "MMQGetInfoResponse".to_owned(),
        type_name: TYPE_NAME.to_owned(),
        service_slot: S2_MASTER_SLOT,
        command_id: header.command_id,
        bytes: bytes[..byte_count].to_vec(),
        logical_bits,
        fields,
    })
}

fn inspect_game_group_update(
    protocol: &Protocol,
    header: RoutingHeader,
    reader: &mut BitReader<'_>,
    bytes: &[u8],
) -> Result<Record> {
    const TYPE_NAME: &str = "Battlenet::Client::S2Map::GameGroupUpdate";
    const RESULTS_TYPE_NAME: &str = "Battlenet::Client::S2Map::GameGroupUpdate::Results";

    let mut fields = route_fields(header, S2_MULTIPLAYER_SLOT);
    let payload_start = reader.position();
    let payload_index = fields.len();
    fields.push(Field::leaf(
        "payload",
        "incoming payload",
        "GameGroupUpdate",
        payload_start,
        payload_start,
        0,
        FieldRole::Payload,
    ));
    let prefix = read_number(
        reader,
        &mut fields,
        "payload.generated_prefix",
        "opaque uint5",
        5,
        1,
    )?;
    if prefix != 0 {
        return Err(crate::Error::Native(format!(
            "game-group update has unknown generated prefix {prefix:#x}"
        )));
    }
    read_number(reader, &mut fields, "payload.is_last", "bool", 1, 1)?;
    let marker = read_number(
        reader,
        &mut fields,
        "payload.generated_marker",
        "opaque uint28",
        28,
        1,
    )?;
    if marker != 0x00d5_cc00 {
        return Err(crate::Error::Native(format!(
            "game-group update has unknown generated marker {marker:#x}"
        )));
    }
    read_fourcc(reader, &mut fields, "payload.tag", 1)?;
    read_number(reader, &mut fields, "payload.is_first", "bool", 1, 1)?;

    let results_type = protocol
        .codec()
        .schema()
        .unique_type_id(RESULTS_TYPE_NAME)?;
    let results = protocol.codec().decode_traced_from(reader, results_type)?;
    append_provenance(&mut fields, &results.fields);

    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    fields[payload_index].end_bit = logical_bits;
    append_padding(&mut fields, logical_bits, byte_count * 8);
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: Direction::Incoming,
        service: "S2Map".to_owned(),
        command: "GameGroupUpdate".to_owned(),
        type_name: TYPE_NAME.to_owned(),
        service_slot: S2_MULTIPLAYER_SLOT,
        command_id: header.command_id,
        bytes: bytes[..byte_count].to_vec(),
        logical_bits,
        fields,
    })
}

fn inspect_outgoing_strict(protocol: &Protocol, bytes: &[u8]) -> Result<Record> {
    let (header, mut reader) = read_routing_header(bytes)?;
    let Some(service_slot) = header.service_slot else {
        return inspect_command_response(header, &mut reader, Direction::Outgoing, bytes);
    };
    let route = (service_slot, header.command_id);
    if route == (CHAT_SLOT, CHAT_MODIFY_CHANNEL_LIST_COMMAND) {
        return inspect_modify_channel_list(protocol, header, &mut reader, bytes);
    }
    if route == (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND) {
        let decoded = protocol.decode_incoming_with_provenance_from(&mut reader, header)?;
        let logical_bits = reader.position();
        let byte_count = checked_record_byte_count(bytes, logical_bits)?;
        return inspect_decoded(
            protocol,
            DecodedRecord {
                direction: Direction::Outgoing,
                header,
                type_id: decoded.type_id,
                payload: &decoded.payload,
                provenance: &decoded.provenance,
                bytes: &bytes[..byte_count],
                logical_bits,
            },
        )
        .ok_or_else(|| crate::Error::Native("outgoing record has no service slot".to_owned()));
    }
    if let Some(type_name) = reflected_outgoing_type(route)
        && route != (CONNECTION_SLOT, CONNECTION_ENABLE_ENCRYPTION_COMMAND)
    {
        let type_id = protocol.codec().schema().unique_type_id(type_name)?;
        let decoded = protocol.codec().decode_traced_from(&mut reader, type_id)?;
        let payload = Payload::Reflected(decoded.value);
        let logical_bits = reader.position();
        let byte_count = checked_record_byte_count(bytes, logical_bits)?;
        return inspect_decoded(
            protocol,
            DecodedRecord {
                direction: Direction::Outgoing,
                header,
                type_id,
                payload: &payload,
                provenance: &decoded.fields,
                bytes: &bytes[..byte_count],
                logical_bits,
            },
        )
        .ok_or_else(|| crate::Error::Native("outgoing record has no service slot".to_owned()));
    }
    let type_name = manual_outgoing_type(route).ok_or(crate::Error::UnmappedNativeRoute {
        slot: service_slot,
        command: header.command_id,
    })?;
    let (service, command) = labels(type_name);
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
    decode_manual_outgoing(protocol, route, &mut reader, &mut fields)?;
    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    fields[payload_index].end_bit = logical_bits;
    append_padding(&mut fields, logical_bits, byte_count * 8);
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: Direction::Outgoing,
        service,
        command,
        type_name: type_name.to_owned(),
        service_slot,
        command_id: header.command_id,
        bytes: bytes[..byte_count].to_vec(),
        logical_bits,
        fields,
    })
}

fn inspect_modify_channel_list(
    protocol: &Protocol,
    header: RoutingHeader,
    reader: &mut BitReader<'_>,
    bytes: &[u8],
) -> Result<Record> {
    let mut discriminator = reader.clone();
    discriminator.read(32)?;
    discriminator.read(1)?;
    let first_alignment = discriminator.read(4)?;
    discriminator.read(5)?;
    let second_alignment = discriminator.read(3)?;
    let type_name = if first_alignment == 0 && second_alignment == 0 {
        "Battlenet::Client::Chat::ModifyChannelListRequest"
    } else {
        "Battlenet::Client::Chat::ModifyChannelListRequest2"
    };
    let (service, command) = labels(type_name);
    let mut fields = route_fields(header, CHAT_SLOT);
    let type_id = protocol.codec().schema().unique_type_id(type_name)?;
    let decoded = protocol.codec().decode_traced_from(reader, type_id)?;
    append_provenance(&mut fields, &decoded.fields);

    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    append_padding(&mut fields, logical_bits, byte_count * 8);
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: Direction::Outgoing,
        service,
        command,
        type_name: type_name.to_owned(),
        service_slot: CHAT_SLOT,
        command_id: header.command_id,
        bytes: bytes[..byte_count].to_vec(),
        logical_bits,
        fields,
    })
}

fn inspect_command_response(
    header: RoutingHeader,
    reader: &mut BitReader<'_>,
    direction: Direction,
    bytes: &[u8],
) -> Result<Record> {
    let result = u16::try_from(reader.read(9)?).expect("nine bits fit in u16");
    let result_description = if result == 0 {
        "success".to_owned()
    } else {
        super::errors::description(result)
    };
    let logical_bits = reader.position();
    let byte_count = checked_record_byte_count(bytes, logical_bits)?;
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction,
        service: "Command".to_owned(),
        command: format!("Response {}", header.command_id),
        type_name: "Battlenet::Client::CommandResponse".to_owned(),
        service_slot: u8::MAX,
        command_id: header.command_id,
        bytes: bytes[..byte_count].to_vec(),
        logical_bits,
        fields: vec![
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
                "payload.result",
                "Battle.net error",
                result_description,
                header.bit_count,
                logical_bits,
                1,
                FieldRole::Payload,
            ),
        ],
    })
}

fn checked_record_byte_count(bytes: &[u8], logical_bits: usize) -> Result<usize> {
    let byte_count = logical_bits.div_ceil(8);
    if byte_count > bytes.len() {
        return Err(crate::Error::IncompleteFrame(format!(
            "decoded record needs {byte_count} bytes, only {} are buffered",
            bytes.len()
        )));
    }
    Ok(byte_count)
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
    let decoded = decode_manual_outgoing(protocol, route, &mut reader, &mut fields);
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

fn inspect_bgs(raw: &RawBgsRecord) -> Record {
    let total_bits = raw.bytes.len() * 8;
    let header_length = raw.bytes.get(..2).map_or(0, |bytes| {
        usize::from(u16::from_be_bytes([bytes[0], bytes[1]]))
    });
    let header_start = 2.min(raw.bytes.len());
    let header_end = header_start
        .saturating_add(header_length)
        .min(raw.bytes.len());
    let (service_name, full_service_name) = raw.route.map_or(("Unknown", None), |route| {
        bgs_service(route.service_hash)
            .map_or(("Unknown", None), |(short, full)| (short, Some(full)))
    });
    let method_id = raw.route.map(|route| route.method_id);
    let method_name = method_id.map_or_else(
        || "Message".to_owned(),
        |method_id| bgs_method(service_name, method_id),
    );
    let response = raw
        .header
        .as_ref()
        .is_some_and(|header| header.service_id == 0xfe || header.is_response == Some(true));
    let command = if raw.header.is_none() {
        "Malformed message".to_owned()
    } else if response {
        format!("{method_name} response")
    } else {
        method_name.clone()
    };
    let type_name = full_service_name.map_or_else(
        || {
            raw.header.as_ref().map_or_else(
                || "BGS malformed WebSocket message".to_owned(),
                |header| format!("BGS RPC token {}", header.token),
            )
        },
        |service| format!("{service}::{method_name}"),
    );
    let fields = bgs_fields(raw, header_start, header_end, header_length);
    Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: raw.direction,
        service: format!("BGS · {service_name}"),
        command,
        type_name,
        service_slot: u8::MAX,
        command_id: method_id
            .and_then(|id| u8::try_from(id).ok())
            .unwrap_or(u8::MAX),
        bytes: raw.bytes.clone(),
        logical_bits: total_bits,
        fields,
    }
}

fn bgs_fields(
    raw: &RawBgsRecord,
    header_start: usize,
    header_end: usize,
    header_length: usize,
) -> Vec<Field> {
    let total_bits = raw.bytes.len() * 8;
    let body_start = header_end;
    let mut fields = vec![Field::leaf(
        "message",
        "BGS RPC message",
        format!("{} bytes", raw.bytes.len()),
        0,
        total_bits,
        0,
        FieldRole::Control,
    )];
    if raw.bytes.len() >= 2 {
        fields.push(Field::leaf(
            "message.header_length",
            "uint16 big-endian",
            header_length.to_string(),
            0,
            16,
            1,
            FieldRole::Control,
        ));
    }
    if header_end > header_start {
        fields.push(Field::leaf(
            "header",
            "protobuf message",
            format!("{header_length} bytes"),
            header_start * 8,
            header_end * 8,
            0,
            FieldRole::Control,
        ));
        append_protobuf_fields(
            &mut fields,
            "header",
            &raw.bytes[header_start..header_end],
            header_start,
            1,
            FieldRole::Control,
            Some(bgs_header_field_name),
        );
    }
    if body_start < raw.bytes.len() {
        fields.push(Field::leaf(
            "body",
            "protobuf message",
            format!("{} bytes", raw.bytes.len() - body_start),
            body_start * 8,
            total_bits,
            0,
            FieldRole::Payload,
        ));
        append_protobuf_fields(
            &mut fields,
            "body",
            &raw.bytes[body_start..],
            body_start,
            1,
            FieldRole::Payload,
            None,
        );
    }
    if let Some(error) = &raw.error {
        let mut field = Field::decoded("error", "decode error", error, 0, total_bits, 0);
        field.role = FieldRole::Control;
        fields.push(field);
    }
    fields
}

fn inspect_http(raw: &RawHttpRecord) -> Record {
    let (bytes, mut fields) = http_wire_image(raw);
    let total_bits = bytes.len() * 8;
    fields.insert(
        0,
        Field::leaf(
            "message",
            if raw.status.is_some() {
                "HTTP response"
            } else {
                "HTTP request"
            },
            format!("{} bytes", bytes.len()),
            0,
            total_bits,
            0,
            FieldRole::Control,
        ),
    );
    let target = http_target(&raw.url);
    let command = raw.status.map_or_else(
        || format!("{} {target}", raw.method),
        |status| {
            format!(
                "{status} {} · {} {target}",
                http_status_reason(status),
                raw.method
            )
        },
    );
    Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        direction: raw.direction,
        service: "HTTP".to_owned(),
        command,
        type_name: raw.url.clone(),
        service_slot: u8::MAX,
        command_id: u8::MAX,
        bytes,
        logical_bits: total_bits,
        fields,
    }
}

fn http_wire_image(raw: &RawHttpRecord) -> (Vec<u8>, Vec<Field>) {
    let mut bytes = Vec::new();
    let mut fields = Vec::new();
    append_http_start_line(raw, &mut bytes, &mut fields);
    let headers = http_headers_with_defaults(raw);
    append_http_headers(&headers, &mut bytes, &mut fields);
    append_http_body(&headers, &raw.body, &mut bytes, &mut fields);
    (bytes, fields)
}

fn append_http_start_line(raw: &RawHttpRecord, bytes: &mut Vec<u8>, fields: &mut Vec<Field>) {
    if let Some(status) = raw.status {
        append_http_token(
            bytes,
            fields,
            "response.version",
            "HTTP version",
            "HTTP/1.1",
            FieldRole::Control,
        );
        bytes.push(b' ');
        append_http_token(
            bytes,
            fields,
            "response.status",
            "status code",
            &status.to_string(),
            FieldRole::Control,
        );
        bytes.push(b' ');
        append_http_token(
            bytes,
            fields,
            "response.reason",
            "reason phrase",
            http_status_reason(status),
            FieldRole::Control,
        );
    } else {
        append_http_token(
            bytes,
            fields,
            "request.method",
            "HTTP method",
            &raw.method,
            FieldRole::Control,
        );
        bytes.push(b' ');
        append_http_token(
            bytes,
            fields,
            "request.target",
            "request target",
            &http_target(&raw.url),
            FieldRole::Control,
        );
        bytes.push(b' ');
        append_http_token(
            bytes,
            fields,
            "request.version",
            "HTTP version",
            "HTTP/1.1",
            FieldRole::Control,
        );
    }
    bytes.extend_from_slice(b"\r\n");
}

fn http_headers_with_defaults(raw: &RawHttpRecord) -> Vec<(String, String)> {
    let mut headers = raw.headers.clone();
    if raw.status.is_none()
        && !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("host"))
        && let Some(host) = http_host(&raw.url)
    {
        headers.insert(0, ("Host".to_owned(), host));
    }
    if !raw.body.is_empty()
        && !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    {
        headers.push(("Content-Length".to_owned(), raw.body.len().to_string()));
    }
    headers
}

fn append_http_headers(headers: &[(String, String)], bytes: &mut Vec<u8>, fields: &mut Vec<Field>) {
    let header_start = bytes.len();
    let root_index = fields.len();
    for (index, (name, value)) in headers.iter().enumerate() {
        let line_start = bytes.len();
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(b": ");
        let value_start = bytes.len();
        bytes.extend_from_slice(value.as_bytes());
        let line_end = bytes.len();
        bytes.extend_from_slice(b"\r\n");
        let path = format!("headers[{index}]");
        fields.push(Field::leaf(
            path.clone(),
            "HTTP header",
            name,
            line_start * 8,
            line_end * 8,
            1,
            FieldRole::Control,
        ));
        fields.push(Field::leaf(
            format!("{path}.value"),
            "header value",
            value,
            value_start * 8,
            line_end * 8,
            2,
            FieldRole::Control,
        ));
    }
    let header_end = bytes.len();
    if header_end > header_start {
        fields.insert(
            root_index,
            Field::leaf(
                "headers",
                "HTTP headers",
                format!("{} fields", headers.len()),
                header_start * 8,
                header_end * 8,
                0,
                FieldRole::Control,
            ),
        );
    }
    bytes.extend_from_slice(b"\r\n");
}

fn append_http_body(
    headers: &[(String, String)],
    body: &[u8],
    bytes: &mut Vec<u8>,
    fields: &mut Vec<Field>,
) {
    let body_start = bytes.len();
    bytes.extend_from_slice(body);
    if !body.is_empty() {
        fields.push(Field::leaf(
            "body",
            http_body_kind(headers, body),
            byte_summary(body),
            body_start * 8,
            bytes.len() * 8,
            0,
            FieldRole::Payload,
        ));
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
            append_json_value(
                fields,
                "body.json",
                "json",
                &json,
                body_start * 8,
                bytes.len() * 8,
                1,
                false,
            );
        }
    }
}

fn append_http_token(
    bytes: &mut Vec<u8>,
    fields: &mut Vec<Field>,
    path: &str,
    kind: &'static str,
    value: &str,
    role: FieldRole,
) {
    let start = bytes.len();
    bytes.extend_from_slice(value.as_bytes());
    fields.push(Field::leaf(
        path,
        kind,
        value,
        start * 8,
        bytes.len() * 8,
        0,
        role,
    ));
}

fn http_target(url: &str) -> String {
    let Ok(url) = url::Url::parse(url) else {
        return url.to_owned();
    };
    let mut target = url.path().to_owned();
    if target.is_empty() {
        target.push('/');
    }
    if let Some(query) = url.query() {
        target.push('?');
        target.push_str(query);
    }
    target
}

fn http_host(url: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    let host = url.host_str()?;
    Some(
        url.port()
            .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}")),
    )
}

fn http_status_reason(status: u16) -> &'static str {
    match status {
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Content Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Response",
    }
}

fn http_body_kind(headers: &[(String, String)], body: &[u8]) -> &'static str {
    let content_type = headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then_some(value.as_str())
    });
    if content_type.is_some_and(|value| value.contains("json"))
        || serde_json::from_slice::<serde_json::Value>(body).is_ok()
    {
        "JSON body"
    } else if content_type.is_some_and(|value| value.starts_with("text/"))
        || std::str::from_utf8(body).is_ok()
    {
        "text body"
    } else {
        "binary body"
    }
}

fn append_protobuf_fields(
    fields: &mut Vec<Field>,
    prefix: &str,
    bytes: &[u8],
    byte_offset: usize,
    depth: usize,
    role: FieldRole,
    names: Option<fn(u32) -> Option<&'static str>>,
) {
    let mut cursor = 0;
    let mut occurrences = HashMap::<u32, usize>::new();
    while cursor < bytes.len() {
        let start = cursor;
        let Some(key) = read_protobuf_varint(bytes, &mut cursor) else {
            break;
        };
        let number = u32::try_from(key >> 3).unwrap_or(u32::MAX);
        let wire_type = u8::try_from(key & 7).expect("protobuf wire type fits in u8");
        if number == 0 {
            break;
        }
        let Some((value_start, value, kind)) = decode_protobuf_value(wire_type, bytes, &mut cursor)
        else {
            break;
        };
        let occurrence = occurrences.entry(number).or_default();
        let name = names
            .and_then(|names| names(number))
            .map_or_else(|| format!("field_{number}"), str::to_owned);
        let path = if *occurrence == 0 {
            format!("{prefix}.{name}")
        } else {
            format!("{prefix}.{name}[{occurrence}]")
        };
        *occurrence += 1;
        fields.push(Field::leaf(
            path.clone(),
            kind,
            value.clone(),
            (byte_offset + start) * 8,
            (byte_offset + cursor) * 8,
            depth,
            role,
        ));
        if wire_type == 2 && value_start < cursor {
            fields.push(Field::leaf(
                format!("{path}.value"),
                if std::str::from_utf8(&bytes[value_start..cursor]).is_ok() {
                    "string contents"
                } else {
                    "byte contents"
                },
                value,
                (byte_offset + value_start) * 8,
                (byte_offset + cursor) * 8,
                depth + 1,
                role,
            ));
        }
    }
}

fn decode_protobuf_value(
    wire_type: u8,
    bytes: &[u8],
    cursor: &mut usize,
) -> Option<(usize, String, &'static str)> {
    match wire_type {
        0 => {
            let value_start = *cursor;
            let value = read_protobuf_varint(bytes, cursor)?;
            Some((value_start, value.to_string(), "protobuf varint"))
        }
        1 => {
            let value_start = *cursor;
            let raw = bytes.get(*cursor..cursor.saturating_add(8))?;
            *cursor += 8;
            Some((
                value_start,
                format!("0x{:016x}", u64::from_le_bytes(raw.try_into().ok()?)),
                "protobuf fixed64",
            ))
        }
        2 => {
            let length = usize::try_from(read_protobuf_varint(bytes, cursor)?).ok()?;
            let value_start = *cursor;
            let raw = bytes.get(*cursor..cursor.saturating_add(length))?;
            *cursor += length;
            let kind = if std::str::from_utf8(raw).is_ok() {
                "protobuf string"
            } else {
                "protobuf bytes"
            };
            Some((value_start, byte_summary(raw), kind))
        }
        5 => {
            let value_start = *cursor;
            let raw = bytes.get(*cursor..cursor.saturating_add(4))?;
            *cursor += 4;
            Some((
                value_start,
                format!("0x{:08x}", u32::from_le_bytes(raw.try_into().ok()?)),
                "protobuf fixed32",
            ))
        }
        _ => None,
    }
}

fn read_protobuf_varint(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        if shift == 63 && byte > 1 {
            return None;
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn byte_summary(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes)
        && text
            .chars()
            .all(|character| !character.is_control() || character.is_ascii_whitespace())
    {
        let mut summary = text.chars().take(160).collect::<String>();
        if text.chars().count() > 160 {
            summary.push('…');
        }
        return summary;
    }
    let shown = bytes.len().min(32);
    let suffix = if shown < bytes.len() { "…" } else { "" };
    format!(
        "{} bytes · {}{suffix}",
        bytes.len(),
        hex::encode(&bytes[..shown])
    )
}

fn bgs_header_field_name(number: u32) -> Option<&'static str> {
    Some(match number {
        1 => "service_id",
        2 => "method_id",
        3 => "token",
        4 => "object_id",
        5 => "size",
        6 => "status",
        7 => "error",
        8 => "timeout",
        9 => "is_response",
        10 => "forward_targets",
        11 => "service_hash",
        13 => "client_id",
        14 => "fanout_target",
        15 => "client_id_fanout_target",
        16 => "client_record",
        _ => return None,
    })
}

fn bgs_service(hash: u32) -> Option<(&'static str, &'static str)> {
    const SERVICES: [(&str, &str); 9] = [
        ("Connection", "bnet.protocol.connection.ConnectionService"),
        (
            "Authentication",
            "bnet.protocol.authentication.AuthenticationServer",
        ),
        (
            "Authentication Client",
            "bnet.protocol.authentication.AuthenticationClient",
        ),
        ("Challenge", "bnet.protocol.challenge.ChallengeNotify"),
        (
            "Game Utilities",
            "bnet.protocol.game_utilities.GameUtilities",
        ),
        ("Account", "bnet.protocol.account.AccountService"),
        ("Session", "bnet.protocol.session.SessionService"),
        ("Presence", "bnet.protocol.presence.PresenceService"),
        ("Channel", "bnet.protocol.channel.ChannelService"),
    ];
    SERVICES
        .iter()
        .copied()
        .find(|(_, name)| bgs_service_hash(name) == hash)
}

fn bgs_method(service: &str, method: u32) -> String {
    let name = match (service, method) {
        ("Connection", 1) => "Connect",
        ("Connection", 2) => "Bind",
        ("Connection", 3) => "Echo",
        ("Connection", 4) => "ForceDisconnect",
        ("Connection", 5) => "KeepAlive",
        ("Connection", 6) => "Encrypt",
        ("Connection", 7) => "RequestDisconnect",
        ("Authentication", 1) => "Logon",
        ("Authentication", 2) => "ModuleNotify",
        ("Authentication", 3) => "ModuleMessage",
        ("Authentication", 4) => "SelectGameAccountDeprecated",
        ("Authentication", 5) => "GenerateSsoToken",
        ("Authentication", 6) => "SelectGameAccount",
        ("Authentication", 7) => "VerifyWebCredentials",
        ("Authentication", 8) => "GenerateWebCredentials",
        ("Authentication Client", 1) => "OnModuleLoad",
        ("Authentication Client", 2) => "OnModuleMessage",
        ("Authentication Client", 4) => "OnServerStateChange",
        ("Authentication Client", 5) => "OnLogonComplete",
        ("Authentication Client", 6) => "OnMemModuleLoad",
        ("Authentication Client", 10) => "OnLogonUpdate",
        ("Authentication Client", 11) => "OnVersionInfoUpdated",
        ("Authentication Client", 12) => "OnLogonQueueUpdate",
        ("Authentication Client", 13) => "OnLogonQueueEnd",
        ("Authentication Client", 14) => "OnGameAccountSelected",
        ("Challenge", 3) => "OnExternalChallenge",
        ("Challenge", 4) => "OnExternalChallengeResult",
        ("Game Utilities", 1) => "ProcessClientRequest",
        ("Game Utilities", 2) => "PresenceChannelCreated",
        ("Game Utilities", 6) => "ProcessServerRequest",
        ("Game Utilities", 7) => "OnGameAccountOnline",
        ("Game Utilities", 8) => "OnGameAccountOffline",
        ("Game Utilities", 10) => "GetAllValuesForAttribute",
        ("Game Utilities", 11) => "RegisterUtilities",
        ("Game Utilities", 12) => "UnregisterUtilities",
        _ => return format!("Method {method}"),
    };
    name.to_owned()
}

fn bgs_service_hash(name: &str) -> u32 {
    name.as_bytes().iter().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

fn reflected_outgoing_type(route: (u8, u8)) -> Option<&'static str> {
    Some(match route {
        (ACHIEVEMENT_SLOT, ACHIEVEMENT_LISTEN_COMMAND) => {
            "Battlenet::Client::Achievement::ListenRequest"
        }
        (AUTHENTICATION_SLOT, AUTH_LOGON_COMMAND) => {
            "Battlenet::Client::Authentication::LogonRequest3"
        }
        (AUTHENTICATION_SLOT, AUTH_RESUME_COMMAND) => {
            "Battlenet::Client::Authentication::ResumeRequest"
        }
        (AUTHENTICATION_SLOT, AUTH_PROOF_COMMAND) => {
            "Battlenet::Client::Authentication::ProofResponse"
        }
        (AUTHENTICATION_SLOT, AUTH_GENERATE_WEB_TOKEN_COMMAND) => {
            "Battlenet::Client::Authentication::GenerateWebTokenRequest"
        }
        (AUTHENTICATION_SLOT, AUTH_SINGLE_SIGN_ON_COMMAND) => {
            "Battlenet::Client::Authentication::SingleSignOnRequest3"
        }
        (CONNECTION_SLOT, CONNECTION_ENABLE_ENCRYPTION_COMMAND) => {
            "Battlenet::Client::Connection::EnableEncryption"
        }
        (CONNECTION_SLOT, CONNECTION_LOGOUT_COMMAND) => {
            "Battlenet::Client::Connection::LogoutRequest"
        }
        (CONNECTION_SLOT, CONNECTION_PING_COMMAND) => "Battlenet::Client::Connection::Ping",
        (CONNECTION_SLOT, CONNECTION_PONG_COMMAND) => "Battlenet::Client::Connection::Pong",
        (CONNECTION_SLOT, CONNECTION_MESSAGE_FRAME_COMMAND) => {
            "Battlenet::Client::Connection::MessageFrame"
        }
        (CHAT_SLOT, CHAT_STATUS_CHANGE_COMMAND) => "Battlenet::Client::Chat::StatusChangeRequest",
        (CHAT_SLOT, CHAT_CREATE_AND_INVITE_COMMAND) => {
            "Battlenet::Client::Chat::CreateAndInviteRequest"
        }
        (CHAT_SLOT, CHAT_DATAGRAM_CONNECTION_UPDATE_COMMAND) => {
            "Battlenet::Client::Chat::DatagramConnectionUpdate"
        }
        (FRIENDS_SLOT, FRIENDS_TOONS_COMMAND) => {
            "Battlenet::Client::Friends::ToonsOfFriendsRequest"
        }
        (PRESENCE_SLOT, PRESENCE_TEMPORARY_COMMAND) => {
            "Battlenet::Client::Presence::TemporaryPresenceRequest"
        }
        (PARTY_SLOT, PARTY_MODIFY_MAP_OPTIONS_COMMAND) => {
            "Battlenet::Client::Party::ModifyMapOptions"
        }
        (PARTY_SLOT, PARTY_BEGIN_READY_PROCESS_COMMAND) => {
            "Battlenet::Client::Party::BeginReadyProcess"
        }
        (PARTY_SLOT, PARTY_READY_PROCESS_UPDATE_COMMAND) => {
            "Battlenet::Client::Party::ReadyProcessUpdate"
        }
        (PARTY_SLOT, PARTY_MODIFY_NON_LOBBY_ATTRIBUTE_LIST_COMMAND) => {
            "Battlenet::Client::Party::ModifyNonLobbyAttributeList"
        }
        (PROFILE_SLOT, PROFILE_ADDRESS_QUERY_COMMAND) => {
            "Battlenet::Client::Profile::AddressQueryRequest"
        }
        (PROFILE_SLOT, PROFILE_RESOLVE_TOON_HANDLE_REQUEST_COMMAND) => {
            "Battlenet::Client::Profile::ResolveToonHandleToNameRequest"
        }
        (PROFILE_SLOT, PROFILE_RESOLVE_TOON_NAME_REQUEST_COMMAND) => {
            "Battlenet::Client::Profile::ResolveToonNameToHandleRequest"
        }
        (PROFILE_SLOT, PROFILE_SEND_STATS_UI_EVENTS_COMMAND) => {
            "Battlenet::Client::Profile::SendStatsUIEvent"
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_SEARCH_CLUBS_COMMAND) => {
            "Battlenet::Client::Club::SearchClubsRequest"
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_GET_CLUB_INFO_COMMAND) => {
            "Battlenet::Client::Club::GetClubInfoRequest"
        }
        (S2_MULTIPLAYER_SLOT, S2_MULTIPLAYER_CLUB_SUBSCRIBE_COMMAND) => {
            "Battlenet::Client::Club::ClubSubscribeRequest"
        }
        (S2_MASTER_SLOT, S2_MASTER_MMQ_GET_INFO_COMMAND) => {
            "Battlenet::Client::S2Master::MMQGetInfoRequest"
        }
        (S2_MASTER_SLOT, S2_MASTER_MMQ_SUBSCRIBE_COMMAND) => {
            "Battlenet::Client::S2Master::MMQSubscribe"
        }
        (S2_MULTIPLAYER_SLOT, S2_MAP_LIST_FAVORITES_COMMAND) => {
            "Battlenet::Client::S2Map::S2ListMapFavoritesRequest"
        }
        (TOON_SLOT, TOON_BILLING_UPDATE_COMMAND) => "Battlenet::Client::Toon::BillingUpdateNotify",
        (TOON_SLOT, TOON_CAIS_TIME_UPDATE_COMMAND) => "Battlenet::Client::Toon::CaisTimeUpdate",
        (TOON_SLOT, TOON_CREATE_INIT_COMMAND) => "Battlenet::Client::Toon::ToonCreateInit",
        (TOON_SLOT, TOON_CREATE_FINAL_COMMAND) => "Battlenet::Client::Toon::ToonCreateFinal",
        (TOON_SLOT, TOON_CREATE_CANCEL_COMMAND) => "Battlenet::Client::Toon::ToonCreateCancel",
        _ => return None,
    })
}

fn manual_outgoing_type(route: (u8, u8)) -> Option<&'static str> {
    Some(match route {
        (CONNECTION_SLOT, CONNECTION_ENABLE_ENCRYPTION_COMMAND) => {
            "Battlenet::Client::Connection::EnableEncryption"
        }
        (S2_MASTER_SLOT, S2_MASTER_SITE_LATENCY_INFO_COMMAND) => {
            "Battlenet::Client::S2Master::SiteLatencyInfo"
        }
        (S2_MASTER_SLOT, S2_MASTER_MMQ_GET_LIST_COMMAND) => {
            "Battlenet::Client::S2Master::MMQGetListRequest"
        }
        (S2_MASTER_SLOT, S2_MASTER_CURRENT_SEASON_COMMAND) => {
            "Battlenet::Client::S2Master::CurrentSeason"
        }
        (S2_MULTIPLAYER_SLOT, S2_MAP_GAME_GROUP_SUBSCRIBE_COMMAND) => {
            "Battlenet::Client::S2Map::GameGroupSubscribeRequest"
        }
        (PROFILE_SLOT, PROFILE_CHANGE_SETTINGS_COMMAND) => {
            "Battlenet::Client::Profile::ChangeSettings"
        }
        (PRESENCE_SLOT, PRESENCE_STATISTICS_SUBSCRIBE_COMMAND) => {
            "Battlenet::Client::Presence::StatisticsSubscribe"
        }
        (PRESENCE_SLOT, PRESENCE_UPDATE_COMMAND) => "Battlenet::Client::Presence::UpdateRequest",
        (CACHE_SLOT, CACHE_GET_STREAM_ITEMS_COMMAND) => {
            "Battlenet::Client::Cache::GetStreamItemsRequest"
        }
        (CHAT_SLOT, CHAT_JOIN_REQUEST_COMMAND) => "Battlenet::Client::Chat::JoinRequest",
        (CHAT_SLOT, CHAT_CHANNEL_LIST_REQUEST_COMMAND) => {
            "Battlenet::Client::Chat::ChannelListRequest"
        }
        (CHAT_SLOT, CHAT_ENUM_CONFERENCES_COMMAND) => {
            "Battlenet::Client::Chat::EnumConferenceDescriptions"
        }
        (CHAT_SLOT, CHAT_ENUM_CATEGORIES_COMMAND) => {
            "Battlenet::Client::Chat::EnumCategoryDescriptions"
        }
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
        S2_MASTER_SLOT => "S2Master",
        S2_MULTIPLAYER_SLOT => "Club",
        PROFILE_SLOT => "Profile",
        TOON_SLOT => "Toon",
        _ => "Native",
    }
}

fn decode_manual_outgoing(
    protocol: &Protocol,
    route: (u8, u8),
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
) -> Result<()> {
    match route {
        (CONNECTION_SLOT, CONNECTION_ENABLE_ENCRYPTION_COMMAND)
        | (S2_MASTER_SLOT, S2_MASTER_CURRENT_SEASON_COMMAND)
        | (
            CHAT_SLOT,
            CHAT_CHANNEL_LIST_REQUEST_COMMAND
            | CHAT_ENUM_CATEGORIES_COMMAND
            | CHAT_ENUM_CONFERENCES_COMMAND,
        ) => Ok(()),
        (S2_MASTER_SLOT, S2_MASTER_SITE_LATENCY_INFO_COMMAND) => {
            decode_site_latency_info(reader, fields)
        }
        (S2_MASTER_SLOT, S2_MASTER_MMQ_GET_LIST_COMMAND) => {
            decode_mmq_get_list_request(reader, fields)
        }
        (S2_MULTIPLAYER_SLOT, S2_MAP_GAME_GROUP_SUBSCRIBE_COMMAND) => {
            read_fourcc(reader, fields, "payload.tag", 1)?;
            read_number(reader, fields, "payload.type", "bool", 1, 1)?;
            Ok(())
        }
        (PROFILE_SLOT, PROFILE_CHANGE_SETTINGS_COMMAND) => {
            decode_profile_change_settings(reader, fields)
        }
        (PRESENCE_SLOT, PRESENCE_STATISTICS_SUBSCRIBE_COMMAND) => {
            read_number(reader, fields, "payload.on", "bool", 1, 1)?;
            Ok(())
        }
        (PRESENCE_SLOT, PRESENCE_UPDATE_COMMAND) => decode_presence_update_request(reader, fields),
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
        (PROFILE_SLOT, PROFILE_READ_COMMAND) => {
            decode_profile_read_request(protocol, reader, fields)
        }
        _ => Err(crate::Error::Native(
            "outgoing route has no structured decoder".to_owned(),
        )),
    }
}

fn decode_site_latency_info(reader: &mut BitReader<'_>, fields: &mut Vec<Field>) -> Result<()> {
    let count = read_number(reader, fields, "payload.sites.count", "uint7", 7, 1)?;
    if count > 64 {
        return Err(crate::Error::Native(format!(
            "site latency count {count} exceeds the protocol maximum"
        )));
    }
    for index in 0..count {
        let base = format!("payload.sites[{index}]");
        let length = read_number(
            reader,
            fields,
            &format!("{base}.name.length"),
            "uint6",
            6,
            3,
        )?;
        if length > 32 {
            return Err(crate::Error::Native(format!(
                "site name length {length} exceeds the protocol maximum"
            )));
        }
        let padding = reader.position().wrapping_neg() & 7;
        if padding != 0 {
            let start = reader.position();
            let value = reader.read(padding)?;
            if value != 0 {
                return Err(crate::Error::Native(
                    "site name has non-zero alignment padding".to_owned(),
                ));
            }
            fields.push(Field::leaf(
                format!("{base}.name.alignment"),
                "padding",
                format!("{padding} zero bits"),
                start,
                reader.position(),
                3,
                FieldRole::Padding,
            ));
        }
        let start = reader.position();
        let bytes = reader.read_bytes(
            usize::try_from(length).map_err(|_| {
                crate::Error::Native("site name length exceeds platform limits".to_owned())
            })?,
            false,
        )?;
        let name = String::from_utf8(bytes)
            .map_err(|_| crate::Error::Native("site name is not valid UTF-8".to_owned()))?;
        fields.push(Field::leaf(
            format!("{base}.name"),
            "string",
            name,
            start,
            reader.position(),
            2,
            FieldRole::Payload,
        ));
        read_number(
            reader,
            fields,
            &format!("{base}.wire_tag"),
            "opaque uint3",
            3,
            2,
        )?;
        read_number(
            reader,
            fields,
            &format!("{base}.latency_ms"),
            "uint32",
            32,
            2,
        )?;
        read_number(reader, fields, &format!("{base}.preferred"), "bool", 1, 2)?;
    }
    Ok(())
}

fn decode_mmq_get_list_request(reader: &mut BitReader<'_>, fields: &mut Vec<Field>) -> Result<()> {
    let prefix = read_number(
        reader,
        fields,
        "payload.filter.generated_prefix",
        "opaque uint20",
        20,
        2,
    )?;
    if prefix != 0x7be40 {
        return Err(crate::Error::Native(format!(
            "MMQ get-list filter has unknown generated prefix {prefix:#x}"
        )));
    }
    let count = read_number(reader, fields, "payload.filter.tags.count", "uint5", 5, 2)?;
    if count > 16 {
        return Err(crate::Error::Native(format!(
            "MMQ get-list tag count {count} exceeds the protocol maximum"
        )));
    }
    for index in 0..count {
        read_fourcc(reader, fields, &format!("payload.filter.tags[{index}]"), 3)?;
    }
    let suffix = read_number(
        reader,
        fields,
        "payload.filter.generated_suffix",
        "opaque bool",
        1,
        2,
    )?;
    if suffix != 0 {
        return Err(crate::Error::Native(
            "MMQ get-list filter has unknown generated suffix".to_owned(),
        ));
    }
    read_fourcc(reader, fields, "payload.program_id", 1)?;
    read_number(reader, fields, "payload.token", "uint32", 32, 1)?;
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "the wire choice is clearer as one exhaustive decoder"
)]
fn decode_presence_update_request(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
) -> Result<()> {
    let selector = read_number(reader, fields, "payload.value.kind", "choice", 5, 1)?;
    match selector {
        0 => {}
        1 | 2 => {
            read_number(reader, fields, "payload.value.data", "uint8", 8, 2)?;
        }
        3 | 4 => {
            read_number(reader, fields, "payload.value.data", "uint16", 16, 2)?;
        }
        5 | 6 | 9 => {
            read_number(reader, fields, "payload.value.data", "uint32", 32, 2)?;
        }
        7 | 8 | 10 => {
            read_number(reader, fields, "payload.value.data", "uint64", 64, 2)?;
        }
        11 => {
            read_number(reader, fields, "payload.value.data", "bool", 1, 2)?;
        }
        12 => {
            read_fourcc(reader, fields, "payload.value.data", 2)?;
        }
        13 => {
            read_utf8(reader, fields, "payload.value.data", 7, 2)?;
        }
        15 => {
            read_number(
                reader,
                fields,
                "payload.value.image_table_entry",
                "opaque uint32",
                32,
                2,
            )?;
        }
        17 => {
            read_number(reader, fields, "payload.value.region", "uint8", 8, 2)?;
            read_fourcc(reader, fields, "payload.value.program_id", 2)?;
            read_number(reader, fields, "payload.value.realm", "uint32", 32, 2)?;
            let encoded_length = read_number(
                reader,
                fields,
                "payload.value.name.encoded_length",
                "uint5",
                5,
                3,
            )?;
            read_utf8_bytes(
                reader,
                fields,
                "payload.value.name",
                usize::try_from(encoded_length + 2).unwrap_or(usize::MAX),
                2,
            )?;
        }
        19 => {
            read_number(reader, fields, "payload.value.label", "uint32", 32, 2)?;
            read_number(reader, fields, "payload.value.id", "uint64", 64, 2)?;
        }
        20 => {
            read_number(reader, fields, "payload.value.id", "uint32", 32, 2)?;
            read_number(reader, fields, "payload.value.version", "uint16", 16, 2)?;
        }
        24 => {
            read_number(
                reader,
                fields,
                "payload.value.toon_handle",
                "opaque uint64",
                64,
                2,
            )?;
            read_number(
                reader,
                fields,
                "payload.value.toon_handle_continued",
                "opaque uint64",
                64,
                2,
            )?;
            read_number(
                reader,
                fields,
                "payload.value.toon_handle_tail",
                "opaque uint8",
                8,
                2,
            )?;
        }
        25 => {
            read_number(reader, fields, "payload.value.region", "uint8", 8, 2)?;
            read_fourcc(reader, fields, "payload.value.program_id", 2)?;
            read_number(reader, fields, "payload.value.id", "uint32", 32, 2)?;
        }
        26 => {
            read_number(reader, fields, "payload.value.account_id", "uint32", 32, 2)?;
        }
        27 => {
            read_number(
                reader,
                fields,
                "payload.value.achievement",
                "opaque uint64",
                64,
                2,
            )?;
            read_number(
                reader,
                fields,
                "payload.value.achievement_continued",
                "opaque uint64",
                64,
                2,
            )?;
        }
        28 => {
            read_number(reader, fields, "payload.value.region", "uint8", 8, 2)?;
            read_fourcc(reader, fields, "payload.value.program_id", 2)?;
            read_number(reader, fields, "payload.value.id", "uint32", 32, 2)?;
            read_number(reader, fields, "payload.value.version", "uint16", 16, 2)?;
        }
        29 => {
            read_utf8(reader, fields, "payload.value.nickname", 5, 2)?;
        }
        _ => {
            return Err(crate::Error::Native(format!(
                "presence update value kind {selector} has no verified decoder"
            )));
        }
    }
    read_number(reader, fields, "payload.handle", "uint32", 32, 1)?;
    read_number(
        reader,
        fields,
        "payload.generated_checksum",
        "opaque uint9",
        9,
        1,
    )?;
    read_number(reader, fields, "payload.presence_id", "uint32", 32, 1)?;
    Ok(())
}

fn decode_profile_read_request(
    protocol: &Protocol,
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
) -> Result<()> {
    read_number(reader, fields, "payload.client_hash", "uint32", 32, 1)?;
    read_number(reader, fields, "payload.request_id", "uint32", 32, 1)?;
    read_number(reader, fields, "payload.address.label", "uint32", 32, 2)?;
    read_number(reader, fields, "payload.address.id", "uint64", 64, 2)?;
    read_number(
        reader,
        fields,
        "payload.specification.generated_reserved",
        "reserved bit",
        1,
        2,
    )?;
    let selector = read_number(
        reader,
        fields,
        "payload.specification.selection",
        "uint3 choice",
        3,
        2,
    )?;
    let reader_present = read_number(
        reader,
        fields,
        "payload.specification.reader.present",
        "bool",
        1,
        2,
    )?;
    match selector {
        0 => {
            let path_length = read_number(
                reader,
                fields,
                "payload.specification.path.length",
                "uint8",
                8,
                3,
            )?;
            read_hex_bytes(
                reader,
                fields,
                "payload.specification.path",
                usize::try_from(path_length).unwrap_or(usize::MAX),
                2,
            )?;
        }
        1 => {
            let type_id = protocol
                .codec()
                .schema()
                .unique_type_id("Battlenet::Profile::ReadSelection::Slice")?;
            let decoded = protocol.codec().decode_reflected_traced_from(
                reader,
                type_id,
                "payload.specification.selection.slice",
                3,
            )?;
            append_provenance(fields, &decoded.fields);
        }
        3 => {
            let count = read_number(
                reader,
                fields,
                "payload.specification.generated_path.count",
                "uint8",
                8,
                3,
            )?;
            if count > 32 {
                return Err(crate::Error::Native(format!(
                    "profile generated path contains {count} segments"
                )));
            }
            for index in 0..count {
                let length_path = format!("payload.specification.generated_path[{index}].length");
                let length = read_number(reader, fields, &length_path, "uint8", 8, 4)?;
                let value_path = format!("payload.specification.generated_path[{index}]");
                read_hex_bytes(
                    reader,
                    fields,
                    &value_path,
                    usize::try_from(length).unwrap_or(usize::MAX),
                    3,
                )?;
            }
        }
        _ => {
            return Err(crate::Error::Native(format!(
                "unsupported profile read selection {selector}"
            )));
        }
    }
    if reader_present != 0 {
        let type_id = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Profile::ReaderList")?;
        let decoded = protocol.codec().decode_reflected_traced_from(
            reader,
            type_id,
            "payload.specification.reader",
            3,
        )?;
        append_provenance(fields, &decoded.fields);
    }
    Ok(())
}

fn decode_profile_change_settings(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
) -> Result<()> {
    let count = read_number(
        reader,
        fields,
        "payload.settings.string_settings.count",
        "uint7",
        7,
        2,
    )?;
    if count > 100 {
        return Err(crate::Error::Native(format!(
            "profile settings count {count} exceeds the protocol maximum"
        )));
    }

    for index in 0..count {
        let base = format!("payload.settings.string_settings[{index}]");
        read_number(
            reader,
            fields,
            &format!("{base}.wire_tag"),
            "opaque uint29",
            29,
            3,
        )?;
        let length = read_number(
            reader,
            fields,
            &format!("{base}.value.length"),
            "uint10",
            10,
            4,
        )?;
        let padding = reader.position().wrapping_neg() & 7;
        if padding != 0 {
            let start = reader.position();
            let value = reader.read(padding)?;
            if value != 0 {
                return Err(crate::Error::Native(
                    "profile setting string has non-zero alignment padding".to_owned(),
                ));
            }
            fields.push(Field::leaf(
                format!("{base}.value.alignment"),
                "padding",
                format!("{padding} zero bits"),
                start,
                reader.position(),
                4,
                FieldRole::Padding,
            ));
        }
        let start = reader.position();
        let bytes = reader.read_bytes(
            usize::try_from(length).map_err(|_| {
                crate::Error::Native("profile setting length exceeds platform limits".to_owned())
            })?,
            false,
        )?;
        let value = String::from_utf8(bytes).map_err(|_| {
            crate::Error::Native("profile setting value is not valid UTF-8".to_owned())
        })?;
        fields.push(Field::leaf(
            format!("{base}.value"),
            "string",
            value,
            start,
            reader.position(),
            3,
            FieldRole::Payload,
        ));
        read_number(reader, fields, &format!("{base}.index"), "uint32", 32, 3)?;
    }

    let settings_type = read_number(reader, fields, "payload.settings_type", "enum2", 2, 1)?;
    if settings_type > 2 {
        return Err(crate::Error::Native(format!(
            "profile settings type {} is outside the protocol range",
            settings_type + 1
        )));
    }
    fields
        .last_mut()
        .expect("the settings type field was just added")
        .value = (settings_type + 1).to_string();
    Ok(())
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

fn read_hex_bytes(
    reader: &mut BitReader<'_>,
    fields: &mut Vec<Field>,
    path: &str,
    length: usize,
    depth: usize,
) -> Result<Vec<u8>> {
    let start = reader.position();
    let bytes = reader.read_bytes(length, true)?;
    fields.push(Field::leaf(
        path,
        "bytes",
        hex::encode(&bytes),
        start,
        reader.position(),
        depth,
        FieldRole::Payload,
    ));
    Ok(bytes)
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
    append_decode_audit(protocol, direction, header, type_id, route_end, &mut fields);
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

fn append_decode_audit(
    protocol: &Protocol,
    direction: Direction,
    header: RoutingHeader,
    type_id: u32,
    route_end: usize,
    fields: &mut Vec<Field>,
) {
    let support = protocol.codec().wire_layout_support(type_id).ok();
    if support == Some(crate::bsn::codec::WireLayoutSupport::Candidate) {
        fields.push(Field::leaf(
            "audit.wire_layout",
            "unverified wire layout",
            "the captured field order has not been verified with distinct values for every field",
            route_end,
            route_end,
            0,
            FieldRole::Control,
        ));
    }
    if header.service_slot == Some(AUTHENTICATION_SLOT)
        && header.command_id == AUTH_GENERATE_WEB_TOKEN_COMMAND
    {
        let detail = match direction {
            Direction::Outgoing => {
                "Authentication/16 matches the complete GenerateWebTokenRequest schema, but GenerateAppTokenRequest has the same wire shape"
            }
            Direction::Incoming => {
                "Authentication/16 matches the complete GenerateWebTokenResponse schema, but GenerateAppTokenResponse has the same wire shape"
            }
        };
        fields.push(Field::leaf(
            "audit.route_mapping",
            "candidate route identity",
            detail,
            route_end,
            route_end,
            0,
            FieldRole::Control,
        ));
    }
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
            "unused bits",
            "ignored",
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
        BsnValue::FourCc(value) => (
            "fourcc",
            String::from_utf8_lossy(&value.to_be_bytes()).into_owned(),
        ),
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
    fn chat_create_and_invite_uses_the_captured_generated_order() {
        let protocol = Protocol::current().unwrap();
        let bytes =
            hex::decode("4a150001930d0100014c32000000010b6e63617272696c6c6f2336373510000c9d03")
                .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Chat");
        assert_eq!(record.command, "CreateAndInviteRequest");
        assert_eq!(record.command_id, CHAT_CREATE_AND_INVITE_COMMAND);
        assert_eq!(record.bytes, bytes);
        assert_eq!(record.logical_bits, 268);
        assert!(
            record
                .fields
                .iter()
                .any(|field| { field.path.ends_with("m_name") && field.value == "ncarrillo#675" })
        );
        assert!(record.fields.iter().any(|field| {
            field.path == "audit.wire_layout" && field.kind == "unverified wire layout"
        }));
    }

    #[test]
    fn outgoing_chat_datagram_update_stops_before_the_command_response() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "4d0d0100014c32000000010b6e63617272696c6c6f2336373500000000000000e00000000000000001000000000101",
        )
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Chat");
        assert_eq!(record.command, "DatagramConnectionUpdate");
        assert_eq!(record.command_id, CHAT_DATAGRAM_CONNECTION_UPDATE_COMMAND);
        assert_eq!(record.bytes.len(), 45);
        assert_eq!(record.bytes, bytes[..45]);
    }

    #[test]
    fn outgoing_logout_request_is_an_empty_record() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("4601").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Connection");
        assert_eq!(record.command, "LogoutRequest");
        assert_eq!(record.command_id, CONNECTION_LOGOUT_COMMAND);
        assert_eq!(record.logical_bits, 11);
        assert_eq!(record.bytes, bytes);
    }

    #[test]
    fn party_non_lobby_attribute_list_uses_the_captured_prefix() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(concat!(
            "d10ca4001000000f2700002e3c00000003e7000013ec00000000f90300051400",
            "0000003e070000bc0b0000000f2700002f0501000003e700000bc600000000f9",
            "030002f3000000003e0700014b040000000f2700002f0800000003e700000bc9",
            "00000000f9030004e2000000003e07000138090000000f2700004e0a00000003",
            "e70000138b00000000f9030004e3000000003e070001380d00"
        ))
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Party");
        assert_eq!(record.command, "ModifyNonLobbyAttributeList");
        assert_eq!(
            record.command_id,
            PARTY_MODIFY_NON_LOBBY_ATTRIBUTE_LIST_COMMAND
        );
        assert_eq!(record.bytes, bytes);
        assert_eq!(record.logical_bits, 1222);
        assert!(
            record.fields.iter().any(|field| {
                field.path.ends_with("m_attrSelection") && field.value == "16 items"
            })
        );
    }

    #[test]
    fn begin_ready_process_is_bidirectional() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(concat!(
            "cc0401000299120000090205097332716800005553f9da4b8c897218058a871f",
            "4fbbf7f2dddbdc08fe0f00cf93a19072379ffb8d4bcafebabe7e5b880c000000",
            "000000"
        ))
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Party");
        assert_eq!(record.command, "BeginReadyProcess");
        assert_eq!(record.command_id, PARTY_BEGIN_READY_PROCESS_COMMAND);
        assert_eq!(record.bytes, bytes);
    }

    #[test]
    fn ready_process_update_is_bidirectional() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("ce040000").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Party");
        assert_eq!(record.command, "ReadyProcessUpdate");
        assert_eq!(record.command_id, PARTY_READY_PROCESS_UPDATE_COMMAND);
        assert_eq!(record.logical_bits, 28);
        assert_eq!(record.bytes, bytes);
        assert!(
            record
                .fields
                .iter()
                .any(|field| { field.path == "payload.m_memberHandle" && field.value == "none" })
        );
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_reason" && field.value == "ERROR_OK (Battle.net error 0)"
        }));
    }

    #[test]
    fn party_modify_map_options_uses_the_metadata_payload() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("d30c00000000").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Party");
        assert_eq!(record.command, "ModifyMapOptions");
        assert_eq!(record.command_id, PARTY_MODIFY_MAP_OPTIONS_COMMAND);
        assert_eq!(record.bytes, bytes);
        assert_eq!(record.logical_bits, 47);
    }

    #[test]
    fn profile_change_settings_uses_verified_generated_layout() {
        assert_eq!(
            manual_outgoing_type((PROFILE_SLOT, PROFILE_CHANGE_SETTINGS_COMMAND)),
            Some("Battlenet::Client::Profile::ChangeSettings")
        );

        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "c506013638001501426174746c6541757468656e7469636174696f6e2e4163636f756e74436f756e7472793d35353931383733771b72e001",
        )
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "Profile");
        assert_eq!(record.command_id, PROFILE_CHANGE_SETTINGS_COMMAND);
        assert_eq!(record.logical_bits, 442);
        assert_eq!(record.bytes.len(), 56);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.settings.string_settings[0].value"
                && field.value == "BattleAuthentication.AccountCountry=5591873"
        }));
    }

    #[test]
    fn profile_stats_ui_event_uses_the_complete_bzip_payload() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(concat!(
            "c8064301425a683131415926535959ff742200003c1f8058047df00260000a26",
            "a7dd0a20006a2091ea1934d0f420683108a7a329ea369000193464c84571e7b9",
            "71e9d016d1514db0d20ec1086d3447d91328b14b662239a41d5299cb3ee056fc",
            "280da213dd42584cd59550502c60b0de6df1ca8a21225605e0eb80a68fa2ab2f",
            "c5dc914e1424167fdd0880",
        ))
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Profile");
        assert_eq!(record.command_id, PROFILE_SEND_STATS_UI_EVENTS_COMMAND);
        assert_eq!(record.command, "SendStatsUIEvent");
        assert_eq!(record.logical_bits, bytes.len() * 8);
        assert_eq!(record.bytes, bytes);
        assert!(record.fields.iter().any(|field| {
            field.path.ends_with("m_events")
                && field.value == "135 bytes"
                && field.end_bit == record.logical_bits
        }));
    }

    #[test]
    fn club_subscribe_request_uses_the_generated_subscription_layout() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("ef054180000000000020b04f01").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Club");
        assert_eq!(record.command, "ClubSubscribeRequest");
        assert_eq!(record.command_id, S2_MULTIPLAYER_CLUB_SUBSCRIBE_COMMAND);
        assert_eq!(record.logical_bits, 97);
        assert_eq!(record.bytes, bytes);
        assert!(
            record.fields.iter().any(|field| {
                field.path == "payload.m_subscriptions" && field.value == "1 items"
            })
        );
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_subscriptions[0].m_stamp" && field.value == "0"
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_subscriptions[0].m_clubId" && field.value == "535567"
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_subscriptions[0].m_type" && field.value == "3"
        }));
        assert!(
            record
                .fields
                .iter()
                .all(|field| field.path != "audit.wire_layout")
        );
    }

    #[test]
    fn strict_outgoing_inspection_accepts_command_responses() {
        let protocol = Protocol::current().unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &[0, 0]).unwrap();
        assert_eq!(record.service, "Command");
        assert_eq!(record.command, "Response 0");
        assert_eq!(record.bytes, [0, 0]);
        assert_eq!(record.logical_bits, 16);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.result"
                && field.kind == "Battle.net error"
                && field.value == "success"
        }));
    }

    #[test]
    fn schema_error_codes_are_annotated_without_field_name_guesses() {
        let protocol = Protocol::current().unwrap();
        let type_id = protocol
            .codec()
            .schema()
            .unique_type_id("Battlenet::Error::Code")
            .unwrap();
        let encoded = protocol
            .codec()
            .encode(type_id, &BsnValue::Integer(402))
            .unwrap();
        let mut reader = BitReader::new(&encoded.data, Some(encoded.bit_count)).unwrap();
        let decoded = protocol
            .codec()
            .decode_traced_from(&mut reader, type_id)
            .unwrap();
        assert_eq!(decoded.value, BsnValue::Integer(402));
        assert!(decoded.fields.iter().any(|field| {
            field.kind == "Battle.net error" && field.value == "Battle.net error 402"
        }));
    }

    #[test]
    fn generic_cais_candidate_is_labeled_and_framed() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("d7070000000000000000").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Toon");
        assert_eq!(record.command_id, TOON_CAIS_TIME_UPDATE_COMMAND);
        assert_eq!(record.logical_bits, 75);
        assert_eq!(record.bytes.len(), 10);
        assert!(record.fields.iter().any(|field| {
            field.path == "audit.wire_layout"
                && field.kind == "unverified wire layout"
                && field.role == FieldRole::Control
        }));
    }

    #[test]
    fn authentication_token_response_preserves_route_ambiguity() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "5000000000210d55532d38393463316662663233613632353132353561633432366366616230613032342d393931363531343130",
        )
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Incoming, &bytes).unwrap();

        assert_eq!(record.service, "Authentication");
        assert_eq!(record.command_id, AUTH_GENERATE_WEB_TOKEN_COMMAND);
        assert_eq!(record.logical_bits, 416);
        assert_eq!(record.bytes.len(), 52);
        assert!(record.fields.iter().any(|field| {
            field.path == "audit.route_mapping"
                && field.value.contains("GenerateAppTokenResponse")
                && field.value.contains("same wire shape")
                && field.role == FieldRole::Control
        }));
        assert!(record.fields.iter().any(|field| {
            field.path.ends_with("m_webToken")
                && field.value == "45 bytes"
                && field.end_bit == record.logical_bits
        }));
    }

    #[test]
    fn authentication_token_request_preserves_route_ambiguity() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("500000000001").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Authentication");
        assert_eq!(record.command_id, AUTH_GENERATE_WEB_TOKEN_COMMAND);
        assert_eq!(record.logical_bits, 43);
        assert_eq!(record.bytes.len(), 6);
        assert!(record.fields.iter().any(|field| {
            field.path == "audit.route_mapping"
                && field.value.contains("GenerateAppTokenRequest")
                && field.value.contains("same wire shape")
                && field.role == FieldRole::Control
        }));
        assert!(record.fields.iter().any(|field| {
            field.path.ends_with("m_token")
                && field.value == "1"
                && field.end_bit == record.logical_bits
        }));
    }

    #[test]
    fn strict_outgoing_inspection_frames_pong_before_next_record() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("4cc93530000000000003cdcf2319000000000d00000001090001").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "Connection");
        assert_eq!(record.command_id, CONNECTION_PONG_COMMAND);
        assert_eq!(record.command, "Pong");
        assert_eq!(record.logical_bits, 76);
        assert_eq!(record.bytes, bytes[..10]);
        assert!(record.fields.iter().any(|field| {
            field.path.ends_with(".m_timeData") && field.start_bit == 11 && field.end_bit == 76
        }));
    }

    #[test]
    fn strict_outgoing_inspection_frames_billing_update_before_next_record() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("cdcf2319000000000d00000001090001").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "Toon");
        assert_eq!(record.command_id, TOON_BILLING_UPDATE_COMMAND);
        assert_eq!(record.command, "BillingUpdateNotify");
        assert_eq!(record.logical_bits, 100);
        assert_eq!(record.bytes, bytes[..13]);
        assert!(record.fields.iter().any(|field| {
            field.path.ends_with(".m_info.m_flags") && field.start_bit == 11 && field.end_bit == 43
        }));
        assert!(record.fields.iter().any(|field| {
            field
                .path
                .ends_with(".m_info.filler_before_m_subscriptionExpires")
                && field.start_bit == 43
                && field.end_bit == 62
        }));
        assert!(record.fields.iter().any(|field| {
            field
                .path
                .ends_with(".m_info.filler_before_m_unitsRemaining")
                && field.start_bit == 63
                && field.end_bit == 91
        }));
    }

    #[test]
    fn strict_outgoing_inspection_waits_for_complete_message_frame() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "4d010601010008906c00000000000000318104414cc93530000000000023f1e08115000000000d00000001090001",
        )
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        for end in 1..bytes.len() {
            assert!(
                matches!(
                    inspect_native_record(&protocol, Direction::Outgoing, &bytes[..end]),
                    Err(crate::Error::IncompleteFrame(_))
                ),
                "accepted a partial message frame containing {end} of {} bytes",
                bytes.len()
            );
        }
        assert_eq!(record.service, "Connection");
        assert_eq!(record.command_id, CONNECTION_MESSAGE_FRAME_COMMAND);
        assert_eq!(record.command, "MessageFrame");
        assert_eq!(record.logical_bits, 367);
        assert_eq!(record.bytes, bytes);
    }

    #[test]
    fn strict_outgoing_inspection_frames_site_latency_info() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "e30a1e555331302d53320200000310034f5244312d53320200000317024155312d53320200000e1c025341312d5332020000080903555333030000050e035347310100000f0c",
        )
        .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "S2Master");
        assert_eq!(record.command_id, S2_MASTER_SITE_LATENCY_INFO_COMMAND);
        assert_eq!(record.logical_bits, 556);
        assert_eq!(record.bytes.len(), 70);
        assert_eq!(
            record
                .fields
                .iter()
                .filter(|field| field.path.ends_with(".name") && field.kind == "string")
                .map(|field| field.value.as_str())
                .collect::<Vec<_>>(),
            ["US10-S2", "ORD1-S2", "AU1-S2", "SA1-S2", "US3", "SG1"]
        );
    }

    #[test]
    fn strict_outgoing_inspection_frames_mmq_get_list_variants() {
        let protocol = Protocol::current().unwrap();
        for (hex, logical_bits, tags) in [
            ("dc7a7c4041d75747020002991280001413", 133, 1),
            ("dc7a7c4042c6164644c6f74506000299128000650a", 165, 2),
        ] {
            let bytes = hex::decode(hex).unwrap();
            let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
            assert_eq!(record.service, "S2Master");
            assert_eq!(record.command_id, S2_MASTER_MMQ_GET_LIST_COMMAND);
            assert_eq!(record.command, "MMQGetListRequest");
            assert_eq!(record.logical_bits, logical_bits);
            assert_eq!(record.bytes, bytes);
            assert_eq!(
                record
                    .fields
                    .iter()
                    .filter(|field| field.path.starts_with("payload.filter.tags[")
                        && field.kind == "fourcc")
                    .count(),
                tags
            );
            assert!(
                record
                    .fields
                    .iter()
                    .any(|field| { field.path == "payload.program_id" && field.value == "\0\0S2" })
            );
        }
    }

    #[test]
    fn strict_outgoing_inspection_frames_empty_mmq_subscription() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("cb623800").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        for end in 1..bytes.len() {
            assert!(matches!(
                inspect_native_record(&protocol, Direction::Outgoing, &bytes[..end]),
                Err(crate::Error::IncompleteFrame(_))
            ));
        }
        assert_eq!(record.service, "S2Master");
        assert_eq!(record.command_id, S2_MASTER_MMQ_SUBSCRIBE_COMMAND);
        assert_eq!(record.command, "MMQSubscribe");
        assert_eq!(record.logical_bits, 32);
        assert_eq!(record.bytes, bytes);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_enabled" && field.kind == "bool" && field.value == "false"
        }));
    }

    #[test]
    fn strict_outgoing_inspection_frames_tagged_mmq_subscription() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("cb6a38404236f43641c6f74506").unwrap();
        for end in 1..bytes.len() {
            assert!(matches!(
                inspect_native_record(&protocol, Direction::Outgoing, &bytes[..end]),
                Err(crate::Error::IncompleteFrame(_))
            ));
        }
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "S2Master");
        assert_eq!(record.command_id, S2_MASTER_MMQ_SUBSCRIBE_COMMAND);
        assert_eq!(record.command, "MMQSubscribe");
        assert_eq!(record.logical_bits, 101);
        assert_eq!(record.bytes, bytes);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_enabled" && field.kind == "bool" && field.value == "true"
        }));
        assert_eq!(
            record
                .fields
                .iter()
                .filter(|field| {
                    field.path.starts_with("payload.m_filter.m_tags.value[")
                        && field.kind == "fourcc"
                })
                .map(|field| field.value.as_str())
                .collect::<Vec<_>>(),
            ["CoCa", "LotV"]
        );
    }

    #[test]
    fn strict_outgoing_inspection_frames_full_mmq_subscription() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("cb6a38404236f43641c6f7451600000902").unwrap();
        for end in 1..bytes.len() {
            assert!(matches!(
                inspect_native_record(&protocol, Direction::Outgoing, &bytes[..end]),
                Err(crate::Error::IncompleteFrame(_))
            ));
        }
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "S2Master");
        assert_eq!(record.command_id, S2_MASTER_MMQ_SUBSCRIBE_COMMAND);
        assert_eq!(record.command, "MMQSubscribe");
        assert_eq!(record.logical_bits, 133);
        assert_eq!(record.bytes, bytes);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_filter.m_mmqId.value" && field.value == "290"
        }));
        assert_eq!(
            record
                .fields
                .iter()
                .filter(|field| {
                    field.path.starts_with("payload.m_filter.m_tags.value[")
                        && field.kind == "fourcc"
                })
                .map(|field| field.value.as_str())
                .collect::<Vec<_>>(),
            ["CoCa", "LotV"]
        );
    }

    #[test]
    fn strict_incoming_inspection_frames_mmq_get_info_response() {
        let protocol = Protocol::current().unwrap();
        let mut bytes = hex::decode(
            "d102000000147332716800005553f0d0428b768f9e4189e52bd7174be28ad0758b158bd85bc6f20f214dc3b0fcfdcafebabe2fe3aa0d0000000001000053320000012300a1",
        )
        .unwrap();
        bytes.extend_from_slice(&[0xc0, 0x06, 0, 0]);
        let record = inspect_native_record(&protocol, Direction::Incoming, &bytes).unwrap();
        assert_eq!(record.service, "S2Master");
        assert_eq!(record.command_id, S2_MASTER_MMQ_GET_INFO_COMMAND);
        assert_eq!(record.command, "MMQGetInfoResponse");
        assert_eq!(record.logical_bits, 552);
        assert_eq!(record.bytes.len(), 69);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.queue_info.handle.program_id" && field.value == "\0\0S2"
        }));
    }

    #[test]
    fn strict_outgoing_inspection_frames_game_group_subscribe() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("cb558a49290c").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "S2Map");
        assert_eq!(record.command_id, S2_MAP_GAME_GROUP_SUBSCRIBE_COMMAND);
        assert_eq!(record.logical_bits, 44);
        assert_eq!(record.bytes.len(), 6);
        assert!(
            record
                .fields
                .iter()
                .any(|field| field.path == "payload.tag" && field.value == "TRIL")
        );
    }

    #[test]
    fn strict_inspection_frames_s2_map_favorites_exchange() {
        let protocol = Protocol::current().unwrap();
        let request = inspect_native_record(&protocol, Direction::Outgoing, &[0xcd, 0x05]).unwrap();
        assert_eq!(request.service, "S2Map");
        assert_eq!(request.command_id, S2_MAP_LIST_FAVORITES_COMMAND);
        assert_eq!(request.command, "S2ListMapFavoritesRequest");
        assert_eq!(request.logical_bits, 12);
        assert_eq!(request.bytes, [0xcd, 0x05]);

        let response =
            inspect_native_record(&protocol, Direction::Incoming, &[0xcd, 0x05, 0x08]).unwrap();
        assert_eq!(response.service, "S2Map");
        assert_eq!(response.command_id, S2_MAP_LIST_FAVORITES_COMMAND);
        assert_eq!(response.command, "S2ListMapFavoritesResponse");
        assert_eq!(response.logical_bits, 21);
        assert_eq!(response.bytes, [0xcd, 0x05, 0x08]);
        assert!(
            response
                .fields
                .iter()
                .any(|field| { field.path.ends_with(".m_endOfList") && field.value == "true" })
        );
    }

    #[test]
    fn strict_incoming_inspection_frames_modify_channel_list_response() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("6115000164e3a80c590c").unwrap();
        let response = inspect_native_record(&protocol, Direction::Incoming, &bytes).unwrap();
        assert_eq!(response.service, "Chat");
        assert_eq!(
            response.command_id,
            crate::native::protocol::CHAT_MODIFY_CHANNEL_LIST_RESPONSE_COMMAND
        );
        assert_eq!(response.command, "ModifyChannelListResponse2");
        assert_eq!(response.logical_bits, 76);
        assert_eq!(response.bytes, bytes);
    }

    #[test]
    fn strict_outgoing_inspection_frames_empty_chat_requests() {
        let protocol = Protocol::current().unwrap();
        for (bytes, command_id, command) in [
            (
                [0x55, 0x05],
                CHAT_CHANNEL_LIST_REQUEST_COMMAND,
                "ChannelListRequest",
            ),
            (
                [0x59, 0x05],
                CHAT_ENUM_CONFERENCES_COMMAND,
                "EnumConferenceDescriptions",
            ),
        ] {
            let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
            assert_eq!(record.service, "Chat");
            assert_eq!(record.command_id, command_id);
            assert_eq!(record.command, command);
            assert_eq!(record.logical_bits, 11);
            assert_eq!(record.bytes, bytes);
        }
    }

    #[test]
    fn strict_outgoing_inspection_frames_current_season_request() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("db02").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "S2Master");
        assert_eq!(record.command_id, S2_MASTER_CURRENT_SEASON_COMMAND);
        assert_eq!(record.command, "CurrentSeason");
        assert_eq!(record.logical_bits, 11);
        assert_eq!(record.bytes, bytes);
    }

    #[test]
    fn strict_outgoing_inspection_frames_achievement_listen_request() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("c000c85fd757ce3aab22a00000000001").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(record.service, "Achievement");
        assert_eq!(record.command_id, ACHIEVEMENT_LISTEN_COMMAND);
        assert_eq!(record.command, "ListenRequest");
        assert_eq!(record.logical_bits, 123);
        assert_eq!(record.bytes, bytes);
    }

    #[test]
    fn strict_incoming_inspection_frames_game_group_update() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("cc050dae600002bb7a6c0100000002").unwrap();
        let record = inspect_native_record(&protocol, Direction::Incoming, &bytes).unwrap();
        assert_eq!(record.service, "S2Map");
        assert_eq!(record.command_id, S2_MAP_GAME_GROUP_UPDATE_COMMAND);
        assert_eq!(record.command, "GameGroupUpdate");
        assert_eq!(record.logical_bits, 117);
        assert_eq!(record.bytes, bytes);
        assert!(
            record
                .fields
                .iter()
                .any(|field| field.path == "payload.tag" && field.value == "\0WoL")
        );
    }

    #[test]
    fn strict_outgoing_inspection_frames_presence_statistics_subscription() {
        let protocol = Protocol::current().unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &[0x42, 0x0c]).unwrap();
        assert_eq!(record.service, "Presence");
        assert_eq!(record.command_id, PRESENCE_STATISTICS_SUBSCRIBE_COMMAND);
        assert_eq!(record.command, "StatisticsSubscribe");
        assert_eq!(record.logical_bits, 12);
        assert_eq!(record.bytes, [0x42, 0x0c]);
        assert!(
            record
                .fields
                .iter()
                .any(|field| field.path == "payload.on" && field.value == "1")
        );
    }

    #[test]
    fn strict_outgoing_inspection_frames_presence_updates() {
        let protocol = Protocol::current().unwrap();
        for (wire, logical_bits, kind) in [
            ("405c000280070303bdb68303", 90, "11"),
            ("4034ea80aa940001000b05037b6d0601", 121, "6"),
            ("402c000000000005000c0602079f2601", 121, "5"),
            (
                "408c000000000000000000012020200005000b0503079f2601",
                193,
                "17",
            ),
            ("400c00000500050203079f2601", 97, "1"),
        ] {
            let bytes = hex::decode(wire).unwrap();
            let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes)
                .unwrap_or_else(|error| panic!("{wire}: {error}"));
            assert_eq!(record.service, "Presence");
            assert_eq!(record.command_id, PRESENCE_UPDATE_COMMAND);
            assert_eq!(record.command, "UpdateRequest");
            assert_eq!(record.logical_bits, logical_bits);
            assert_eq!(record.bytes, bytes);
            assert!(
                record
                    .fields
                    .iter()
                    .any(|field| field.path == "payload.value.kind" && field.value == kind)
            );
        }
    }

    #[test]
    fn strict_outgoing_inspection_frames_encryption_and_profile_read() {
        let protocol = Protocol::current().unwrap();
        let marker = protocol.enable_encryption().unwrap();
        let marker = inspect_native_record(&protocol, Direction::Outgoing, &marker).unwrap();
        assert_eq!(marker.service, "Connection");
        assert_eq!(marker.command_id, CONNECTION_ENABLE_ENCRYPTION_COMMAND);

        let profile = protocol
            .profile_read(
                24,
                super::super::model::ProfileAddress {
                    label: 0xcafe_babe,
                    id: 0xdc82_c80e_0000_0000,
                },
                &[4],
            )
            .unwrap();
        let profile = inspect_native_record(&protocol, Direction::Outgoing, &profile).unwrap();
        assert_eq!(profile.service, "Profile");
        assert_eq!(profile.command_id, PROFILE_READ_COMMAND);
        assert_eq!(profile.bytes.len(), 24);
        assert!(profile.fields.iter().any(|field| {
            field.path == "payload.specification.path" && field.start_bit < field.end_bit
        }));

        let generated_path =
            hex::decode("c00600000000000001c95fd757c6daab22a000000030020221000116").unwrap();
        let generated_path =
            inspect_native_record(&protocol, Direction::Outgoing, &generated_path).unwrap();
        assert_eq!(generated_path.logical_bits, 224);
        assert_eq!(generated_path.bytes.len(), 28);
        assert!(generated_path.fields.iter().any(|field| {
            field.path == "payload.specification.generated_path[0]" && field.value == "2100"
        }));
        assert!(generated_path.fields.iter().any(|field| {
            field.path == "payload.specification.generated_path[1]" && field.value == "16"
        }));
    }

    #[test]
    fn strict_outgoing_inspection_decodes_profile_slice_selection() {
        let protocol = Protocol::current().unwrap();
        let bytes =
            hex::decode("c00600000000000005cc5fd757ce1aab22a00000001001027ffffffffffffe0c00")
                .unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Profile");
        assert_eq!(record.command_id, PROFILE_READ_COMMAND);
        assert_eq!(record.logical_bits, 260);
        assert_eq!(record.bytes, bytes);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.specification.selection" && field.value == "1"
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.specification.selection.slice.m_sliceStart.value"
                && field.value == "-1000"
        }));
    }

    #[test]
    fn profile_handle_resolution_uses_the_captured_generated_order() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode("c20601002999000100000001000000006acf9300").unwrap();
        let record = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();

        assert_eq!(record.service, "Profile");
        assert_eq!(record.command, "ResolveToonHandleToNameRequest");
        assert_eq!(
            record.command_id,
            PROFILE_RESOLVE_TOON_HANDLE_REQUEST_COMMAND
        );
        assert_eq!(record.logical_bits, 153);
        assert_eq!(record.bytes, bytes);
        assert!(record.fields.iter().any(|field| {
            field.path == "payload.m_handles[0].m_id" && field.value == "13999910"
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "audit.wire_layout" && field.kind == "unverified wire layout"
        }));
    }

    #[test]
    fn strict_outgoing_inspection_frames_generated_profile_path_before_message_frame() {
        let protocol = Protocol::current().unwrap();
        let bytes = hex::decode(
            "c00600000000000003cf5fd757ee02ab62a000000030030102010401014d010601010008906c00000000000000318104414cc93530000000000023f1e08115000000000d00000001090001",
        )
        .unwrap();
        let profile = inspect_native_record(&protocol, Direction::Outgoing, &bytes).unwrap();
        assert_eq!(profile.service, "Profile");
        assert_eq!(profile.command_id, PROFILE_READ_COMMAND);
        assert_eq!(profile.logical_bits, 232);
        assert_eq!(profile.bytes.len(), 29);
        assert_eq!(
            profile
                .fields
                .iter()
                .filter(|field| {
                    field
                        .path
                        .starts_with("payload.specification.generated_path[")
                        && !field.path.ends_with(".length")
                })
                .map(|field| field.value.as_str())
                .collect::<Vec<_>>(),
            ["02", "04", "01"]
        );

        let frame = inspect_native_record(
            &protocol,
            Direction::Outgoing,
            &bytes[profile.bytes.len()..],
        )
        .unwrap();
        assert_eq!(frame.service, "Connection");
        assert_eq!(frame.command_id, CONNECTION_MESSAGE_FRAME_COMMAND);
        assert_eq!(frame.bytes.len(), 46);
    }

    #[test]
    fn bgs_messages_expose_framing_headers_and_generic_protobuf_fields() {
        let header = RpcHeader {
            service_id: 0,
            method_id: Some(1),
            token: 7,
            service_hash: Some(bgs_service_hash(
                "bnet.protocol.authentication.AuthenticationServer",
            )),
            ..RpcHeader::default()
        };
        let body = [0x08, 0x01, 0x12, 0x03, b'a', b'b', b'c'];
        let bytes = crate::wire::protobuf::encode_frame(&header, &body).unwrap();
        let record = inspect_bgs(&RawBgsRecord {
            direction: Direction::Outgoing,
            header: Some(header),
            route: Some(BgsRoute {
                service_hash: bgs_service_hash("bnet.protocol.authentication.AuthenticationServer"),
                method_id: 1,
            }),
            bytes,
            error: None,
        });

        assert_eq!(record.service, "BGS · Authentication");
        assert_eq!(record.command, "Logon");
        assert!(record.fields.iter().any(|field| {
            field.path == "header.service_hash"
                && field.kind == "protobuf fixed32"
                && field.exact_range
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "body.field_1" && field.value == "1" && field.exact_range
        }));
        assert!(record.fields.iter().any(|field| {
            field.path == "body.field_2.value" && field.value == "abc" && field.exact_range
        }));
    }

    #[test]
    fn http_messages_expose_start_line_headers_json_and_redaction() {
        let headers = sanitized_headers(&[
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("Authorization".to_owned(), "Bearer secret".to_owned()),
        ]);
        let record = inspect_http(&RawHttpRecord {
            direction: Direction::Outgoing,
            method: "POST".to_owned(),
            url: "https://example.com/v1/events?batch=1".to_owned(),
            status: None,
            headers,
            body: br#"{"accepted":5}"#.to_vec(),
        });

        assert_eq!(record.service, "HTTP");
        assert_eq!(record.command, "POST /v1/events?batch=1");
        assert!(
            record
                .bytes
                .starts_with(b"POST /v1/events?batch=1 HTTP/1.1\r\n")
        );
        assert!(
            record
                .bytes
                .windows(b"Authorization: <redacted>".len())
                .any(|window| window == b"Authorization: <redacted>")
        );
        assert!(
            record
                .fields
                .iter()
                .any(|field| field.path == "body.json.accepted" && field.value == "5")
        );
        assert!(
            record
                .fields
                .iter()
                .filter(|field| field.exact_range)
                .all(|field| field.end_bit <= record.logical_bits)
        );
    }

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
