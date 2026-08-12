use std::collections::BTreeMap;

use crate::{
    Error, Result,
    bsn::{
        bits::{BitReader, BitWriter},
        value::{BsnBitArray, BsnField, BsnStruct, BsnValue},
    },
    metadata::{IntegerRange, Metadata, Schema, TypeKind, TypeShape},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedValue {
    pub data: Vec<u8>,
    pub bit_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedValue {
    pub value: BsnValue,
    pub bit_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedField {
    pub path: String,
    pub kind: &'static str,
    pub value: String,
    pub start_bit: usize,
    pub end_bit: usize,
    pub depth: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TracedDecodedValue {
    pub value: BsnValue,
    pub bit_count: usize,
    pub fields: Vec<DecodedField>,
}

pub type DecodeWireLayout = for<'data> fn(&Codec, u32, &mut BitReader<'data>) -> Result<BsnValue>;
pub type DecodeTracedWireLayout = for<'data> fn(
    &Codec,
    u32,
    &mut BitReader<'data>,
    &str,
    usize,
) -> Result<(BsnValue, Vec<DecodedField>)>;
pub type EncodeWireLayout = fn(&Codec, u32, &mut BitWriter, &BsnValue) -> Result<()>;

#[derive(Clone, Copy)]
pub struct WireLayout {
    name: &'static str,
    decode: DecodeWireLayout,
    decode_traced: Option<DecodeTracedWireLayout>,
    encode: EncodeWireLayout,
}

impl WireLayout {
    #[must_use]
    pub const fn new(
        name: &'static str,
        decode: DecodeWireLayout,
        encode: EncodeWireLayout,
    ) -> Self {
        Self {
            name,
            decode,
            decode_traced: None,
            encode,
        }
    }

    #[must_use]
    pub const fn new_traced(
        name: &'static str,
        decode: DecodeWireLayout,
        decode_traced: DecodeTracedWireLayout,
        encode: EncodeWireLayout,
    ) -> Self {
        Self {
            name,
            decode,
            decode_traced: Some(decode_traced),
            encode,
        }
    }
}

impl std::fmt::Debug for WireLayout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WireLayout")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
enum RegisteredWireLayout {
    VerifiedReflected,
    Custom(WireLayout),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireLayoutSupport {
    Reflected,
    VerifiedReflected,
    Custom(&'static str),
    UnsupportedObfuscated,
}

#[derive(Clone, Debug)]
pub struct Codec {
    schema: Schema,
    wire_layouts: BTreeMap<u32, RegisteredWireLayout>,
}

impl Codec {
    #[must_use]
    pub fn new(metadata: Metadata) -> Self {
        Self::from_schema(metadata.into())
    }

    #[must_use]
    pub fn from_schema(schema: Schema) -> Self {
        Self {
            schema,
            wire_layouts: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn register_wire_layout(&mut self, exact_name: &str, layout: WireLayout) -> Result<()> {
        let type_id = self.schema.unique_type_id(exact_name)?;
        self.register_obfuscated(type_id, RegisteredWireLayout::Custom(layout))
    }

    pub fn register_verified_reflected(&mut self, exact_name: &str) -> Result<()> {
        let type_id = self.schema.unique_type_id(exact_name)?;
        self.register_obfuscated(type_id, RegisteredWireLayout::VerifiedReflected)
    }

    pub fn wire_layout_support(&self, type_id: u32) -> Result<WireLayoutSupport> {
        let shape = self.schema.shape(type_id)?;
        Ok(match self.wire_layouts.get(&type_id) {
            Some(RegisteredWireLayout::VerifiedReflected) => WireLayoutSupport::VerifiedReflected,
            Some(RegisteredWireLayout::Custom(layout)) => WireLayoutSupport::Custom(layout.name),
            None if shape.obfuscated => WireLayoutSupport::UnsupportedObfuscated,
            None => WireLayoutSupport::Reflected,
        })
    }

    fn register_obfuscated(&mut self, type_id: u32, layout: RegisteredWireLayout) -> Result<()> {
        let shape = self.schema.shape(type_id)?;
        if !shape.obfuscated {
            return Err(codec_error(format!(
                "type {} (#{type_id}) is not marked Obfuscated",
                self.type_name(type_id)
            )));
        }
        if self.wire_layouts.contains_key(&type_id) {
            return Err(codec_error(format!(
                "type {} (#{type_id}) already has a wire-layout registration",
                self.type_name(type_id)
            )));
        }
        self.wire_layouts.insert(type_id, layout);
        Ok(())
    }

    pub fn encode(&self, type_id: u32, value: &BsnValue) -> Result<EncodedValue> {
        let mut writer = BitWriter::new();
        self.encode_into(&mut writer, type_id, value)?;
        let bit_count = writer.position();
        Ok(EncodedValue {
            data: writer.into_bytes(),
            bit_count,
        })
    }

    pub fn decode(
        &self,
        type_id: u32,
        data: &[u8],
        bit_count: Option<usize>,
        start_position: usize,
    ) -> Result<DecodedValue> {
        let mut reader = BitReader::new(data, bit_count)?;
        reader.set_position(start_position)?;
        let value = self.decode_from(&mut reader, type_id)?;
        Ok(DecodedValue {
            value,
            bit_count: reader.position() - start_position,
        })
    }

    #[allow(clippy::too_many_lines)]
    pub fn encode_into(
        &self,
        writer: &mut BitWriter,
        type_id: u32,
        value: &BsnValue,
    ) -> Result<()> {
        self.encode_into_scoped(writer, type_id, value, false)
    }

    pub(crate) fn encode_reflected_into(
        &self,
        writer: &mut BitWriter,
        type_id: u32,
        value: &BsnValue,
    ) -> Result<()> {
        self.encode_into_scoped(writer, type_id, value, true)
    }

    #[allow(clippy::too_many_lines)]
    fn encode_into_scoped(
        &self,
        writer: &mut BitWriter,
        type_id: u32,
        value: &BsnValue,
        reflected_scope: bool,
    ) -> Result<()> {
        let shape = self.schema.shape(type_id)?;
        if let Some(RegisteredWireLayout::Custom(layout)) = self.wire_layouts.get(&type_id) {
            return (layout.encode)(self, type_id, writer, value);
        }
        let reflected_scope = self.reflected_scope(type_id, &shape, reflected_scope)?;
        match shape.kind {
            TypeKind::Alias => {
                self.encode_into_scoped(writer, required_element(&shape)?, value, reflected_scope)
            }
            TypeKind::Void => expect_variant(value, matches!(value, BsnValue::Void), "void"),
            TypeKind::Bool => {
                let BsnValue::Bool(value) = value else {
                    return Err(codec_error("bool BSN value must be bool"));
                };
                writer.write(u64::from(*value), 1)
            }
            TypeKind::Integer | TypeKind::Enum => {
                let BsnValue::Integer(value) = value else {
                    return Err(codec_error("integer BSN value must be integer"));
                };
                if shape.kind == TypeKind::Enum && !shape.index_values.contains(value) {
                    return Err(codec_error(format!(
                        "enum value {value} is not declared by type {type_id}"
                    )));
                }
                write_range(writer, required_range(&shape)?, *value)
            }
            TypeKind::FourCc => {
                let value = match value {
                    BsnValue::FourCc(value) => *value,
                    _ => return Err(codec_error("FourCC BSN value must be FourCc")),
                };
                writer.write(u64::from(value), 32)
            }
            TypeKind::Float32 => {
                let BsnValue::Float32(value) = value else {
                    return Err(codec_error("float32 BSN value must be Float32"));
                };
                writer.write(u64::from(value.to_bits()), 32)
            }
            TypeKind::Float64 => {
                let BsnValue::Float64(value) = value else {
                    return Err(codec_error("float64 BSN value must be Float64"));
                };
                writer.write(value.to_bits(), 64)
            }
            TypeKind::ByteString | TypeKind::Blob => {
                let BsnValue::Bytes(value) = value else {
                    return Err(codec_error("binary BSN value must be bytes"));
                };
                write_length(writer, required_range(&shape)?, value.len())?;
                writer.write_bytes(value, true)
            }
            TypeKind::String => {
                let BsnValue::String(value) = value else {
                    return Err(codec_error("string BSN value must be text"));
                };
                write_length(writer, byte_counted(required_range(&shape)?), value.len())?;
                writer.write_bytes(value.as_bytes(), true)
            }
            TypeKind::BitArray => {
                let BsnValue::BitArray(value) = value else {
                    return Err(codec_error("bit-array BSN value must be a bit array"));
                };
                if value.bit_count > value.data.len() * 8 {
                    return Err(codec_error(
                        "bit-array length is outside its supplied buffer",
                    ));
                }
                write_length(writer, required_range(&shape)?, value.bit_count)?;
                writer.write_raw(&value.data, Some(value.bit_count))
            }
            TypeKind::Array => {
                let BsnValue::Array(values) = value else {
                    return Err(codec_error("array BSN value must be an array"));
                };
                write_length(writer, required_range(&shape)?, values.len())?;
                let element = required_element(&shape)?;
                for value in values {
                    self.encode_into_scoped(writer, element, value, reflected_scope)?;
                }
                Ok(())
            }
            TypeKind::Optional => {
                let BsnValue::Optional(value) = value else {
                    return Err(codec_error("optional BSN value must be optional"));
                };
                writer.write(u64::from(value.is_some()), 1)?;
                if let Some(value) = value {
                    self.encode_into_scoped(
                        writer,
                        required_element(&shape)?,
                        value,
                        reflected_scope,
                    )?;
                }
                Ok(())
            }
            TypeKind::Choice => {
                let BsnValue::Choice { index, value } = value else {
                    return Err(codec_error("choice BSN value must be a choice"));
                };
                let position = shape
                    .index_values
                    .iter()
                    .position(|candidate| candidate == index)
                    .ok_or_else(|| {
                        codec_error(format!(
                            "choice index {index} is not declared by type {type_id}"
                        ))
                    })?;
                write_range(writer, required_range(&shape)?, *index)?;
                self.encode_into_scoped(
                    writer,
                    shape.member_types[position],
                    value,
                    reflected_scope,
                )
            }
            TypeKind::Struct => self.encode_struct(writer, type_id, &shape, value, reflected_scope),
        }
    }

    pub fn decode_from(&self, reader: &mut BitReader<'_>, type_id: u32) -> Result<BsnValue> {
        self.decode_from_scoped(reader, type_id, false)
    }

    pub fn decode_traced_from(
        &self,
        reader: &mut BitReader<'_>,
        type_id: u32,
    ) -> Result<TracedDecodedValue> {
        let start_position = reader.position();
        let mut fields = Some(Vec::new());
        let value =
            self.decode_from_scoped_traced(reader, type_id, false, "value", 0, &mut fields)?;
        let mut fields = fields.unwrap_or_default();
        fields.sort_by(|left, right| {
            left.start_bit
                .cmp(&right.start_bit)
                .then_with(|| left.depth.cmp(&right.depth))
                .then_with(|| right.end_bit.cmp(&left.end_bit))
        });
        Ok(TracedDecodedValue {
            value,
            bit_count: reader.position() - start_position,
            fields,
        })
    }

    pub(crate) fn decode_reflected_from(
        &self,
        reader: &mut BitReader<'_>,
        type_id: u32,
    ) -> Result<BsnValue> {
        self.decode_from_scoped(reader, type_id, true)
    }

    pub(crate) fn decode_reflected_traced_from(
        &self,
        reader: &mut BitReader<'_>,
        type_id: u32,
        path: &str,
        depth: usize,
    ) -> Result<TracedDecodedValue> {
        let start_position = reader.position();
        let mut fields = Some(Vec::new());
        let value =
            self.decode_from_scoped_traced(reader, type_id, true, path, depth, &mut fields)?;
        let mut fields = fields.unwrap_or_default();
        fields.sort_by(|left, right| {
            left.start_bit
                .cmp(&right.start_bit)
                .then_with(|| left.depth.cmp(&right.depth))
                .then_with(|| right.end_bit.cmp(&left.end_bit))
        });
        Ok(TracedDecodedValue {
            value,
            bit_count: reader.position() - start_position,
            fields,
        })
    }

    fn decode_from_scoped(
        &self,
        reader: &mut BitReader<'_>,
        type_id: u32,
        reflected_scope: bool,
    ) -> Result<BsnValue> {
        let mut fields = None;
        self.decode_from_scoped_traced(reader, type_id, reflected_scope, "value", 0, &mut fields)
    }

    fn decode_from_scoped_traced(
        &self,
        reader: &mut BitReader<'_>,
        type_id: u32,
        reflected_scope: bool,
        path: &str,
        depth: usize,
        fields: &mut Option<Vec<DecodedField>>,
    ) -> Result<BsnValue> {
        let start_position = reader.position();
        let shape = self.schema.shape(type_id)?;
        let value = self
            .decode_from_inner(reader, type_id, reflected_scope, path, depth, fields)
            .map_err(|error| {
                append_decode_context(
                    error,
                    format!(
                        "type {} (#{type_id}, {:?}), bits {start_position}..{}",
                        self.type_name(type_id),
                        self.schema
                            .shape(type_id)
                            .map_or(TypeKind::Void, |shape| shape.kind),
                        reader.position()
                    ),
                )
            })?;
        if shape.kind != TypeKind::Alias
            && let Some(fields) = fields
        {
            fields.push(DecodedField {
                path: path.to_owned(),
                kind: type_kind_name(shape.kind),
                value: decoded_summary(&value),
                start_bit: start_position,
                end_bit: reader.position(),
                depth,
            });
        }
        Ok(value)
    }

    #[allow(clippy::too_many_lines)]
    fn decode_from_inner(
        &self,
        reader: &mut BitReader<'_>,
        type_id: u32,
        reflected_scope: bool,
        path: &str,
        depth: usize,
        fields: &mut Option<Vec<DecodedField>>,
    ) -> Result<BsnValue> {
        let shape = self.schema.shape(type_id)?;
        if let Some(RegisteredWireLayout::Custom(layout)) = self.wire_layouts.get(&type_id) {
            if let Some(decode_traced) = layout.decode_traced
                && let Some(destination) = fields
            {
                let (value, traced_fields) = decode_traced(self, type_id, reader, path, depth)?;
                destination.extend(traced_fields);
                return Ok(value);
            }
            return (layout.decode)(self, type_id, reader);
        }
        let reflected_scope = self.reflected_scope(type_id, &shape, reflected_scope)?;
        match shape.kind {
            TypeKind::Alias => self.decode_from_scoped_traced(
                reader,
                required_element(&shape)?,
                reflected_scope,
                path,
                depth,
                fields,
            ),
            TypeKind::Void => Ok(BsnValue::Void),
            TypeKind::Bool => Ok(BsnValue::Bool(reader.read(1)? != 0)),
            TypeKind::Integer | TypeKind::Enum => {
                let value = read_range(reader, required_range(&shape)?)?;
                Ok(BsnValue::Integer(value))
            }
            TypeKind::FourCc => Ok(BsnValue::FourCc(
                u32::try_from(reader.read(32)?).expect("32-bit field fits in u32"),
            )),
            TypeKind::Float32 => Ok(BsnValue::Float32(f32::from_bits(
                u32::try_from(reader.read(32)?).expect("32-bit field fits in u32"),
            ))),
            TypeKind::Float64 => Ok(BsnValue::Float64(f64::from_bits(reader.read(64)?))),
            TypeKind::ByteString | TypeKind::Blob => {
                let length = read_length(reader, required_range(&shape)?)?;
                Ok(BsnValue::Bytes(reader.read_bytes(length, true)?))
            }
            TypeKind::String => {
                let length = read_length(reader, byte_counted(required_range(&shape)?))?;
                let bytes = reader.read_bytes(length, true)?;
                let value = String::from_utf8_lossy(&bytes).into_owned();
                Ok(BsnValue::String(value))
            }
            TypeKind::BitArray => {
                let bit_count = read_length(reader, required_range(&shape)?)?;
                Ok(BsnValue::BitArray(BsnBitArray {
                    data: reader.read_raw(bit_count)?,
                    bit_count,
                }))
            }
            TypeKind::Array => {
                let length = read_length(reader, required_range(&shape)?)?;
                let element = required_element(&shape)?;
                let mut values = Vec::with_capacity(length);
                for index in 0..length {
                    values.push(
                        self.decode_from_scoped_traced(
                            reader,
                            element,
                            reflected_scope,
                            &format!("{path}[{index}]"),
                            depth + 1,
                            fields,
                        )
                        .map_err(|error| {
                            append_decode_context(error, format!("array element [{index}]"))
                        })?,
                    );
                }
                Ok(BsnValue::Array(values))
            }
            TypeKind::Optional => {
                let value = if reader.read(1)? == 0 {
                    None
                } else {
                    Some(Box::new(self.decode_from_scoped_traced(
                        reader,
                        required_element(&shape)?,
                        reflected_scope,
                        &format!("{path}.value"),
                        depth + 1,
                        fields,
                    )?))
                };
                Ok(BsnValue::Optional(value))
            }
            TypeKind::Choice => {
                let index = read_range(reader, required_range(&shape)?)?;
                let position = shape
                    .index_values
                    .iter()
                    .position(|candidate| *candidate == index)
                    .ok_or_else(|| {
                        codec_error(format!("decoded choice index {index} is not declared"))
                    })?;
                Ok(BsnValue::choice(
                    index,
                    self.decode_from_scoped_traced(
                        reader,
                        shape.member_types[position],
                        reflected_scope,
                        &format!("{path}.variant[{index}]"),
                        depth + 1,
                        fields,
                    )?,
                ))
            }
            TypeKind::Struct => {
                let mut struct_fields = Vec::with_capacity(shape.member_types.len());
                for position in 0..shape.member_types.len() {
                    let field_name = shape.member_names[position].as_deref().map_or_else(
                        || format!("#{}", shape.index_values[position]),
                        str::to_owned,
                    );
                    struct_fields.push(BsnField {
                        index: shape.index_values[position],
                        name: shape.member_names[position].clone(),
                        value: self
                            .decode_from_scoped_traced(
                                reader,
                                shape.member_types[position],
                                reflected_scope,
                                &format!("{path}.{field_name}"),
                                depth + 1,
                                fields,
                            )
                            .map_err(|error| {
                                append_decode_context(
                                    error,
                                    format!(
                                        "field {field_name} of {} (#{type_id})",
                                        self.type_name(type_id)
                                    ),
                                )
                            })?,
                    });
                }
                Ok(BsnValue::Struct(BsnStruct::new(type_id, struct_fields)))
            }
        }
    }

    fn reflected_scope(&self, type_id: u32, shape: &TypeShape, inherited: bool) -> Result<bool> {
        if inherited || !shape.obfuscated {
            return Ok(inherited);
        }
        match self.wire_layouts.get(&type_id) {
            Some(RegisteredWireLayout::VerifiedReflected) => Ok(true),
            Some(RegisteredWireLayout::Custom(_)) => {
                unreachable!("custom layouts are dispatched before reflected_scope")
            }
            None => Err(codec_error(format!(
                "type {} (#{type_id}) is marked Obfuscated; reflection order is not a wire layout, and no custom codec or verified-reflection registration is installed",
                self.type_name(type_id)
            ))),
        }
    }

    fn type_name(&self, type_id: u32) -> String {
        self.schema
            .type_metadata(type_id)
            .ok()
            .and_then(|metadata| metadata.name)
            .unwrap_or_else(|| "unnamed".to_owned())
    }

    fn encode_struct(
        &self,
        writer: &mut BitWriter,
        type_id: u32,
        shape: &TypeShape,
        value: &BsnValue,
        reflected_scope: bool,
    ) -> Result<()> {
        let BsnValue::Struct(value) = value else {
            return Err(codec_error("BSN struct value must be a struct"));
        };
        let by_index = value
            .fields
            .iter()
            .map(|field| (field.index, &field.value))
            .collect::<BTreeMap<_, _>>();
        for position in 0..shape.member_types.len() {
            let index = shape.index_values[position];
            let member_type = shape.member_types[position];
            if let Some(value) = by_index.get(&index) {
                self.encode_into_scoped(writer, member_type, value, reflected_scope)?;
            } else {
                let default = self.default_value(member_type)?;
                self.encode_into_scoped(writer, member_type, &default, reflected_scope)?;
            }
        }
        let _ = type_id;
        Ok(())
    }

    fn default_value(&self, type_id: u32) -> Result<BsnValue> {
        let shape = self.schema.shape(type_id)?;
        match shape.kind {
            TypeKind::Alias => self.default_value(required_element(&shape)?),
            TypeKind::Struct if shape.member_types.is_empty() => {
                Ok(BsnValue::Struct(BsnStruct::new(type_id, Vec::new())))
            }
            TypeKind::Optional => Ok(BsnValue::none()),
            TypeKind::Void => Ok(BsnValue::Void),
            _ => Err(codec_error(format!(
                "required BSN field of type {type_id} is missing"
            ))),
        }
    }
}

fn required_range(shape: &TypeShape) -> Result<IntegerRange> {
    shape
        .value_range
        .ok_or_else(|| codec_error(format!("{:?} schema has no numeric range", shape.kind)))
}

const fn type_kind_name(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Array => "array",
        TypeKind::ByteString => "byte string",
        TypeKind::BitArray => "bit array",
        TypeKind::Blob => "blob",
        TypeKind::Bool => "bool",
        TypeKind::Choice => "choice",
        TypeKind::Enum => "enum",
        TypeKind::FourCc => "fourcc",
        TypeKind::Integer => "integer",
        TypeKind::Void => "void",
        TypeKind::Optional => "optional",
        TypeKind::Float32 => "float32",
        TypeKind::Float64 => "float64",
        TypeKind::Struct => "struct",
        TypeKind::String => "string",
        TypeKind::Alias => "alias",
    }
}

fn decoded_summary(value: &BsnValue) -> String {
    match value {
        BsnValue::Void => "void".to_owned(),
        BsnValue::Bool(value) => value.to_string(),
        BsnValue::Integer(value) => value.to_string(),
        BsnValue::FourCc(value) => format!("0x{value:08x}"),
        BsnValue::Float32(value) => value.to_string(),
        BsnValue::Float64(value) => value.to_string(),
        BsnValue::Bytes(value) => format!("{} bytes", value.len()),
        BsnValue::String(value) => value.clone(),
        BsnValue::BitArray(value) => format!("{} bits", value.bit_count),
        BsnValue::Array(value) => format!("{} items", value.len()),
        BsnValue::Optional(value) => {
            if value.is_some() {
                "present".to_owned()
            } else {
                "none".to_owned()
            }
        }
        BsnValue::Choice { index, .. } => format!("variant {index}"),
        BsnValue::Struct(value) => format!("type #{}", value.type_id),
    }
}

fn required_element(shape: &TypeShape) -> Result<u32> {
    shape
        .element_type
        .ok_or_else(|| codec_error(format!("{:?} schema has no element type", shape.kind)))
}

fn write_length(writer: &mut BitWriter, range: IntegerRange, value: usize) -> Result<()> {
    write_range(writer, range, value as i128)
}

fn read_length(reader: &mut BitReader<'_>, range: IntegerRange) -> Result<usize> {
    usize::try_from(read_range(reader, range)?)
        .map_err(|_| codec_error("decoded BSN length does not fit this platform"))
}

const fn byte_counted(range: IntegerRange) -> IntegerRange {
    // bsn declares character counts but encodes utf-8 byte counts.
    IntegerRange {
        bit_width: match range.bit_width {
            Some(width) => Some(width + 2),
            None => None,
        },
        maximum: range.maximum * 4,
        ..range
    }
}

fn write_range(writer: &mut BitWriter, range: IntegerRange, value: i128) -> Result<()> {
    if !(range.minimum..=range.maximum).contains(&value) {
        return Err(codec_error(format!(
            "value {value} is outside BSN range [{}, {}]",
            range.minimum, range.maximum
        )));
    }
    let Some(width) = range.bit_width else {
        if value != range.minimum {
            return Err(codec_error("fixed BSN range received a different value"));
        }
        return Ok(());
    };
    let normalized = u64::try_from(value - range.minimum)
        .map_err(|_| codec_error("ranged BSN value exceeds 64 bits"))?;
    writer.write(normalized, width)
}

fn read_range(reader: &mut BitReader<'_>, range: IntegerRange) -> Result<i128> {
    read_range_with_maximum(reader, range, range.maximum)
}

fn read_range_with_maximum(
    reader: &mut BitReader<'_>,
    range: IntegerRange,
    maximum: i128,
) -> Result<i128> {
    let value = if let Some(width) = range.bit_width {
        i128::from(reader.read(width)?) + range.minimum
    } else {
        range.minimum
    };
    if value > maximum {
        return Err(codec_error(format!(
            "decoded value {value} exceeds BSN range maximum {maximum}"
        )));
    }
    Ok(value)
}

fn expect_variant(_value: &BsnValue, matches: bool, label: &str) -> Result<()> {
    if matches {
        Ok(())
    } else {
        Err(codec_error(format!("{label} BSN value has the wrong type")))
    }
}

fn codec_error(message: impl Into<String>) -> Error {
    Error::BsnWire(message.into())
}

fn append_decode_context(error: Error, context: impl AsRef<str>) -> Error {
    match error {
        Error::IncompleteFrame(_) => error,
        Error::BsnWire(message) => Error::BsnWire(format!("{message}; {}", context.as_ref())),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::metadata::{METADATA_VERSION, Metadata, Schema, StaticSchema, StaticTypeShape};

    fn current_codec() -> Codec {
        Codec::from_schema(Schema::Static(&crate::native::schema::wire::SCHEMA))
    }

    #[test]
    fn bool_and_alias_round_trip() {
        let metadata = Metadata::parse(Arc::<[u8]>::from(synthetic_metadata())).unwrap();
        let codec = Codec::new(metadata);
        for type_id in [0, 1] {
            let encoded = codec.encode(type_id, &BsnValue::Bool(true)).unwrap();
            assert_eq!(encoded.bit_count, 1);
            assert_eq!(encoded.data, [1]);
            assert_eq!(
                codec
                    .decode(type_id, &encoded.data, Some(encoded.bit_count), 0)
                    .unwrap()
                    .value,
                BsnValue::Bool(true)
            );
        }
    }

    #[test]
    fn traced_decode_reports_exact_nested_bit_ranges() {
        static TYPES: &[StaticTypeShape] = &[
            StaticTypeShape {
                type_id: 0,
                name: Some("Flag"),
                kind: TypeKind::Bool,
                implicit_indices: false,
                obfuscated: false,
                value_range: None,
                element_type: None,
                index_values: &[],
                member_types: &[],
                member_names: &[],
            },
            StaticTypeShape {
                type_id: 1,
                name: Some("Count"),
                kind: TypeKind::Integer,
                implicit_indices: false,
                obfuscated: false,
                value_range: Some(IntegerRange {
                    encoding: 0,
                    bit_width: Some(4),
                    control_flag: false,
                    minimum: 0,
                    maximum: 15,
                }),
                element_type: None,
                index_values: &[],
                member_types: &[],
                member_names: &[],
            },
            StaticTypeShape {
                type_id: 2,
                name: Some("Envelope"),
                kind: TypeKind::Struct,
                implicit_indices: false,
                obfuscated: false,
                value_range: None,
                element_type: None,
                index_values: &[0, 1],
                member_types: &[0, 1],
                member_names: &[Some("flag"), Some("count")],
            },
        ];
        static SCHEMA: StaticSchema = StaticSchema::new(3, TYPES);
        let codec = Codec::from_schema(Schema::Static(&SCHEMA));
        let mut writer = BitWriter::new();
        writer.write(0, 3).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(9, 4).unwrap();
        let mut reader = BitReader::new(writer.as_bytes(), Some(8)).unwrap();
        reader.set_position(3).unwrap();

        let decoded = codec.decode_traced_from(&mut reader, 2).unwrap();

        assert_eq!(decoded.bit_count, 5);
        assert_eq!(decoded.fields.len(), 3);
        assert_eq!(
            decoded
                .fields
                .iter()
                .map(|field| (field.path.as_str(), field.start_bit, field.end_bit))
                .collect::<Vec<_>>(),
            [("value", 3, 8), ("value.flag", 3, 4), ("value.count", 4, 8),]
        );
    }

    #[test]
    fn unregistered_obfuscated_type_is_rejected_without_consuming_bits() {
        let codec = current_codec();
        let type_id = codec
            .schema()
            .unique_type_id("Battlenet::Client::Club::InviteAction")
            .unwrap();
        assert_eq!(
            codec.wire_layout_support(type_id).unwrap(),
            WireLayoutSupport::UnsupportedObfuscated
        );

        let packet = hex::decode("f6050002991201000000010000000006acf9060041557c550000").unwrap();
        let mut reader = BitReader::new(&packet, None).unwrap();
        reader.set_position(11).unwrap();
        let error = codec.decode_from(&mut reader, type_id).unwrap_err();
        assert_eq!(reader.position(), 11);
        assert!(error.to_string().contains("marked Obfuscated"));

        let mut writer = BitWriter::new();
        let value = BsnValue::Struct(BsnStruct::new(type_id, Vec::new()));
        let error = codec.encode_into(&mut writer, type_id, &value).unwrap_err();
        assert_eq!(writer.position(), 0);
        assert!(error.to_string().contains("marked Obfuscated"));
    }

    #[test]
    fn current_chat_requests_match_retail_bit_lengths() {
        let codec = current_codec();
        let metadata = codec.schema();
        assert_eq!(metadata.type_count(), 9_982);

        let join_type = metadata
            .unique_type_id("Battlenet::Client::Chat::JoinRequest2")
            .unwrap();
        let join = struct_value(
            join_type,
            metadata,
            [
                ("m_token", BsnValue::Integer(0x1020_3040)),
                (
                    "m_key",
                    BsnValue::choice(
                        2,
                        named_struct(
                            metadata,
                            {
                                let join_shape = metadata.shape(join_type).unwrap();
                                let key_position = join_shape
                                    .member_names
                                    .iter()
                                    .position(|name| name.as_deref() == Some("m_key"))
                                    .unwrap();
                                let choice =
                                    peel_alias(metadata, join_shape.member_types[key_position]);
                                let choice_shape = metadata.shape(choice).unwrap();
                                let choice_position = choice_shape
                                    .index_values
                                    .iter()
                                    .position(|index| *index == 2)
                                    .unwrap();
                                peel_alias(metadata, choice_shape.member_types[choice_position])
                            },
                            [
                                ("m_locale", BsnValue::FourCc(u32::from_be_bytes(*b"enUS"))),
                                ("m_name", BsnValue::Integer(7)),
                            ],
                        ),
                    ),
                ),
            ],
        );
        let mut framed = BitWriter::new();
        framed.write(0, 6).unwrap();
        framed.write(1, 1).unwrap();
        framed.write(5, 4).unwrap();
        codec
            .encode_reflected_into(&mut framed, join_type, &join)
            .unwrap();
        assert_eq!(framed.position(), 93);

        let message_type = metadata
            .unique_type_id("Battlenet::Client::Chat::MessageSend")
            .unwrap();
        let message = struct_value(
            message_type,
            metadata,
            [
                ("m_channelIndex", BsnValue::Integer(1)),
                ("m_body", BsnValue::String("hello".into())),
            ],
        );
        let mut payload = BitWriter::new();
        codec
            .encode_reflected_into(&mut payload, message_type, &message)
            .unwrap();
        let payload_bit_count = payload.position();
        let mut framed = BitWriter::new();
        framed.write(11, 6).unwrap();
        framed.write(1, 1).unwrap();
        framed.write(5, 4).unwrap();
        codec
            .encode_reflected_into(&mut framed, message_type, &message)
            .unwrap();
        assert_ne!(payload_bit_count + 11, framed.position());
        let mut reader = BitReader::new(framed.as_bytes(), Some(framed.position())).unwrap();
        reader.set_position(11).unwrap();
        codec
            .decode_reflected_from(&mut reader, message_type)
            .unwrap();
        assert_eq!(reader.position(), framed.position());
    }

    #[test]
    fn range_errors_identify_the_exact_bsn_type_and_bit_offset() {
        static TYPES: &[StaticTypeShape] = &[StaticTypeShape {
            type_id: 0,
            name: Some("Battlenet::Attribute::VisualFileIndex"),
            kind: TypeKind::Integer,
            implicit_indices: false,
            obfuscated: false,
            value_range: Some(IntegerRange {
                encoding: 1,
                bit_width: Some(6),
                control_flag: true,
                minimum: 0,
                maximum: 32,
            }),
            element_type: None,
            index_values: &[],
            member_types: &[],
            member_names: &[],
        }];
        static SCHEMA: StaticSchema = StaticSchema::new(1, TYPES);
        let type_id = 0;
        let codec = Codec::from_schema(Schema::Static(&SCHEMA));
        let mut writer = BitWriter::new();
        writer.write(51, 6).unwrap();

        let error = codec
            .decode(type_id, writer.as_bytes(), Some(6), 0)
            .unwrap_err()
            .to_string();

        assert!(error.contains("decoded value 51 exceeds BSN range maximum 32"));
        assert!(error.contains("Battlenet::Attribute::VisualFileIndex"));
        assert!(error.contains(&format!("#{type_id}")));
        assert!(error.contains("bits 0..6"));
    }

    #[test]
    fn account_name_parts_use_an_eight_bit_utf8_byte_length() {
        let codec = current_codec();
        let type_id = codec
            .schema()
            .unique_type_id("Battlenet::Account::NamePart")
            .unwrap();
        let surname = "星".repeat(32);
        let encoded = codec
            .encode(type_id, &BsnValue::String(surname.clone()))
            .unwrap();

        assert_eq!(encoded.data[0], 96);
        assert_eq!(encoded.bit_count, 8 + 96 * 8);
        let decoded = codec
            .decode(type_id, &encoded.data, Some(encoded.bit_count), 0)
            .unwrap();
        assert_eq!(decoded.value, BsnValue::String(surname));
        assert_eq!(decoded.bit_count, encoded.bit_count);
    }

    #[test]
    fn name_length_is_counted_in_bytes_up_to_four_per_declared_character() {
        let codec = current_codec();
        let type_id = codec
            .schema()
            .unique_type_id("Battlenet::Account::NamePart")
            .unwrap();

        let encoded = codec
            .encode(type_id, &BsnValue::String("x".repeat(33)))
            .unwrap();
        assert_eq!(encoded.data[0], 33);
        assert_eq!(encoded.bit_count, 8 + 33 * 8);

        let error = codec
            .encode(type_id, &BsnValue::String("x".repeat(129)))
            .unwrap_err()
            .to_string();
        assert!(error.contains("outside BSN range"), "{error}");
    }

    #[test]
    fn full_name_aligns_both_multibyte_name_parts_from_a_friend_record_offset() {
        let codec = current_codec();
        let metadata = codec.schema();
        let type_id = metadata
            .unique_type_id("Battlenet::Account::FullName")
            .unwrap();
        let value = named_struct(
            metadata,
            type_id,
            [
                ("m_givenName", BsnValue::String("李".into())),
                ("m_surname", BsnValue::String("Pérez".into())),
            ],
        );
        let mut writer = BitWriter::new();
        writer.write(0, 57).unwrap();
        codec.encode_into(&mut writer, type_id, &value).unwrap();

        assert_eq!(writer.position(), 152);
        let decoded = codec
            .decode(type_id, writer.as_bytes(), Some(writer.position()), 57)
            .unwrap();
        assert_eq!(decoded.value, value);
        assert_eq!(decoded.bit_count, 95);
    }

    fn struct_value<const N: usize>(
        type_id: u32,
        metadata: &Schema,
        values: [(&str, BsnValue); N],
    ) -> BsnValue {
        named_struct(metadata, type_id, values)
    }

    fn named_struct<const N: usize>(
        metadata: &Schema,
        type_id: u32,
        values: [(&str, BsnValue); N],
    ) -> BsnValue {
        let shape = metadata.shape(type_id).unwrap();
        let fields = values
            .into_iter()
            .map(|(name, value)| {
                let position = shape
                    .member_names
                    .iter()
                    .position(|candidate| candidate.as_deref() == Some(name))
                    .unwrap();
                BsnField::named(shape.index_values[position], name, value)
            })
            .collect();
        BsnValue::Struct(BsnStruct::new(type_id, fields))
    }

    fn peel_alias(metadata: &Schema, mut type_id: u32) -> u32 {
        loop {
            let shape = metadata.shape(type_id).unwrap();
            if shape.kind != TypeKind::Alias {
                return type_id;
            }
            type_id = shape.element_type.unwrap();
        }
    }

    fn synthetic_metadata() -> Vec<u8> {
        let type_count = 2_u32;
        let dump_data_offset = 20 + type_count * 4;
        let dump_records = [0x05, 0x10, 0, 0, 0, 0];
        let string_table_offset = dump_data_offset + u32::try_from(dump_records.len()).unwrap();
        let string_data_offset = string_table_offset + type_count * 4;
        let names = b"Example::Bool\0Example::Alias\0";
        let bool_name = string_data_offset + 16;
        let alias_name = bool_name + u32::try_from(b"Example::Bool\0".len()).unwrap();
        let mut blob = Vec::new();
        for value in [
            METADATA_VERSION,
            0,
            type_count,
            u32::try_from(dump_records.len()).unwrap(),
            16 + u32::try_from(names.len()).unwrap(),
            dump_data_offset,
            dump_data_offset + 1,
        ] {
            blob.extend_from_slice(&value.to_be_bytes());
        }
        blob.extend_from_slice(&dump_records);
        for value in [string_data_offset, string_data_offset + 8] {
            blob.extend_from_slice(&value.to_be_bytes());
        }
        for value in [bool_name, 0, alias_name, 0] {
            blob.extend_from_slice(&value.to_be_bytes());
        }
        blob.extend_from_slice(names);
        blob
    }
}
