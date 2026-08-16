use crate::{
    Error, Result,
    bsn::{
        bits::{BitReader, BitWriter},
        codec::{Codec, DecodedField, StructWireLayout, WireField, WireLayout},
        value::{BsnField, BsnStruct, BsnValue},
    },
};

const VERIFIED_REFLECTED: &[&str] = &[
    "Battlenet::Client::Achievement::ListenRequest",
    "Battlenet::Client::Chat::StatusChangeRequest",
    "Battlenet::Client::Chat::DatagramConnectionUpdate",
    "Battlenet::Client::Chat::WhisperRecv",
    "Battlenet::Client::Chat::WhisperEchoRecv",
    "Battlenet::Client::Connection::ServerVersion",
    "Battlenet::Client::Connection::RegulatorUpdate",
    "Battlenet::Client::Party::BeginReadyProcess",
    "Battlenet::Client::Party::MapOptionsChange",
    "Battlenet::Client::Party::ReadyProcessUpdate",
    "Battlenet::Client::Presence::StatisticsUpdate",
    "Battlenet::Client::Presence::TemporaryPresenceResponse",
    "Battlenet::Client::Profile::SendStatsUIEvent",
    "Battlenet::Client::S2Master::MMQGetInfoRequest",
    "Battlenet::Client::S2Master::MMQGetListResponse",
    "Battlenet::Client::Toon::InitialNotifiesComplete",
];

const CANDIDATE_REFLECTED: &[&str] = &["Battlenet::Client::Toon::CaisTimeUpdate"];

const IDENTITY_0: &[WireField] = &[];
const IDENTITY_1: &[WireField] = &[WireField::new(0, 0)];
const IDENTITY_2: &[WireField] = &[WireField::new(0, 0), WireField::new(1, 0)];
const IDENTITY_3: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
];
const IDENTITY_4: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(1, 0),
    WireField::new(2, 0),
    WireField::new(3, 0),
];
const TOON_HANDLE: &[WireField] = &[
    WireField::new(1, 0),
    WireField::new(0, 0),
    WireField::new(2, 0),
    WireField::new(3, 0),
];
const CLUB_INVITE_ACTION: &[WireField] = &[
    WireField::new(2, 0),
    WireField::new(1, 0),
    WireField::new(0, 0),
    WireField::new(3, 11),
];
const MODIFY_CHANNEL_LIST: &[WireField] = &[
    WireField::new(0, 0),
    WireField::new(3, 0),
    WireField::new(2, 4),
    WireField::new(1, 0),
];
const ACHIEVEMENT_CRITERIA_UPDATE: &[WireField] = &[
    WireField::new(1, 0),
    WireField::new(3, 0),
    WireField::new(0, 0),
    WireField::new(4, 0),
    WireField::new(2, 0),
];
const ACHIEVEMENT_PERSISTENT_RECORD: &[WireField] = &[
    WireField::new(2, 0),
    WireField::new(0, 0),
    WireField::new(1, 0),
];
const CHAT_CATEGORY_DESCRIPTIONS: &[WireField] = &[WireField::new(1, 0), WireField::new(0, 0)];
const CLUB_NAME: &[WireField] = &[WireField::new(0, 16)];
const BILLING_INFO: &[WireField] = &[
    WireField::new(2, 0),
    WireField::new(1, 19),
    WireField::new(0, 28),
    WireField::new(3, 0),
];

pub(super) fn register(codec: &mut Codec) -> Result<()> {
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Map::S2ListMapFavorites",
        StructWireLayout::new("empty S2Map::S2ListMapFavorites", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Club::InviteAction",
        StructWireLayout::new("identity Client::Club::InviteAction", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Achievement::Data",
        StructWireLayout::new("identity Client::Achievement::Data", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Achievement::PersistentRecord",
        StructWireLayout::new(
            "generated Achievement::PersistentRecord",
            ACHIEVEMENT_PERSISTENT_RECORD,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Achievement::QuestUpdateRecord",
        StructWireLayout::new("identity Achievement::QuestUpdateRecord", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Achievement::CriteriaUpdateRecord",
        StructWireLayout::new(
            "generated Achievement::CriteriaUpdateRecord",
            ACHIEVEMENT_CRITERIA_UPDATE,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Club::InviteAction",
        StructWireLayout::new("generated Club::InviteAction", CLUB_INVITE_ACTION),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ModifyChannelListRequest",
        StructWireLayout::new(
            "generated Chat::ModifyChannelListRequest",
            MODIFY_CHANNEL_LIST,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ModifyChannelListRequest2",
        StructWireLayout::new("identity Chat::ModifyChannelListRequest2", IDENTITY_4),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::ModifyChannelListResponse2",
        StructWireLayout::new("identity Chat::ModifyChannelListResponse2", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Chat::CategoryDescriptions",
        StructWireLayout::new(
            "generated Chat::CategoryDescriptions",
            CHAT_CATEGORY_DESCRIPTIONS,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Map::S2ListMapFavoritesRequest",
        StructWireLayout::new("identity S2Map::S2ListMapFavoritesRequest", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::S2Map::S2ListMapFavoritesResponse",
        StructWireLayout::new("identity S2Map::S2ListMapFavoritesResponse", IDENTITY_2),
    )?;
    codec.register_wire_layout(
        "Battlenet::Conference::CategoryDescription",
        WireLayout::new_traced(
            "generated Conference::CategoryDescription",
            decode_category_description,
            decode_category_description_traced,
            encode_category_description,
        ),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::PublicPartialName",
        StructWireLayout::new("identity Conference::PublicPartialName", IDENTITY_2),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Conference::ClubName",
        StructWireLayout::new("generated Conference::ClubName", CLUB_NAME),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Toon::Handle",
        StructWireLayout::new("generated Toon::Handle", TOON_HANDLE),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::BillingUpdateNotify",
        StructWireLayout::new("identity Toon::BillingUpdateNotify", IDENTITY_1),
    )?;
    register_session_and_toon_layouts(codec)?;
    for name in VERIFIED_REFLECTED {
        codec.register_verified_reflected(name)?;
    }
    for name in CANDIDATE_REFLECTED {
        codec.register_candidate_reflected(name)?;
    }
    Ok(())
}

fn register_session_and_toon_layouts(codec: &mut Codec) -> Result<()> {
    codec.register_struct_wire_layout(
        "Battlenet::Session::BillingInfo",
        StructWireLayout::new("generated Session::BillingInfo", BILLING_INFO),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateInit",
        StructWireLayout::new("empty Toon::ToonCreateInit", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateSetup",
        StructWireLayout::new("empty Toon::ToonCreateSetup", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateFinal",
        StructWireLayout::new("identity Toon::ToonCreateFinal", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Toon::ToonCreationData",
        StructWireLayout::new("identity Toon::ToonCreationData", IDENTITY_1),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreateCancel",
        StructWireLayout::new("empty Toon::ToonCreateCancel", IDENTITY_0),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::ToonCreated",
        StructWireLayout::new("identity Toon::ToonCreated", IDENTITY_3),
    )?;
    codec.register_struct_wire_layout(
        "Battlenet::Client::Toon::Failure",
        StructWireLayout::new("identity Toon::Failure", IDENTITY_1),
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct CategoryDescriptionValues {
    start_bit: usize,
    end_bit: usize,
    name: Spanned<i128>,
    generated_start_bit: usize,
    generated_end_bit: usize,
    id: Spanned<i128>,
    sort_order: Spanned<i128>,
}

#[derive(Clone, Debug)]
struct Spanned<T> {
    value: T,
    start_bit: usize,
    end_bit: usize,
}

fn decode_category_description(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<BsnValue> {
    let values = read_category_description(reader)?;
    build_category_description(codec, root_type, &values)
}

fn decode_category_description_traced(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
) -> Result<(BsnValue, Vec<DecodedField>)> {
    let values = read_category_description(reader)?;
    let fields = vec![
        DecodedField {
            path: path.to_owned(),
            kind: "struct",
            value: "3 fields".to_owned(),
            start_bit: values.start_bit,
            end_bit: values.end_bit,
            depth,
        },
        traced_integer(format!("{path}.m_name"), "uint32", &values.name, depth + 1),
        DecodedField {
            path: format!("{path}.generated"),
            kind: "generated fields",
            value: "ignored".to_owned(),
            start_bit: values.generated_start_bit,
            end_bit: values.generated_end_bit,
            depth: depth + 1,
        },
        traced_integer(format!("{path}.m_id"), "uint8", &values.id, depth + 1),
        traced_integer(
            format!("{path}.m_sortOrder"),
            "uint16",
            &values.sort_order,
            depth + 1,
        ),
    ];
    Ok((
        build_category_description(codec, root_type, &values)?,
        fields,
    ))
}

fn read_category_description(reader: &mut BitReader<'_>) -> Result<CategoryDescriptionValues> {
    let start_bit = reader.position();
    let name = read_spanned(reader, 32, i128::from)?;
    let generated_start_bit = reader.position();
    read_category_generated_state(reader)?;
    read_category_generated_variant(reader)?;
    let generated_end_bit = reader.position();
    let id = read_spanned(reader, 8, i128::from)?;
    let sort_order = read_spanned(reader, 16, i128::from)?;
    Ok(CategoryDescriptionValues {
        start_bit,
        end_bit: reader.position(),
        name,
        generated_start_bit,
        generated_end_bit,
        id,
        sort_order,
    })
}

fn read_category_generated_state(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(8)?;
    reader.read(16)?;
    read_bounded_u32_array(reader)?;
    reader.read(32)?;
    read_bounded_u32_array(reader)?;
    reader.read(32)?;
    Ok(())
}

fn read_bounded_u32_array(reader: &mut BitReader<'_>) -> Result<()> {
    let count = usize::try_from(reader.read(3)?).expect("3-bit value fits in usize");
    if count >= 5 {
        return Err(wire_error(format!(
            "generated category array count {count} exceeds four"
        )));
    }
    for _ in 0..count {
        reader.read(32)?;
    }
    Ok(())
}

fn read_category_generated_variant(reader: &mut BitReader<'_>) -> Result<()> {
    reader.read(16)?;
    reader.read(29)?;
    match reader.read(2)? {
        0 => {
            let length = usize::try_from(reader.read(7)?).expect("7-bit value fits in usize");
            if length >= 125 {
                return Err(wire_error(format!(
                    "generated category string length {length} exceeds 124"
                )));
            }
            reader.read_bytes(length, true)?;
        }
        2 => {
            reader.read(32)?;
            reader.read(16)?;
        }
        3 => {
            reader.read(16)?;
            reader.read(32)?;
        }
        tag => {
            return Err(wire_error(format!(
                "unsupported generated category variant {tag}"
            )));
        }
    }
    Ok(())
}

fn build_category_description(
    codec: &Codec,
    root_type: u32,
    values: &CategoryDescriptionValues,
) -> Result<BsnValue> {
    let shape = codec.schema().shape(root_type)?;
    let logical_values = [
        ("m_id", BsnValue::Integer(values.id.value)),
        ("m_name", BsnValue::Integer(values.name.value)),
        ("m_sortOrder", BsnValue::Integer(values.sort_order.value)),
    ];
    let mut fields = Vec::with_capacity(logical_values.len());
    for (name, value) in logical_values {
        let position = shape
            .member_names
            .iter()
            .position(|candidate| candidate.as_deref() == Some(name))
            .ok_or_else(|| wire_error(format!("category metadata omits field {name}")))?;
        fields.push(BsnField {
            index: shape.index_values[position],
            name: shape.member_names[position].clone(),
            value,
        });
    }
    Ok(BsnValue::Struct(BsnStruct::new(root_type, fields)))
}

fn encode_category_description(
    _codec: &Codec,
    _root_type: u32,
    _writer: &mut BitWriter,
    _value: &BsnValue,
) -> Result<()> {
    Err(wire_error(
        "generated category descriptions are inbound-only",
    ))
}

fn read_spanned<T>(
    reader: &mut BitReader<'_>,
    width: usize,
    convert: impl FnOnce(u64) -> T,
) -> Result<Spanned<T>> {
    let start_bit = reader.position();
    let value = convert(reader.read(width)?);
    Ok(Spanned {
        value,
        start_bit,
        end_bit: reader.position(),
    })
}

fn traced_integer(
    path: String,
    kind: &'static str,
    value: &Spanned<i128>,
    depth: usize,
) -> DecodedField {
    DecodedField {
        path,
        kind,
        value: value.value.to_string(),
        start_bit: value.start_bit,
        end_bit: value.end_bit,
        depth,
    }
}

fn wire_error(message: impl Into<String>) -> Error {
    Error::BsnWire(message.into())
}
