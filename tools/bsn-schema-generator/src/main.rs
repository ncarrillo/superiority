//! walks the transitive closure of the client's message roots and write
//! a Rust module *per BSN domain* (chat.rs, club.rs, toon.rs, …) of typed
//! structs/enums/choices with `FromBsn` projections, plus the static schema
//! table used by the runtime bit codec

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;

use superiority_core::metadata::{IntegerRange, Metadata, TypeKind, TypeShape, read_metadata};
/// where StarCraft II's schema is written when nothing says otherwise. Another
/// product that turns out to use BSN writes somewhere else — see `--out`.
const DEFAULT_OUT_DIR: &str = "core/src/games/sc2/native/schema";
/// StarCraft II's metadata blob, likewise.
const DEFAULT_METADATA: &str = "protocol/bsn/sc2-97364-metadata.bin";

const USAGE: &str = "usage: bsn-schema-generator [--metadata <blob>] [--out <dir>]

  --metadata <blob>  BSN metadata to generate from
                     (default: SC2_CODEGEN_METADATA, else StarCraft II's)
  --out <dir>        where to write the schema modules
                     (default: StarCraft II's, under core/src/games/sc2)";

const ROOTS: &[&str] = &[
    "Battlenet::Client::Achievement::ListenRequest",
    "Battlenet::Client::Achievement::Data",
    "Battlenet::Client::Authentication::Configuration",
    "Battlenet::Client::Authentication::LogonRequest3",
    "Battlenet::Client::Authentication::LogonResponse3",
    "Battlenet::Client::Authentication::ProofRequest",
    "Battlenet::Client::Authentication::ProofResponse",
    "Battlenet::Client::Authentication::GenerateWebTokenRequest",
    "Battlenet::Client::Authentication::GenerateWebTokenResponse",
    "Battlenet::Client::Authentication::ResumeRequest",
    "Battlenet::Client::Authentication::ResumeResponse",
    "Battlenet::Client::Authentication::SingleSignOnRequest3",
    "Battlenet::Client::Profile::SendStatsUIEvent",
    "Battlenet::Client::Cache::GetStreamItemsResponse",
    "Battlenet::Client::Chat::ChannelListRequest",
    "Battlenet::Client::Chat::ChannelListResponse",
    "Battlenet::Client::Chat::CategoryDescriptions",
    "Battlenet::Client::Chat::ConferenceDescriptions",
    "Battlenet::Client::Chat::ConferenceMemberCounts",
    "Battlenet::Client::Chat::CreateAndInviteRequest",
    "Battlenet::Client::Chat::DatagramConnectionUpdate",
    "Battlenet::Client::Chat::EnumCategoryDescriptions",
    "Battlenet::Client::Chat::EnumConferenceDescriptions",
    "Battlenet::Client::Chat::EnumConferenceMemberCounts",
    "Battlenet::Client::Chat::InviteAccept",
    "Battlenet::Client::Chat::InviteDecline",
    "Battlenet::Client::Chat::InviteNotify",
    "Battlenet::Client::Chat::JoinRequest2",
    "Battlenet::Client::Chat::JoinNotify2",
    "Battlenet::Client::Chat::MembershipChangeNotify",
    "Battlenet::Client::Chat::MessageRecv",
    "Battlenet::Client::Chat::MessageSend",
    "Battlenet::Client::Chat::ModifyChannelListRequest",
    "Battlenet::Client::Chat::ModifyChannelListRequest2",
    "Battlenet::Client::Chat::ModifyChannelListResponse2",
    "Battlenet::Client::Chat::StatusChangeRequest",
    "Battlenet::Client::Chat::WhisperEchoRecv",
    "Battlenet::Client::Chat::WhisperRecv",
    "Battlenet::Client::Club::ClubSettings",
    "Battlenet::Client::Club::ClubChangeNotification",
    "Battlenet::Client::Club::ClubSubscribeRequest",
    "Battlenet::Client::Club::GetClubInfoRequest",
    "Battlenet::Client::Club::GetClubInfoResponse",
    "Battlenet::Client::Club::GetMemberClanTagsResponse",
    "Battlenet::Client::Club::GetToonClubsResponse",
    "Battlenet::Client::Club::InviteAction",
    "Battlenet::Client::Club::SearchClubs",
    "Battlenet::Client::Club::SearchClubsRequest",
    "Battlenet::Client::Club::SearchClubsRequest::Search::Name",
    "Battlenet::Client::Club::SearchClubsResponse",
    "Battlenet::Client::Connection::Boom",
    "Battlenet::Client::Connection::EnableEncryption",
    "Battlenet::Client::Connection::GameSiteInfo",
    "Battlenet::Client::Connection::LogoutRequest",
    "Battlenet::Client::Connection::MessageFrame",
    "Battlenet::Client::Connection::Ping",
    "Battlenet::Client::Connection::Pong",
    "Battlenet::Client::Connection::RegulatorUpdate",
    "Battlenet::Client::Connection::ServerVersion",
    "Battlenet::Client::Friends::AccountBlockAddedNotify",
    "Battlenet::Client::Friends::FriendInvitationAddedNotify",
    "Battlenet::Client::Friends::FriendsListNotify5",
    "Battlenet::Client::Friends::ToonBlockNotify",
    "Battlenet::Client::Friends::ToonsOfFriendsNotify",
    "Battlenet::Client::Friends::ToonsOfFriendsRequest",
    "Battlenet::Client::Party::BeginReadyProcess",
    "Battlenet::Client::Party::ModifyNonLobbyAttributeList",
    "Battlenet::Client::Party::ModifyMapOptions",
    "Battlenet::Client::Party::MapOptionsChange",
    "Battlenet::Client::Party::ReadyProcessUpdate",
    "Battlenet::Client::Presence::FieldSpecAnnounce",
    "Battlenet::Client::Presence::StatisticsSubscribe",
    "Battlenet::Client::Presence::StatisticsUpdate",
    "Battlenet::Client::Presence::TemporaryPresenceRequest",
    "Battlenet::Client::Presence::TemporaryPresenceResponse",
    "Battlenet::Client::Presence::UpdateNotify",
    "Battlenet::Client::Profile::AddressQueryRequest",
    "Battlenet::Client::Profile::AddressQueryResponse",
    "Battlenet::Client::Profile::ReadRequest",
    "Battlenet::Client::Profile::ReadResponse",
    "Battlenet::Client::Profile::ResolveToonHandleToNameRequest",
    "Battlenet::Client::Profile::ResolveToonHandleToNameResponse",
    "Battlenet::Client::Profile::ResolveToonNameToHandleRequest",
    "Battlenet::Client::Profile::ResolveToonNameToHandleResponse",
    "Battlenet::Client::Profile::SettingsAvailable",
    "Battlenet::Client::S2Map::GameGroupUpdate",
    "Battlenet::Client::S2Map::S2ListMapFavoritesRequest",
    "Battlenet::Client::S2Map::S2ListMapFavoritesResponse",
    "Battlenet::Client::S2Master::CurrentSeasonResponse",
    "Battlenet::Client::S2Master::MMQGetInfoRequest",
    "Battlenet::Client::S2Master::MMQGetListResponse",
    "Battlenet::Client::S2Master::MMQAnnounce",
    "Battlenet::Client::S2Master::MMQSubscribe",
    "Battlenet::Client::Toon::InitialNotifiesComplete",
    "Battlenet::Client::Toon::BillingUpdateNotify",
    "Battlenet::Client::Toon::CaisTimeUpdate",
    "Battlenet::Client::Toon::Failure",
    "Battlenet::Client::Toon::ToonCreateCancel",
    "Battlenet::Client::Toon::ToonCreateFinal",
    "Battlenet::Client::Toon::ToonCreateInit",
    "Battlenet::Client::Toon::ToonCreateSetup",
    "Battlenet::Client::Toon::ToonCreated",
    "Battlenet::Client::Toon::ToonList",
    "Battlenet::Client::Toon::ToonSelected",
    "Battlenet::Client::Toon::Welcome",
    "Battlenet::Club::ClubName",
    "Battlenet::Profile::ProfileDataResponse",
    "Battlenet::Profile::RecordAddress",
    "Battlenet::Token",
    "Battlenet::Toon::FullName",
    "Battlenet::Toon::Handle",
];

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", "box", "gen", "try", "union", "yield",
];

/// `--metadata <path>` and `--out <dir>`, so a second product's blob generates
/// into its own module rather than over StarCraft II's. `SC2_CODEGEN_METADATA`
/// still works and is what `--metadata` defaults to.
fn arguments() -> (PathBuf, PathBuf) {
    let mut metadata = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--metadata" => metadata = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => panic!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    let metadata = metadata
        .or_else(|| std::env::var_os("SC2_CODEGEN_METADATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_METADATA));
    (
        metadata,
        out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT_DIR)),
    )
}

fn main() {
    let (input, out_dir) = arguments();
    let out_dir = out_dir.display().to_string();
    let meta = read_metadata(&input).unwrap_or_else(|error| {
        panic!(
            "could not read BSN codegen input {}: {error}; pass --metadata <blob>",
            input.display()
        )
    });
    let mut queue: VecDeque<u32> = ROOTS
        .iter()
        .map(|name| meta.unique_type_id(name).unwrap())
        .collect();
    let mut seen = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        let shape = meta.shape(id).unwrap();
        queue.extend(shape.member_types.iter().copied());
        queue.extend(shape.element_type);
    }

    let mut named: Vec<u32> = seen
        .iter()
        .copied()
        .filter(|&id| {
            matches!(
                meta.shape(id).unwrap().kind,
                TypeKind::Struct | TypeKind::Choice | TypeKind::Enum
            )
        })
        .collect();
    named.sort_by_key(|&id| type_name(&meta, id));

    // group the named types into one buffer per BSN domain.
    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let (mut structs, mut enums, mut choices) = (0, 0, 0);
    for id in named {
        let shape = meta.shape(id).unwrap();
        let buffer = modules
            .entry(module_of(&meta, id))
            .or_insert_with(file_header);
        match shape.kind {
            TypeKind::Struct => {
                emit_struct(buffer, &meta, id, &shape);
                structs += 1;
            }
            TypeKind::Enum => {
                emit_enum(buffer, &meta, id, &shape);
                enums += 1;
            }
            TypeKind::Choice => {
                emit_choice(buffer, &meta, id, &shape);
                choices += 1;
            }
            _ => unreachable!(),
        }
    }

    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut root = String::from(
        "#![expect(\n\
         \x20   clippy::struct_excessive_bools,\n\
         \x20   reason = \"generated structures preserve the protocol schema\"\n\
         )]\n\n",
    );
    for (module, body) in &modules {
        std::fs::write(format!("{out_dir}/{module}.rs"), body).unwrap();
        writeln!(root, "pub mod {module};").unwrap();
    }
    root.push_str("pub(crate) mod wire;\n");
    std::fs::write(format!("{out_dir}/wire.rs"), emit_wire_schema(&meta, &seen)).unwrap();
    std::fs::write(format!("{out_dir}/mod.rs"), &root).unwrap();

    eprintln!(
        "wrote {} modules to {out_dir}: {structs} structs, {enums} enums, {choices} choices \
         from a {}-type closure",
        modules.len(),
        seen.len()
    );
}

fn emit_wire_schema(meta: &Metadata, ids: &BTreeSet<u32>) -> String {
    let mut out = String::from(
        "#![allow(clippy::unreadable_literal)]\n\n\
         use crate::metadata::{IntegerRange, StaticSchema, StaticTypeShape, TypeKind};\n\n",
    );
    writeln!(
        out,
        "#[rustfmt::skip]\npub static SCHEMA: StaticSchema = StaticSchema::new({}, &[",
        meta.header.type_count
    )
    .unwrap();
    for &id in ids {
        let shape = meta.shape(id).unwrap();
        let name = meta.type_metadata(id).unwrap().name;
        writeln!(out, "    StaticTypeShape {{").unwrap();
        writeln!(out, "        type_id: {id},").unwrap();
        writeln!(out, "        name: {name:?},").unwrap();
        writeln!(out, "        kind: TypeKind::{:?},", shape.kind).unwrap();
        writeln!(out, "        implicit_indices: {},", shape.implicit_indices).unwrap();
        writeln!(out, "        obfuscated: {},", shape.obfuscated).unwrap();
        match shape.value_range {
            Some(range) => writeln!(
                out,
                "        value_range: Some(IntegerRange {{ encoding: {}, bit_width: {:?}, control_flag: {}, minimum: {}, maximum: {} }}),",
                range.encoding,
                range.bit_width,
                range.control_flag,
                range.minimum,
                range.maximum
            )
            .unwrap(),
            None => writeln!(out, "        value_range: None,").unwrap(),
        }
        writeln!(out, "        element_type: {:?},", shape.element_type).unwrap();
        writeln!(out, "        index_values: &{:?},", shape.index_values).unwrap();
        writeln!(out, "        member_types: &{:?},", shape.member_types).unwrap();
        writeln!(out, "        member_names: &{:?},", shape.member_names).unwrap();
        writeln!(out, "    }},").unwrap();
    }
    out.push_str("]);\n");
    out
}

fn file_header() -> String {
    String::from(
        "#![allow(dead_code, unused_imports, clippy::all)]\n\n\
         use bsn_derive::FromBsn;\n\
         use superiority_core::bsn::{BsnBitArray, Bytes, FourCc};\n\n",
    )
}

fn emit_struct(out: &mut String, meta: &Metadata, id: u32, shape: &TypeShape) {
    let name = type_name(meta, id);
    writeln!(out, "#[derive(Clone, Debug, FromBsn)]").unwrap();
    writeln!(out, "pub struct {name} {{").unwrap();
    let mut used = HashSet::new();
    for ((index, member), member_name) in shape
        .index_values
        .iter()
        .zip(&shape.member_types)
        .zip(&shape.member_names)
    {
        let ty = field_type(meta, *member);
        match member_name {
            Some(wire) => {
                let ident = unique(&mut used, field_ident(wire));
                writeln!(out, "    #[bsn(name = \"{wire}\")]").unwrap();
                writeln!(out, "    pub {ident}: {ty},").unwrap();
            }
            None => {
                let ident = unique(&mut used, format!("field_{index}").replace('-', "neg"));
                writeln!(out, "    #[bsn(index = {index})]").unwrap();
                writeln!(out, "    pub {ident}: {ty},").unwrap();
            }
        }
    }
    writeln!(out, "}}\n").unwrap();
}

fn emit_enum(out: &mut String, meta: &Metadata, id: u32, shape: &TypeShape) {
    let name = type_name(meta, id);
    let mut used = HashSet::new();
    let variants: Vec<(i128, String)> = shape
        .index_values
        .iter()
        .zip(&shape.member_names)
        .map(|(index, member_name)| {
            let ident = variant_ident(member_name.as_deref().unwrap_or("Value"));
            (*index, unique(&mut used, ident))
        })
        .collect();

    writeln!(out, "#[derive(Clone, Copy, Debug, PartialEq, Eq)]").unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();
    for (_, ident) in &variants {
        writeln!(out, "    {ident},").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out, "impl superiority_core::bsn::FromBsn for {name} {{").unwrap();
    writeln!(
        out,
        "    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {{"
    )
    .unwrap();
    writeln!(
        out,
        "        match superiority_core::bsn::FromBsn::from_bsn(value)? {{"
    )
    .unwrap();
    for (index, ident) in &variants {
        writeln!(out, "            {index}i128 => Ok(Self::{ident}),").unwrap();
    }
    writeln!(
        out,
        "            other => Err(superiority_core::Error::BsnWire(format!(\"{{other}} is not a valid {name}\"))),"
    )
    .unwrap();
    writeln!(out, "        }}\n    }}\n}}\n").unwrap();
}

fn emit_choice(out: &mut String, meta: &Metadata, id: u32, shape: &TypeShape) {
    let name = type_name(meta, id);
    let mut used = HashSet::new();
    let variants: Vec<(i128, String, String)> = shape
        .index_values
        .iter()
        .zip(&shape.member_names)
        .zip(&shape.member_types)
        .map(|((index, member_name), member)| {
            let ident = variant_ident(member_name.as_deref().unwrap_or("Value"));
            (*index, unique(&mut used, ident), field_type(meta, *member))
        })
        .collect();

    writeln!(out, "#[derive(Clone, Debug)]").unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();
    for (_, ident, payload) in &variants {
        writeln!(out, "    {ident}({payload}),").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out, "impl superiority_core::bsn::FromBsn for {name} {{").unwrap();
    writeln!(
        out,
        "    fn from_bsn(value: &superiority_core::bsn::value::BsnValue) -> superiority_core::Result<Self> {{"
    )
    .unwrap();
    writeln!(out, "        let (index, inner) = match value {{").unwrap();
    writeln!(
        out,
        "            superiority_core::bsn::value::BsnValue::Choice {{ index, value }} => (*index, value.as_ref()),"
    )
    .unwrap();
    writeln!(
        out,
        "            other => return Err(superiority_core::Error::BsnWire(format!(\"expected a choice for {name}, found {{other:?}}\"))),"
    )
    .unwrap();
    writeln!(out, "        }};").unwrap();
    writeln!(out, "        match index {{").unwrap();
    for (index, ident, payload) in &variants {
        writeln!(
            out,
            "            {index}i128 => Ok(Self::{ident}(<{payload} as superiority_core::bsn::FromBsn>::from_bsn(inner)?)),"
        )
        .unwrap();
    }
    writeln!(
        out,
        "            other => Err(superiority_core::Error::BsnWire(format!(\"{{other}} is not a {name} variant\"))),"
    )
    .unwrap();
    writeln!(out, "        }}\n    }}\n}}\n").unwrap();
}

/// the Rust type expression for a field. Named types are referenced through the
/// schema root so cross-module fields resolve: `super::<module>::<Name>`.
fn field_type(meta: &Metadata, id: u32) -> String {
    let shape = meta.shape(id).unwrap();
    match shape.kind {
        TypeKind::Alias => field_type(meta, shape.element_type.unwrap()),
        TypeKind::Array => format!("Vec<{}>", field_type(meta, shape.element_type.unwrap())),
        TypeKind::Optional => format!("Option<{}>", field_type(meta, shape.element_type.unwrap())),
        TypeKind::Struct | TypeKind::Choice | TypeKind::Enum => {
            format!("super::{}::{}", module_of(meta, id), type_name(meta, id))
        }
        TypeKind::Integer => int_type(shape.value_range),
        TypeKind::Bool => "bool".to_string(),
        TypeKind::String => "String".to_string(),
        TypeKind::ByteString | TypeKind::Blob => "Bytes".to_string(),
        TypeKind::BitArray => "BsnBitArray".to_string(),
        TypeKind::FourCc => "FourCc".to_string(),
        TypeKind::Float32 => "f32".to_string(),
        TypeKind::Float64 => "f64".to_string(),
        TypeKind::Void => "()".to_string(),
    }
}

fn int_type(range: Option<IntegerRange>) -> String {
    let Some(range) = range else {
        return "i128".to_string();
    };
    let (min, max) = (range.minimum, range.maximum);
    let name = if min >= 0 {
        if max <= i128::from(u8::MAX) {
            "u8"
        } else if max <= i128::from(u16::MAX) {
            "u16"
        } else if max <= i128::from(u32::MAX) {
            "u32"
        } else if max <= i128::from(u64::MAX) {
            "u64"
        } else {
            "u128"
        }
    } else if min >= i128::from(i8::MIN) && max <= i128::from(i8::MAX) {
        "i8"
    } else if min >= i128::from(i16::MIN) && max <= i128::from(i16::MAX) {
        "i16"
    } else if min >= i128::from(i32::MIN) && max <= i128::from(i32::MAX) {
        "i32"
    } else if min >= i128::from(i64::MIN) && max <= i128::from(i64::MAX) {
        "i64"
    } else {
        "i128"
    };
    name.to_string()
}

/// the BSN domain a type belongs to — the file it lands in.
fn module_of(meta: &Metadata, id: u32) -> String {
    let raw = raw_name(meta, id);
    let stripped = raw.strip_prefix("Battlenet::").unwrap_or(&raw);
    let segments: Vec<&str> = stripped.split("::").collect();
    let domain = if segments.first() == Some(&"Client") && segments.len() >= 2 {
        segments[1]
    } else {
        segments.first().copied().unwrap_or("misc")
    };
    let module: String = domain
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if module.is_empty() || module.chars().next().unwrap().is_ascii_digit() {
        format!("m_{module}")
    } else {
        module
    }
}

fn raw_name(meta: &Metadata, id: u32) -> String {
    meta.type_metadata(id)
        .ok()
        .and_then(|info| info.name)
        .unwrap_or_else(|| format!("Type{id}"))
}

fn type_name(meta: &Metadata, id: u32) -> String {
    let raw = raw_name(meta, id);
    to_pascal(raw.strip_prefix("Battlenet::").unwrap_or(&raw))
}

fn field_ident(name: &str) -> String {
    sanitize_ident(&to_snake(name.strip_prefix("m_").unwrap_or(name)))
}

fn variant_ident(name: &str) -> String {
    let ident = to_pascal(name);
    if ident.is_empty() {
        "Value".to_string()
    } else if KEYWORDS.contains(&ident.as_str()) {
        format!("{ident}_")
    } else {
        ident
    }
}

fn to_snake(input: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in input.chars() {
        if ch == '_' || ch == ':' {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        } else if ch.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = false;
        } else {
            out.push(ch);
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

fn to_pascal(input: &str) -> String {
    let mut out = String::new();
    for segment in input.split("::").flat_map(|part| part.split('_')) {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn sanitize_ident(input: &str) -> String {
    let mut cleaned: String = input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    if cleaned.is_empty() {
        cleaned = "field".to_string();
    }
    if cleaned.chars().next().unwrap().is_ascii_digit() {
        cleaned = format!("f_{cleaned}");
    }
    if KEYWORDS.contains(&cleaned.as_str()) {
        cleaned.push('_');
    }
    cleaned
}

fn unique(used: &mut HashSet<String>, mut ident: String) -> String {
    if used.insert(ident.clone()) {
        return ident;
    }
    for suffix in 2.. {
        let candidate = format!("{ident}_{suffix}");
        if used.insert(candidate.clone()) {
            ident = candidate;
            break;
        }
    }
    ident
}
