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
    "Battlenet::Client::Chat::StatusChangeRequest",
    "Battlenet::Client::Chat::DatagramConnectionUpdate",
    "Battlenet::Client::Chat::WhisperRecv",
    "Battlenet::Client::Chat::WhisperEchoRecv",
    "Battlenet::Client::Connection::ServerVersion",
    "Battlenet::Client::Connection::RegulatorUpdate",
    "Battlenet::Client::Party::BeginReadyProcess",
    "Battlenet::Client::Party::MapOptionsChange",
    "Battlenet::Client::Presence::StatisticsUpdate",
    "Battlenet::Client::Presence::TemporaryPresenceResponse",
    "Battlenet::Client::Toon::InitialNotifiesComplete",
];

pub(super) fn register(codec: &mut Codec) -> Result<()> {
    codec.register_wire_layout(
        CLUB_INVITE_ACTION,
        WireLayout::new_traced(
            "generated Club::InviteAction",
            decode_club_invite_action,
            decode_club_invite_action_traced,
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
    let values = read_club_invite_action(reader)?;
    build_club_invite_action(codec, root_type, &values)
}

fn decode_club_invite_action_traced(
    codec: &Codec,
    root_type: u32,
    reader: &mut BitReader<'_>,
    path: &str,
    depth: usize,
) -> Result<(BsnValue, Vec<crate::bsn::codec::DecodedField>)> {
    let values = read_club_invite_action(reader)?;
    let fields = values.fields(path, depth);
    Ok((build_club_invite_action(codec, root_type, &values)?, fields))
}

#[derive(Clone, Debug)]
struct Spanned<T> {
    value: T,
    start_bit: usize,
    end_bit: usize,
}

#[derive(Clone, Debug)]
struct ClubInviteActionValues {
    start_bit: usize,
    end_bit: usize,
    code: Spanned<i128>,
    program: Spanned<u32>,
    region: Spanned<i128>,
    realm: Spanned<i128>,
    id: Spanned<i128>,
    club_id: Spanned<i128>,
    reserved: Spanned<u64>,
    result: Spanned<i128>,
}

impl ClubInviteActionValues {
    fn fields(&self, path: &str, depth: usize) -> Vec<crate::bsn::codec::DecodedField> {
        use crate::bsn::codec::DecodedField;

        let action = format!("{path}.m_action");
        let member = format!("{action}.m_member");
        let mut fields = vec![
            DecodedField {
                path: action.clone(),
                kind: "struct",
                value: "4 fields".to_owned(),
                start_bit: self.start_bit,
                end_bit: self.end_bit,
                depth: depth + 1,
            },
            DecodedField {
                path: member.clone(),
                kind: "struct",
                value: "4 fields".to_owned(),
                start_bit: self.program.start_bit,
                end_bit: self.id.end_bit,
                depth: depth + 2,
            },
        ];
        fields.extend([
            traced_integer(
                format!("{action}.m_code"),
                "uint2",
                &self.code.value,
                &self.code,
                depth + 2,
            ),
            crate::bsn::codec::DecodedField {
                path: format!("{member}.m_programId"),
                kind: "fourcc",
                value: format!("0x{:08x}", self.program.value),
                start_bit: self.program.start_bit,
                end_bit: self.program.end_bit,
                depth: depth + 3,
            },
            traced_integer(
                format!("{member}.m_region"),
                "uint8",
                &self.region.value,
                &self.region,
                depth + 3,
            ),
            traced_integer(
                format!("{member}.m_realm"),
                "uint32",
                &self.realm.value,
                &self.realm,
                depth + 3,
            ),
            traced_integer(
                format!("{member}.m_id"),
                "uint64",
                &self.id.value,
                &self.id,
                depth + 3,
            ),
            traced_integer(
                format!("{action}.m_clubId"),
                "uint32",
                &self.club_id.value,
                &self.club_id,
                depth + 2,
            ),
            DecodedField {
                path: format!("{action}.reserved"),
                kind: "reserved bits",
                value: self.reserved.value.to_string(),
                start_bit: self.reserved.start_bit,
                end_bit: self.reserved.end_bit,
                depth: depth + 2,
            },
            traced_integer(
                format!("{action}.m_result"),
                "uint16",
                &self.result.value,
                &self.result,
                depth + 2,
            ),
        ]);
        fields
    }
}

fn traced_integer<T: ToString>(
    path: String,
    kind: &'static str,
    value: &T,
    span: &Spanned<T>,
    depth: usize,
) -> crate::bsn::codec::DecodedField {
    crate::bsn::codec::DecodedField {
        path,
        kind,
        value: value.to_string(),
        start_bit: span.start_bit,
        end_bit: span.end_bit,
        depth,
    }
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

fn read_club_invite_action(reader: &mut BitReader<'_>) -> Result<ClubInviteActionValues> {
    let start_bit = reader.position();
    let code = read_spanned(reader, 2, i128::from)?;
    let program = read_spanned(reader, 32, |value| {
        u32::try_from(value).expect("32-bit field fits in u32")
    })?;
    let region = read_spanned(reader, 8, i128::from)?;
    let realm = read_spanned(reader, 32, i128::from)?;
    let id = read_spanned(reader, 64, i128::from)?;
    let club_id = read_spanned(reader, 32, i128::from)?;
    let reserved = read_spanned(reader, 11, |value| value)?;
    let result = read_spanned(reader, 16, i128::from)?;
    Ok(ClubInviteActionValues {
        start_bit,
        end_bit: reader.position(),
        code,
        program,
        region,
        realm,
        id,
        club_id,
        reserved,
        result,
    })
}

fn build_club_invite_action(
    codec: &Codec,
    root_type: u32,
    values: &ClubInviteActionValues,
) -> Result<BsnValue> {
    let root_type = peel_alias(codec, root_type)?;
    let action_type = peel_alias(codec, member_type(codec, root_type, "m_action")?)?;
    let member_type = peel_alias(codec, member_type(codec, action_type, "m_member")?)?;

    let member = BsnValue::Struct(BsnStruct::new(
        member_type,
        vec![
            named_field(
                codec,
                member_type,
                "m_region",
                BsnValue::Integer(values.region.value),
            )?,
            named_field(
                codec,
                member_type,
                "m_programId",
                BsnValue::FourCc(values.program.value),
            )?,
            named_field(
                codec,
                member_type,
                "m_realm",
                BsnValue::Integer(values.realm.value),
            )?,
            named_field(
                codec,
                member_type,
                "m_id",
                BsnValue::Integer(values.id.value),
            )?,
        ],
    ));
    let action = BsnValue::Struct(BsnStruct::new(
        action_type,
        vec![
            named_field(
                codec,
                action_type,
                "m_clubId",
                BsnValue::Integer(values.club_id.value),
            )?,
            named_field(codec, action_type, "m_member", member)?,
            named_field(
                codec,
                action_type,
                "m_code",
                BsnValue::Integer(values.code.value),
            )?,
            named_field(
                codec,
                action_type,
                "m_result",
                BsnValue::Integer(values.result.value),
            )?,
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
