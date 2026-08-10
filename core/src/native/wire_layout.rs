use crate::{
    Error, Result,
    bsn::{
        bits::{BitReader, BitWriter},
        codec::{Codec, WireLayout},
        value::{BsnField, BsnStruct, BsnValue},
    },
    metadata::{TypeKind, TypeShape},
};

const CLUB_INVITE_ACTION: &str = "Battlenet::Client::Club::InviteAction";
const INVITE_RESERVED: u64 = 4;

const VERIFIED_REFLECTED: &[&str] = &[
    "Battlenet::Client::Chat::WhisperRecv",
    "Battlenet::Client::Chat::WhisperEchoRecv",
    "Battlenet::Client::Connection::ServerVersion",
    "Battlenet::Client::Connection::RegulatorUpdate",
    "Battlenet::Client::Presence::StatisticsUpdate",
    "Battlenet::Client::Presence::TemporaryPresenceResponse",
    "Battlenet::Client::Toon::InitialNotifiesComplete",
];

pub(super) fn register(codec: &mut Codec) -> Result<()> {
    codec.register_wire_layout(
        CLUB_INVITE_ACTION,
        WireLayout::new(
            "generated Club::InviteAction",
            decode_club_invite_action,
            encode_club_invite_action,
        ),
    )?;
    for name in VERIFIED_REFLECTED {
        codec.register_verified_reflected(name)?;
    }
    Ok(())
}

fn decode_club_invite_action(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
) -> Result<BsnValue> {
    let root_type = peel_alias(codec, root_type)?;
    let action_type = peel_alias(codec, member_type(codec, root_type, "m_action")?)?;
    let member_type = peel_alias(codec, member_type(codec, action_type, "m_member")?)?;

    let code = i128::from(reader.read(2)?);
    let program = u32::try_from(reader.read(32)?).expect("32-bit field fits in u32");
    let region = i128::from(reader.read(8)?);
    let realm = i128::from(reader.read(32)?);
    let id = i128::from(reader.read(64)?);
    let club_id = i128::from(reader.read(32)?);
    reader.read(11)?;
    let result = i128::from(reader.read(16)?);

    let member = BsnValue::Struct(BsnStruct::new(
        member_type,
        vec![
            named_field(codec, member_type, "m_region", BsnValue::Integer(region))?,
            named_field(codec, member_type, "m_programId", BsnValue::FourCc(program))?,
            named_field(codec, member_type, "m_realm", BsnValue::Integer(realm))?,
            named_field(codec, member_type, "m_id", BsnValue::Integer(id))?,
        ],
    ));
    let action = BsnValue::Struct(BsnStruct::new(
        action_type,
        vec![
            named_field(codec, action_type, "m_clubId", BsnValue::Integer(club_id))?,
            named_field(codec, action_type, "m_member", member)?,
            named_field(codec, action_type, "m_code", BsnValue::Integer(code))?,
            named_field(codec, action_type, "m_result", BsnValue::Integer(result))?,
        ],
    ));
    Ok(BsnValue::Struct(BsnStruct::new(
        root_type,
        vec![named_field(codec, root_type, "m_action", action)?],
    )))
}

fn encode_club_invite_action(
    _codec: &Codec,
    _root_type: u32,
    writer: &mut BitWriter,
    value: &BsnValue,
) -> Result<()> {
    let root = expect_struct(value, "client club invite")?;
    let action = expect_struct(required_field(root, "m_action")?, "club invite action")?;
    let member = expect_struct(required_field(action, "m_member")?, "club invite member")?;

    writer.write(
        unsigned(required_field(action, "m_code")?, 2, "invite code")?,
        2,
    )?;
    writer.write(
        fourcc(required_field(member, "m_programId")?, "invite program")?,
        32,
    )?;
    writer.write(
        unsigned(
            required_field(member, "m_region")?,
            8,
            "invite member region",
        )?,
        8,
    )?;
    writer.write(
        unsigned(
            required_field(member, "m_realm")?,
            32,
            "invite member realm",
        )?,
        32,
    )?;
    writer.write(
        unsigned(required_field(member, "m_id")?, 64, "invite member id")?,
        64,
    )?;
    writer.write(
        unsigned(required_field(action, "m_clubId")?, 32, "invite club id")?,
        32,
    )?;
    writer.write(INVITE_RESERVED, 11)?;
    writer.write(
        unsigned(required_field(action, "m_result")?, 16, "invite result")?,
        16,
    )
}

fn peel_alias(codec: &Codec, mut type_id: u32) -> Result<u32> {
    loop {
        let shape = codec.schema().shape(type_id)?;
        if shape.kind != TypeKind::Alias {
            return Ok(type_id);
        }
        type_id = shape
            .element_type
            .ok_or_else(|| wire_error(format!("alias type {type_id} has no element")))?;
    }
}

fn struct_shape(codec: &Codec, type_id: u32) -> Result<TypeShape> {
    let shape = codec.schema().shape(type_id)?;
    if shape.kind != TypeKind::Struct {
        return Err(wire_error(format!("type {type_id} is not a struct")));
    }
    Ok(shape)
}

fn member_position(shape: &TypeShape, name: &str) -> Result<usize> {
    shape
        .member_names
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name))
        .ok_or_else(|| wire_error(format!("wire-layout metadata omits field {name}")))
}

fn member_type(codec: &Codec, struct_type: u32, name: &str) -> Result<u32> {
    let shape = struct_shape(codec, struct_type)?;
    Ok(shape.member_types[member_position(&shape, name)?])
}

fn named_field(codec: &Codec, struct_type: u32, name: &str, value: BsnValue) -> Result<BsnField> {
    let shape = struct_shape(codec, struct_type)?;
    let position = member_position(&shape, name)?;
    Ok(BsnField {
        index: shape.index_values[position],
        name: shape.member_names[position].clone(),
        value,
    })
}

fn expect_struct<'value>(value: &'value BsnValue, label: &str) -> Result<&'value BsnStruct> {
    value
        .as_struct()
        .ok_or_else(|| wire_error(format!("{label} is not a struct")))
}

fn required_field<'value>(value: &'value BsnStruct, name: &str) -> Result<&'value BsnValue> {
    value
        .get(name)
        .ok_or_else(|| wire_error(format!("wire-layout value omits field {name}")))
}

fn unsigned(value: &BsnValue, bits: usize, label: &str) -> Result<u64> {
    let BsnValue::Integer(value) = value else {
        return Err(wire_error(format!("{label} is not an integer")));
    };
    let value = u64::try_from(*value)
        .map_err(|_| wire_error(format!("{label} is outside an unsigned {bits}-bit value")))?;
    if bits < 64 && value >= 1_u64 << bits {
        return Err(wire_error(format!(
            "{label} is outside an unsigned {bits}-bit value"
        )));
    }
    Ok(value)
}

fn fourcc(value: &BsnValue, label: &str) -> Result<u64> {
    let BsnValue::FourCc(value) = value else {
        return Err(wire_error(format!("{label} is not a FourCC")));
    };
    Ok(u64::from(*value))
}

fn wire_error(message: impl Into<String>) -> Error {
    Error::BsnWire(message.into())
}
