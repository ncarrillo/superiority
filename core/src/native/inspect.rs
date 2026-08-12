use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    Result,
    bsn::bits::{BitReader, RoutingHeader},
    bsn::value::BsnValue,
};

use super::{Payload, Protocol, protocol::CHAT_SLOT};

const CAPTURE_LIMIT: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldRole {
    Route,
    Control,
    Payload,
    Padding,
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

    fn container(
        path: impl Into<String>,
        kind: &'static str,
        value: impl Into<String>,
        start_bit: usize,
        end_bit: usize,
        depth: usize,
    ) -> Self {
        Self::leaf(
            path,
            kind,
            value,
            start_bit,
            end_bit,
            depth,
            FieldRole::Payload,
        )
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
    header: RoutingHeader,
    type_id: u32,
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
                    let mut reader = BitReader::new(&raw.bytes, None).ok()?;
                    reader.set_position(raw.header.bit_count).ok()?;
                    let (_, payload) = protocol
                        .decode_incoming_from(&mut reader, raw.header)
                        .ok()?;
                    let mut record = inspect_incoming(
                        &protocol,
                        raw.header,
                        raw.type_id,
                        &payload,
                        &raw.bytes,
                        raw.logical_bits,
                    )?;
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
    type_id: u32,
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
        header,
        type_id,
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
    let mut reader = BitReader::new(bytes, None)?;
    let mut fields = Vec::new();
    let command_start = reader.position();
    let command_id = read_u8(&mut reader, 6)?;
    fields.push(Field::leaf(
        "route.command_id",
        "uint6",
        command_id.to_string(),
        command_start,
        reader.position(),
        1,
        FieldRole::Route,
    ));
    let present_start = reader.position();
    let service_present = reader.read(1)? != 0;
    fields.push(Field::leaf(
        "route.service_present",
        "bool",
        service_present.to_string(),
        present_start,
        reader.position(),
        1,
        FieldRole::Route,
    ));
    let slot_start = reader.position();
    let service_slot = if service_present {
        read_u8(&mut reader, 4)?
    } else {
        0
    };
    if service_present {
        fields.push(Field::leaf(
            "route.service_slot",
            "uint4",
            service_slot.to_string(),
            slot_start,
            reader.position(),
            1,
            FieldRole::Route,
        ));
    }
    let route_end = reader.position();
    fields.insert(
        0,
        Field::container(
            "route",
            "routing header",
            format!("slot {service_slot}, command {command_id}"),
            0,
            route_end,
            0,
        ),
    );

    let payload_start = reader.position();
    let success_start = reader.position();
    let success = reader.read(1)? == 0;
    fields.push(Field::leaf(
        "payload.success",
        "bool (inverted)",
        success.to_string(),
        success_start,
        reader.position(),
        1,
        FieldRole::Control,
    ));
    if success {
        push_integer(
            &mut fields,
            &mut reader,
            "payload.member_handle",
            "uint32",
            32,
            1,
        )?;
        push_integer(
            &mut fields,
            &mut reader,
            "payload.channel_index",
            "uint3",
            3,
            1,
        )?;
        push_integer(
            &mut fields,
            &mut reader,
            "payload.conference_id",
            "uint32",
            32,
            1,
        )?;
        push_integer(
            &mut fields,
            &mut reader,
            "payload.owner_id",
            "uint32",
            32,
            1,
        )?;
        push_integer(
            &mut fields,
            &mut reader,
            "payload.channel_type",
            "uint4",
            4,
            1,
        )?;

        let name_present_start = reader.position();
        let name_present = reader.read(1)? != 0;
        fields.push(Field::leaf(
            "payload.channel_name.present",
            "optional flag",
            name_present.to_string(),
            name_present_start,
            reader.position(),
            2,
            FieldRole::Control,
        ));
        if name_present {
            let name_start = name_present_start;
            push_integer(
                &mut fields,
                &mut reader,
                "payload.channel_name.region",
                "uint16",
                16,
                2,
            )?;
            push_integer(
                &mut fields,
                &mut reader,
                "payload.channel_name.namespace",
                "uint29",
                29,
                2,
            )?;
            let variant_start = reader.position();
            let variant = read_u8(&mut reader, 2)?;
            fields.push(Field::leaf(
                "payload.channel_name.kind",
                "choice selector",
                channel_name_kind(variant),
                variant_start,
                reader.position(),
                2,
                FieldRole::Control,
            ));
            match variant {
                2 => {
                    push_integer(
                        &mut fields,
                        &mut reader,
                        "payload.channel_name.locale",
                        "fourcc",
                        32,
                        2,
                    )?;
                    push_integer(
                        &mut fields,
                        &mut reader,
                        "payload.channel_name.id",
                        "uint16",
                        16,
                        2,
                    )?;
                }
                1 | 3 => {
                    push_integer(
                        &mut fields,
                        &mut reader,
                        "payload.channel_name.index",
                        "uint16",
                        16,
                        2,
                    )?;
                    push_integer(
                        &mut fields,
                        &mut reader,
                        "payload.channel_name.owner",
                        "uint32",
                        32,
                        2,
                    )?;
                }
                0 => push_string(
                    &mut fields,
                    &mut reader,
                    "payload.channel_name.literal",
                    7,
                    2,
                )?,
                _ => unreachable!(),
            }
            fields.push(Field::container(
                "payload.channel_name",
                "optional choice",
                "present",
                name_start,
                reader.position(),
                1,
            ));
        }

        push_optional_bits(&mut fields, &mut reader, "payload.channel_config", 0, 1)?;
        push_optional_bits(&mut fields, &mut reader, "payload.reserved", 32, 1)?;

        let token_start = reader.position();
        let token_present = reader.read(1)? != 0;
        fields.push(Field::leaf(
            "payload.token.present",
            "optional flag",
            token_present.to_string(),
            token_start,
            reader.position(),
            2,
            FieldRole::Control,
        ));
        if token_present {
            push_integer(
                &mut fields,
                &mut reader,
                "payload.token.value",
                "uint32",
                32,
                2,
            )?;
        }
        fields.push(Field::container(
            "payload.token",
            "optional<uint32>",
            if token_present { "present" } else { "none" },
            token_start,
            reader.position(),
            1,
        ));
    } else {
        push_integer(&mut fields, &mut reader, "payload.reason", "uint16", 16, 1)?;
        push_optional_bits(&mut fields, &mut reader, "payload.channel_type", 4, 1)?;
        push_optional_bits(&mut fields, &mut reader, "payload.token", 32, 1)?;
    }
    let payload_end = reader.position();
    fields.push(Field::container(
        "payload",
        "Chat.JoinNotify2",
        if success { "success" } else { "failure" },
        payload_start,
        payload_end,
        0,
    ));

    let padding_start = reader.position();
    let padding = padding_start.wrapping_neg() & 7;
    if padding > 0 {
        let value = reader.read(padding)?;
        fields.push(Field::leaf(
            "padding",
            "zero bits",
            value.to_string(),
            padding_start,
            reader.position(),
            0,
            FieldRole::Padding,
        ));
    }
    let logical_bits = padding_start;
    let mut validation = BitReader::new(bytes, None)?;
    validation.set_position(route_end)?;
    protocol.decode_incoming_from(
        &mut validation,
        crate::bsn::bits::RoutingHeader {
            command_id,
            service_slot: service_present.then_some(service_slot),
            bit_count: route_end,
        },
    )?;
    fields.sort_by(|left, right| {
        left.start_bit
            .cmp(&right.start_bit)
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| right.end_bit.cmp(&left.end_bit))
    });
    Ok(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
        service: "Chat".to_owned(),
        command: "JoinNotify2".to_owned(),
        type_name: "Battlenet::Client::Chat::JoinNotify2".to_owned(),
        service_slot,
        command_id,
        bytes: bytes.to_vec(),
        logical_bits,
        fields,
    })
}

fn inspect_incoming(
    protocol: &Protocol,
    header: RoutingHeader,
    type_id: u32,
    payload: &Payload,
    bytes: &[u8],
    logical_bits: usize,
) -> Option<Record> {
    if header.service_slot == Some(CHAT_SLOT)
        && header.command_id == super::protocol::CHAT_JOIN_NOTIFY_COMMAND
    {
        return inspect_chat_join(protocol, bytes).ok();
    }
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
    let mut fields = vec![
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
    ];
    let traced_fields = append_traced_payload(
        &mut fields,
        protocol,
        type_id,
        bytes,
        route_end,
        logical_bits,
    );
    if traced_fields == 0 {
        append_decoded_payload(&mut fields, payload, bytes, route_end, logical_bits);
    } else if traced_fields == 1 && payload.reflected().is_none() {
        fields.pop();
        append_decoded_payload(&mut fields, payload, bytes, route_end, logical_bits);
    }
    let total_bits = bytes.len() * 8;
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
    Some(Record {
        sequence: 0,
        captured_at_millis: now_millis(),
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

fn append_traced_payload(
    fields: &mut Vec<Field>,
    protocol: &Protocol,
    type_id: u32,
    bytes: &[u8],
    start_bit: usize,
    end_bit: usize,
) -> usize {
    let Ok(mut reader) = BitReader::new(bytes, Some(end_bit)) else {
        return 0;
    };
    if reader.set_position(start_bit).is_err() {
        return 0;
    }
    let Ok(decoded) = protocol.codec().decode_traced_from(&mut reader, type_id) else {
        return 0;
    };
    if reader.position() != end_bit {
        return 0;
    }
    let field_count = decoded.fields.len();
    fields.extend(decoded.fields.into_iter().map(|field| {
        let path = field
            .path
            .strip_prefix("value")
            .map_or(field.path.clone(), |suffix| format!("payload{suffix}"));
        Field::leaf(
            path,
            field.kind,
            field.value,
            field.start_bit,
            field.end_bit,
            field.depth,
            FieldRole::Payload,
        )
    }));
    field_count
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
    bytes: &[u8],
    start_bit: usize,
    end_bit: usize,
) {
    if let Some(value) = payload.reflected() {
        append_bsn_value(fields, "payload", value, start_bit, end_bit, 1);
        return;
    }
    if let Payload::ClubSummaries(clubs) | Payload::ClubInfo(clubs) = payload
        && append_club_summaries(fields, clubs, bytes, start_bit, end_bit)
    {
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

fn append_club_summaries(
    fields: &mut Vec<Field>,
    clubs: &[super::model::ClubSummary],
    bytes: &[u8],
    start_bit: usize,
    end_bit: usize,
) -> bool {
    let traces = super::decode::club_summary_traces(bytes, end_bit);
    if traces.len() != clubs.len() {
        return false;
    }
    fields.push(Field::container(
        "payload",
        "array",
        format!("{} items", clubs.len()),
        start_bit,
        end_bit,
        0,
    ));
    for (index, (club, trace)) in clubs.iter().zip(traces).enumerate() {
        let path = format!("payload[{index}]");
        fields.push(Field::container(
            path.clone(),
            "object",
            "5 fields",
            trace.item.start,
            trace.item.end,
            1,
        ));
        fields.push(Field::leaf(
            format!("{path}.club_id"),
            "uint32",
            club.club_id.to_string(),
            trace.club_id.start,
            trace.club_id.end,
            2,
            FieldRole::Payload,
        ));
        fields.push(Field::leaf(
            format!("{path}.name"),
            "string",
            club.name.clone().unwrap_or_default(),
            trace.name.start,
            trace.name.end,
            2,
            FieldRole::Payload,
        ));
        fields.push(Field::leaf(
            format!("{path}.kind"),
            "uint8",
            club.kind.to_string(),
            trace.kind.start,
            trace.kind.end,
            2,
            FieldRole::Payload,
        ));
        fields.push(Field::leaf(
            format!("{path}.category"),
            "uint8",
            club.category.to_string(),
            trace.category.start,
            trace.category.end,
            2,
            FieldRole::Payload,
        ));
        fields.push(Field::leaf(
            format!("{path}.private"),
            "bool",
            club.private.to_string(),
            trace.private.start,
            trace.private.end,
            2,
            FieldRole::Payload,
        ));
    }
    true
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

fn push_integer(
    fields: &mut Vec<Field>,
    reader: &mut BitReader<'_>,
    path: impl Into<String>,
    kind: &'static str,
    width: usize,
    depth: usize,
) -> Result<()> {
    let start = reader.position();
    let value = reader.read(width)?;
    fields.push(Field::leaf(
        path,
        kind,
        if width > 8 {
            format!("0x{value:0digits$x}", digits = width.div_ceil(4))
        } else {
            value.to_string()
        },
        start,
        reader.position(),
        depth,
        FieldRole::Payload,
    ));
    Ok(())
}

fn push_optional_bits(
    fields: &mut Vec<Field>,
    reader: &mut BitReader<'_>,
    path: &'static str,
    value_bits: usize,
    depth: usize,
) -> Result<()> {
    let start = reader.position();
    let present = reader.read(1)? != 0;
    fields.push(Field::leaf(
        format!("{path}.present"),
        "optional flag",
        present.to_string(),
        start,
        reader.position(),
        depth + 1,
        FieldRole::Control,
    ));
    if present && value_bits > 0 {
        push_integer(
            fields,
            reader,
            format!("{path}.value"),
            "bits",
            value_bits,
            depth + 1,
        )?;
    }
    fields.push(Field::container(
        path,
        "optional",
        if present { "present" } else { "none" },
        start,
        reader.position(),
        depth,
    ));
    Ok(())
}

fn push_string(
    fields: &mut Vec<Field>,
    reader: &mut BitReader<'_>,
    path: &'static str,
    length_bits: usize,
    depth: usize,
) -> Result<()> {
    let start = reader.position();
    let length_start = start;
    let byte_count = usize::try_from(reader.read(length_bits)?).expect("length fits usize");
    fields.push(Field::leaf(
        format!("{path}.length"),
        "bounded length",
        byte_count.to_string(),
        length_start,
        reader.position(),
        depth + 1,
        FieldRole::Control,
    ));
    let align_start = reader.position();
    let skipped = reader.align()?;
    if skipped > 0 {
        fields.push(Field::leaf(
            format!("{path}.alignment"),
            "zero bits",
            skipped.to_string(),
            align_start,
            reader.position(),
            depth + 1,
            FieldRole::Padding,
        ));
    }
    let bytes_start = reader.position();
    let bytes = reader.read_bytes(byte_count, false)?;
    fields.push(Field::leaf(
        format!("{path}.utf8"),
        "utf8 bytes",
        String::from_utf8_lossy(&bytes),
        bytes_start,
        reader.position(),
        depth + 1,
        FieldRole::Payload,
    ));
    fields.push(Field::container(
        path,
        "string",
        String::from_utf8_lossy(&bytes),
        start,
        reader.position(),
        depth,
    ));
    Ok(())
}

fn read_u8(reader: &mut BitReader<'_>, width: usize) -> Result<u8> {
    Ok(u8::try_from(reader.read(width)?).expect("at most eight bits"))
}

const fn channel_name_kind(value: u8) -> &'static str {
    match value {
        0 => "literal name",
        1 => "private channel",
        2 => "localized public channel",
        3 => "group channel",
        _ => "invalid",
    }
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
    }

    #[test]
    fn reflected_payload_fields_keep_their_wire_ranges() {
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
        let (type_id, payload) = protocol.decode_incoming_from(&mut reader, header).unwrap();
        let logical_bits = reader.position();

        let record =
            inspect_incoming(&protocol, header, type_id, &payload, &bytes, logical_bits).unwrap();

        let body = record
            .fields
            .iter()
            .find(|field| field.path == "payload.m_body")
            .expect("the decoded body must have a traced node");
        assert!(body.exact_range);
        assert_eq!(body.value, "hola");
        assert!(body.start_bit > header.bit_count);
        assert_eq!(body.end_bit, logical_bits);
        assert!(body.end_bit - body.start_bit < logical_bits - header.bit_count);
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

        append_decoded_payload(&mut fields, &payload, &[], 11, 2304);

        assert_eq!(fields[0].path, "payload");
        assert_eq!(fields[0].kind, "array");
        assert_eq!(fields[0].value, "1 item");
        assert_eq!(fields[1].path, "payload[0]");
        assert_eq!(fields[1].kind, "object");
        assert!(fields.iter().any(|field| {
            field.path == "payload[0].name" && field.value == "cecw" && field.depth == 2
        }));
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
        let (type_id, payload) = protocol.decode_incoming_from(&mut reader, header).unwrap();
        let logical_bits = reader.position();
        let record =
            inspect_incoming(&protocol, header, type_id, &payload, &bytes, logical_bits).unwrap();

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
