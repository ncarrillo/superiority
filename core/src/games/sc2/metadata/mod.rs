use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

use crate::{Error, Result};

pub const METADATA_VERSION: u32 = 7;
const HEADER_SIZE: usize = 20;
const MAX_TYPE_COUNT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataHeader {
    pub version: u32,
    pub flags: u32,
    pub type_count: usize,
    pub dump_size: usize,
    pub string_size: usize,
}

impl MetadataHeader {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(metadata_error("metadata block is smaller than its header"));
        }
        Ok(Self {
            version: read_u32_at(data, 0)?,
            flags: read_u32_at(data, 4)?,
            type_count: read_u32_at(data, 8)? as usize,
            dump_size: read_u32_at(data, 12)? as usize,
            string_size: read_u32_at(data, 16)? as usize,
        })
    }

    pub fn byte_size(self) -> Result<usize> {
        let table_size = self
            .type_count
            .checked_mul(4)
            .ok_or_else(|| metadata_error("metadata type table overflows"))?;
        HEADER_SIZE
            .checked_add(table_size)
            .and_then(|size| size.checked_add(self.dump_size))
            .and_then(|size| size.checked_add(table_size))
            .and_then(|size| size.checked_add(self.string_size))
            .ok_or_else(|| metadata_error("metadata size overflows"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Array,
    ByteString,
    BitArray,
    Blob,
    Bool,
    Choice,
    Enum,
    FourCc,
    Integer,
    Void,
    Optional,
    Float32,
    Float64,
    Struct,
    String,
    Alias,
}

impl TypeKind {
    fn from_tag(tag: u8) -> Result<Self> {
        match tag {
            1 => Ok(Self::Array),
            2 => Ok(Self::ByteString),
            3 => Ok(Self::BitArray),
            4 => Ok(Self::Blob),
            5 => Ok(Self::Bool),
            6 => Ok(Self::Choice),
            7 => Ok(Self::Enum),
            8 => Ok(Self::FourCc),
            9 => Ok(Self::Integer),
            10 => Ok(Self::Void),
            11 => Ok(Self::Optional),
            12 => Ok(Self::Float32),
            13 => Ok(Self::Float64),
            14 => Ok(Self::Struct),
            15 => Ok(Self::String),
            16 => Ok(Self::Alias),
            _ => Err(metadata_error(format!("unknown BSN type tag {tag}"))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeMetadata {
    pub type_id: u32,
    pub name: Option<String>,
    pub tag: u8,
    pub flags: u8,
    pub dump: Vec<u8>,
    pub strings: Vec<Option<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegerRange {
    pub encoding: u8,
    pub bit_width: Option<usize>,
    pub control_flag: bool,
    pub minimum: i128,
    pub maximum: i128,
}

impl IntegerRange {
    #[must_use]
    pub fn count(self) -> u128 {
        if self.maximum < self.minimum {
            0
        } else {
            u128::try_from(self.maximum - self.minimum + 1).expect("non-negative range count")
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeShape {
    pub type_id: u32,
    pub kind: TypeKind,
    pub implicit_indices: bool,
    pub obfuscated: bool,
    pub value_range: Option<IntegerRange>,
    pub element_type: Option<u32>,
    pub index_values: Vec<i128>,
    pub member_types: Vec<u32>,
    pub member_names: Vec<Option<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticTypeShape {
    pub type_id: u32,
    pub name: Option<&'static str>,
    pub kind: TypeKind,
    pub implicit_indices: bool,
    pub obfuscated: bool,
    pub value_range: Option<IntegerRange>,
    pub element_type: Option<u32>,
    pub index_values: &'static [i128],
    pub member_types: &'static [u32],
    pub member_names: &'static [Option<&'static str>],
}

impl StaticTypeShape {
    fn to_shape(self) -> TypeShape {
        TypeShape {
            type_id: self.type_id,
            kind: self.kind,
            implicit_indices: self.implicit_indices,
            obfuscated: self.obfuscated,
            value_range: self.value_range,
            element_type: self.element_type,
            index_values: self.index_values.to_vec(),
            member_types: self.member_types.to_vec(),
            member_names: self
                .member_names
                .iter()
                .map(|name| name.map(str::to_owned))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StaticSchema {
    type_count: usize,
    types: &'static [StaticTypeShape],
}

impl StaticSchema {
    #[must_use]
    pub const fn new(type_count: usize, types: &'static [StaticTypeShape]) -> Self {
        Self { type_count, types }
    }

    fn get(self, type_id: u32) -> Result<StaticTypeShape> {
        self.types
            .binary_search_by_key(&type_id, |shape| shape.type_id)
            .map(|index| self.types[index])
            .map_err(|_| {
                metadata_error(format!(
                    "BSN type id {type_id} is not part of the generated protocol schema"
                ))
            })
    }
}

#[derive(Clone, Debug)]
pub enum Schema {
    Runtime(Metadata),
    Static(&'static StaticSchema),
}

impl From<Metadata> for Schema {
    fn from(metadata: Metadata) -> Self {
        Self::Runtime(metadata)
    }
}

impl Schema {
    #[must_use]
    pub fn type_count(&self) -> usize {
        match self {
            Self::Runtime(metadata) => metadata.header.type_count,
            Self::Static(schema) => schema.type_count,
        }
    }

    #[must_use]
    pub fn known_type_ids(&self) -> Vec<u32> {
        match self {
            Self::Runtime(metadata) => (0..metadata.header.type_count)
                .map(|id| u32::try_from(id).expect("validated metadata type count fits u32"))
                .collect(),
            Self::Static(schema) => schema.types.iter().map(|shape| shape.type_id).collect(),
        }
    }

    pub fn type_metadata(&self, type_id: u32) -> Result<TypeMetadata> {
        match self {
            Self::Runtime(metadata) => metadata.type_metadata(type_id),
            Self::Static(schema) => {
                let shape = schema.get(type_id)?;
                let name = shape.name.map(str::to_owned);
                Ok(TypeMetadata {
                    type_id,
                    name: name.clone(),
                    tag: type_kind_tag(shape.kind),
                    flags: u8::from(shape.implicit_indices) << 1 | u8::from(shape.obfuscated),
                    dump: Vec::new(),
                    strings: vec![name],
                })
            }
        }
    }

    pub fn find(&self, query: &str, exact: bool) -> Result<Vec<TypeMetadata>> {
        if let Self::Runtime(metadata) = self {
            return metadata.find(query, exact);
        }
        let needle = query.to_lowercase();
        let Self::Static(schema) = self else {
            unreachable!()
        };
        schema
            .types
            .iter()
            .filter_map(|shape| {
                let name = shape.name?;
                let hit = if exact {
                    name.eq_ignore_ascii_case(query)
                } else {
                    name.to_lowercase().contains(&needle)
                };
                hit.then_some(shape.type_id)
            })
            .map(|type_id| self.type_metadata(type_id))
            .collect()
    }

    pub fn unique_type_id(&self, exact_name: &str) -> Result<u32> {
        let matches = self.find(exact_name, true)?;
        if matches.len() != 1 {
            return Err(metadata_error(format!(
                "expected one native type named {exact_name:?}, found {}",
                matches.len()
            )));
        }
        Ok(matches[0].type_id)
    }

    pub fn shape(&self, type_id: u32) -> Result<TypeShape> {
        match self {
            Self::Runtime(metadata) => metadata.shape(type_id),
            Self::Static(schema) => schema.get(type_id).map(StaticTypeShape::to_shape),
        }
    }

    #[must_use]
    pub const fn runtime_metadata(&self) -> Option<&Metadata> {
        match self {
            Self::Runtime(metadata) => Some(metadata),
            Self::Static(_) => None,
        }
    }
}

const fn type_kind_tag(kind: TypeKind) -> u8 {
    match kind {
        TypeKind::Array => 1,
        TypeKind::ByteString => 2,
        TypeKind::BitArray => 3,
        TypeKind::Blob => 4,
        TypeKind::Bool => 5,
        TypeKind::Choice => 6,
        TypeKind::Enum => 7,
        TypeKind::FourCc => 8,
        TypeKind::Integer => 9,
        TypeKind::Void => 10,
        TypeKind::Optional => 11,
        TypeKind::Float32 => 12,
        TypeKind::Float64 => 13,
        TypeKind::Struct => 14,
        TypeKind::String => 15,
        TypeKind::Alias => 16,
    }
}

#[derive(Clone, Debug)]
pub struct Metadata {
    blob: Arc<[u8]>,
    pub header: MetadataHeader,
    dump_table_offset: usize,
    string_table_offset: usize,
    string_data_offset: usize,
    string_literal_offset: usize,
    dump_offsets: Vec<usize>,
    string_offsets: Vec<usize>,
    dump_ends: BTreeMap<usize, usize>,
    string_ends: BTreeMap<usize, usize>,
}

impl Metadata {
    pub fn parse(blob: impl Into<Arc<[u8]>>) -> Result<Self> {
        let blob = blob.into();
        let header = MetadataHeader::parse(&blob)?;
        if header.version != METADATA_VERSION {
            return Err(metadata_error(format!(
                "unsupported BSN metadata version {}",
                header.version
            )));
        }
        if header.type_count == 0 {
            return Err(metadata_error("metadata block contains no types"));
        }
        if header.type_count > MAX_TYPE_COUNT {
            return Err(metadata_error("metadata type count is unreasonable"));
        }
        if header.dump_size == 0 {
            return Err(metadata_error("metadata block contains no dump records"));
        }
        if header.byte_size()? != blob.len() {
            return Err(metadata_error(format!(
                "metadata size mismatch: header describes {} bytes, received {}",
                header.byte_size()?,
                blob.len()
            )));
        }

        let table_size = header.type_count * 4;
        let dump_table_offset = HEADER_SIZE;
        let dump_data_offset = dump_table_offset + table_size;
        let string_table_offset = dump_data_offset + header.dump_size;
        let string_data_offset = string_table_offset + table_size;
        let dump_offsets = read_offset_table(&blob, dump_table_offset, header.type_count)?;
        let string_offsets = read_offset_table(&blob, string_table_offset, header.type_count)?;

        validate_offsets(
            &dump_offsets,
            dump_data_offset,
            string_table_offset,
            false,
            "dump",
        )?;
        validate_offsets(
            &string_offsets,
            string_data_offset,
            blob.len(),
            true,
            "string",
        )?;

        let dump_ends = record_ends(&dump_offsets, string_table_offset, false);
        let mut string_record_starts = string_offsets
            .iter()
            .copied()
            .filter(|offset| *offset != 0)
            .collect::<Vec<_>>();
        string_record_starts.sort_unstable();
        string_record_starts.dedup();

        let string_literal_offset = string_record_starts
            .iter()
            .map(|offset| read_u32_at(&blob, *offset).map(|value| value as usize))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|offset| *offset != 0)
            .min()
            .ok_or_else(|| metadata_error("metadata has no named dump strings"))?;
        if !(string_data_offset..=blob.len()).contains(&string_literal_offset) {
            return Err(metadata_error(
                "dump-string records point outside the string region",
            ));
        }
        if string_record_starts
            .last()
            .is_some_and(|offset| *offset >= string_literal_offset)
        {
            return Err(metadata_error(
                "dump-string records overlap literal strings",
            ));
        }
        let string_ends = record_ends(&string_offsets, string_literal_offset, true);

        Ok(Self {
            blob,
            header,
            dump_table_offset,
            string_table_offset,
            string_data_offset,
            string_literal_offset,
            dump_offsets,
            string_offsets,
            dump_ends,
            string_ends,
        })
    }

    #[must_use]
    pub fn blob(&self) -> &[u8] {
        &self.blob
    }

    pub fn dump_record(&self, type_id: u32) -> Result<&[u8]> {
        let start = Self::checked_offset(&self.dump_offsets, type_id)?;
        let end = *self
            .dump_ends
            .get(&start)
            .ok_or_else(|| metadata_error("dump record has no end"))?;
        Ok(&self.blob[start..end])
    }

    pub fn string_offsets(&self, type_id: u32) -> Result<Vec<usize>> {
        let start = Self::checked_offset(&self.string_offsets, type_id)?;
        if start == 0 {
            return Ok(Vec::new());
        }
        let end = *self
            .string_ends
            .get(&start)
            .ok_or_else(|| metadata_error("string record has no end"))?;
        if (end - start) % 4 != 0 {
            return Err(metadata_error(format!(
                "string record for type {type_id} is not u32-aligned"
            )));
        }
        (start..end)
            .step_by(4)
            .map(|offset| read_u32_at(&self.blob, offset).map(|value| value as usize))
            .collect()
    }

    pub fn string_at(&self, offset: usize) -> Result<Option<String>> {
        if offset == 0 {
            return Ok(None);
        }
        if !(self.string_literal_offset..self.blob.len()).contains(&offset) {
            return Err(metadata_error(
                "dump-string offset is outside the string region",
            ));
        }
        let relative_end = self.blob[offset..]
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| metadata_error("dump string is not NUL-terminated"))?;
        let value = std::str::from_utf8(&self.blob[offset..offset + relative_end])
            .map_err(|_| metadata_error("dump string is not valid UTF-8"))?;
        Ok(Some(value.to_owned()))
    }

    pub fn strings(&self, type_id: u32) -> Result<Vec<Option<String>>> {
        self.string_offsets(type_id)?
            .into_iter()
            .map(|offset| self.string_at(offset))
            .collect()
    }

    pub fn type_metadata(&self, type_id: u32) -> Result<TypeMetadata> {
        let dump = self.dump_record(type_id)?.to_vec();
        let first = *dump
            .first()
            .ok_or_else(|| metadata_error(format!("dump record for type {type_id} is empty")))?;
        let strings = self.strings(type_id)?;
        Ok(TypeMetadata {
            type_id,
            name: strings.first().cloned().flatten(),
            tag: first & 0x3f,
            flags: first >> 6,
            dump,
            strings,
        })
    }

    pub fn find(&self, query: &str, exact: bool) -> Result<Vec<TypeMetadata>> {
        let needle = query.to_lowercase();
        let mut matches = Vec::new();
        let type_count = u32::try_from(self.header.type_count)
            .map_err(|_| metadata_error("metadata type count exceeds u32"))?;
        for type_id in 0..type_count {
            let info = self.type_metadata(type_id)?;
            let Some(name) = &info.name else {
                continue;
            };
            let hit = if exact {
                name.eq_ignore_ascii_case(query)
            } else {
                name.to_lowercase().contains(&needle)
            };
            if hit {
                matches.push(info);
            }
        }
        Ok(matches)
    }

    pub fn unique_type_id(&self, exact_name: &str) -> Result<u32> {
        let matches = self.find(exact_name, true)?;
        if matches.len() != 1 {
            return Err(metadata_error(format!(
                "expected one native type named {exact_name:?}, found {}",
                matches.len()
            )));
        }
        Ok(matches[0].type_id)
    }

    pub fn shape(&self, type_id: u32) -> Result<TypeShape> {
        let info = self.type_metadata(type_id)?;
        let raw_tag = info.dump[0];
        let tag = raw_tag & 0x3f;
        let kind = TypeKind::from_tag(tag)?;
        let implicit_indices = raw_tag & 0x80 != 0;
        let obfuscated = raw_tag & 0x40 != 0;
        let mut offset = 1;
        let mut value_range = None;
        let mut element_type = None;
        let mut index_values = Vec::new();
        let mut member_types = Vec::new();

        match kind {
            TypeKind::Array => {
                let (range, next) = read_range(&info.dump, offset, implicit_indices)?;
                value_range = Some(range);
                offset = next;
                let (element, next) = read_u32(&info.dump, offset)?;
                element_type = Some(element);
                offset = next;
            }
            TypeKind::ByteString | TypeKind::String | TypeKind::Integer => {
                let (range, next) = read_range(&info.dump, offset, false)?;
                value_range = Some(range);
                offset = next;
            }
            TypeKind::BitArray | TypeKind::Blob => {
                let (range, next) = read_range(&info.dump, offset, implicit_indices)?;
                value_range = Some(range);
                offset = next;
            }
            TypeKind::Choice | TypeKind::Enum | TypeKind::Struct => {
                let (range, next) = read_range(&info.dump, offset, false)?;
                value_range = Some(range);
                let (indices, next) = read_index_block(&info.dump, next, range, implicit_indices)?;
                index_values = indices;
                offset = next;
                if matches!(kind, TypeKind::Choice | TypeKind::Struct) {
                    for _ in &index_values {
                        let (member, next) = read_u32(&info.dump, offset)?;
                        member_types.push(member);
                        offset = next;
                    }
                }
            }
            TypeKind::Optional | TypeKind::Alias => {
                let (element, next) = read_u32(&info.dump, offset)?;
                element_type = Some(element);
                offset = next;
            }
            TypeKind::Bool
            | TypeKind::FourCc
            | TypeKind::Void
            | TypeKind::Float32
            | TypeKind::Float64 => {}
        }

        if offset != info.dump.len() {
            return Err(metadata_error(format!(
                "{kind:?} dump record for type {type_id} has {} unparsed bytes",
                info.dump.len() - offset
            )));
        }
        let member_names = (0..index_values.len())
            .map(|index| info.strings.get(index + 1).cloned().flatten())
            .collect();
        Ok(TypeShape {
            type_id,
            kind,
            implicit_indices,
            obfuscated,
            value_range,
            element_type,
            index_values,
            member_types,
            member_names,
        })
    }

    fn checked_offset(table: &[usize], type_id: u32) -> Result<usize> {
        table
            .get(type_id as usize)
            .copied()
            .ok_or_else(|| metadata_error(format!("BSN type id {type_id} is out of range")))
    }

    #[must_use]
    pub const fn layout_offsets(&self) -> (usize, usize, usize) {
        (
            self.dump_table_offset,
            self.string_table_offset,
            self.string_data_offset,
        )
    }
}

pub fn read_largest_metadata(path: impl AsRef<Path>) -> Result<Metadata> {
    let data = fs::read(path)?;
    find_metadata_blobs(&data)?
        .into_iter()
        .max_by_key(|(_, metadata)| metadata.blob().len())
        .map(|(_, metadata)| metadata)
        .ok_or_else(|| metadata_error("the executable contains no valid native BSN metadata"))
}

pub fn read_metadata(path: impl AsRef<Path>) -> Result<Metadata> {
    Metadata::parse(Arc::<[u8]>::from(fs::read(path)?))
}

pub fn find_metadata_blobs(data: &[u8]) -> Result<Vec<(usize, Metadata)>> {
    let marker = METADATA_VERSION.to_be_bytes();
    let mut found = Vec::new();
    for offset in 0..data.len().saturating_sub(HEADER_SIZE - 1) {
        if data[offset..offset + 4] != marker {
            continue;
        }
        let Ok(header) = MetadataHeader::parse(&data[offset..]) else {
            continue;
        };
        if header.type_count == 0 || header.type_count > MAX_TYPE_COUNT || header.dump_size == 0 {
            continue;
        }
        let Ok(size) = header.byte_size() else {
            continue;
        };
        let Some(blob) = data.get(offset..offset.saturating_add(size)) else {
            continue;
        };
        if let Ok(metadata) = Metadata::parse(Arc::<[u8]>::from(blob)) {
            found.push((offset, metadata));
        }
    }
    Ok(found)
}

fn read_range(data: &[u8], mut offset: usize, single: bool) -> Result<(IntegerRange, usize)> {
    let encoding = *data
        .get(offset)
        .ok_or_else(|| metadata_error("integer-range metadata is truncated"))?;
    offset += 1;
    if single {
        let (value, offset) = read_scalar(data, offset, encoding)?;
        return Ok((
            IntegerRange {
                encoding,
                bit_width: None,
                control_flag: false,
                minimum: value,
                maximum: value,
            },
            offset,
        ));
    }
    let control = *data
        .get(offset)
        .ok_or_else(|| metadata_error("integer-range control byte is missing"))?;
    offset += 1;
    let (minimum, offset) = read_scalar(data, offset, encoding)?;
    let (maximum, offset) = read_scalar(data, offset, encoding)?;
    Ok((
        IntegerRange {
            encoding,
            bit_width: Some(usize::from(control & 0x7f)),
            control_flag: control & 0x80 != 0,
            minimum,
            maximum,
        },
        offset,
    ))
}

fn read_index_block(
    data: &[u8],
    mut offset: usize,
    range: IntegerRange,
    implicit: bool,
) -> Result<(Vec<i128>, usize)> {
    if implicit {
        return Ok(((range.minimum..=range.maximum).collect(), offset));
    }
    let count_encoding = *data
        .get(offset)
        .ok_or_else(|| metadata_error("indexed-type count encoding is missing"))?;
    offset += 1;
    if !matches!(count_encoding, 1 | 3 | 5 | 7) {
        return Err(metadata_error(format!(
            "unsupported indexed-type count encoding {count_encoding}"
        )));
    }
    let (count, next) = read_scalar(data, offset, count_encoding)?;
    offset = next;
    let count = usize::try_from(count)
        .ok()
        .filter(|count| *count <= MAX_TYPE_COUNT)
        .ok_or_else(|| metadata_error("indexed-type count is unreasonable"))?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let (value, next) = read_scalar(data, offset, range.encoding)?;
        values.push(value);
        offset = next;
    }
    Ok((values, offset))
}

fn read_scalar(data: &[u8], offset: usize, encoding: u8) -> Result<(i128, usize)> {
    let width = match encoding {
        0 | 1 => 1,
        2 | 3 => 2,
        4 | 5 => 4,
        6 | 7 => 8,
        _ => {
            return Err(metadata_error(format!(
                "unknown BSN integer encoding {encoding}"
            )));
        }
    };
    let end = offset
        .checked_add(width)
        .ok_or_else(|| metadata_error("integer metadata overflows"))?;
    let bytes = data
        .get(offset..end)
        .ok_or_else(|| metadata_error("integer metadata is truncated"))?;
    let unsigned = bytes
        .iter()
        .fold(0_u128, |value, byte| (value << 8) | u128::from(*byte));
    let signed = encoding.is_multiple_of(2);
    let unsigned = i128::try_from(unsigned).expect("u64 scalar fits in i128");
    let value = if signed && bytes[0] & 0x80 != 0 {
        unsigned - (1_i128 << (width * 8))
    } else {
        unsigned
    };
    Ok((value, end))
}

fn read_u32(data: &[u8], offset: usize) -> Result<(u32, usize)> {
    Ok((read_u32_at(data, offset)?, offset + 4))
}

fn read_u32_at(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| metadata_error("u32 metadata is truncated"))?;
    Ok(u32::from_be_bytes(bytes.try_into().expect("fixed length")))
}

fn read_offset_table(data: &[u8], offset: usize, count: usize) -> Result<Vec<usize>> {
    (0..count)
        .map(|index| read_u32_at(data, offset + index * 4).map(|value| value as usize))
        .collect()
}

fn validate_offsets(
    offsets: &[usize],
    start: usize,
    end: usize,
    allow_zero: bool,
    label: &str,
) -> Result<()> {
    for (type_id, offset) in offsets.iter().copied().enumerate() {
        if allow_zero && offset == 0 {
            continue;
        }
        if !(start..end).contains(&offset) {
            return Err(metadata_error(format!(
                "{label} offset for type {type_id} is outside its data region"
            )));
        }
    }
    Ok(())
}

fn record_ends(offsets: &[usize], end: usize, allow_zero: bool) -> BTreeMap<usize, usize> {
    let mut starts = offsets
        .iter()
        .copied()
        .filter(|offset| !(allow_zero && *offset == 0))
        .collect::<Vec<_>>();
    starts.sort_unstable();
    starts.dedup();
    starts
        .iter()
        .copied()
        .zip(starts.iter().copied().skip(1).chain([end]))
        .collect()
}

fn metadata_error(message: impl Into<String>) -> Error {
    Error::Metadata(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_metadata() -> Vec<u8> {
        let type_count = 2_u32;
        let dump_data_offset = u32::try_from(HEADER_SIZE).unwrap() + type_count * 4;
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

    #[test]
    fn parses_types_names_and_shapes() {
        let metadata = Metadata::parse(Arc::<[u8]>::from(synthetic_metadata())).unwrap();
        assert_eq!(metadata.header.type_count, 2);
        assert_eq!(
            metadata.type_metadata(0).unwrap().name.as_deref(),
            Some("Example::Bool")
        );
        assert_eq!(metadata.shape(0).unwrap().kind, TypeKind::Bool);
        assert_eq!(metadata.shape(1).unwrap().kind, TypeKind::Alias);
        assert_eq!(metadata.shape(1).unwrap().element_type, Some(0));
    }

    #[test]
    fn finds_embedded_block() {
        let blob = synthetic_metadata();
        let mut image = b"not metadata".to_vec();
        image.extend_from_slice(&blob);
        image.extend_from_slice(b"trailer");
        let found = find_metadata_blobs(&image).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, b"not metadata".len());
    }

    #[test]
    fn parses_signed_implicit_index_range() {
        let record = hex("ce0001feff0000000000000001");
        let (range, offset) = read_range(&record, 1, false).unwrap();
        assert_eq!((range.minimum, range.maximum), (-2, -1));
        let (indices, offset) = read_index_block(&record, offset, range, true).unwrap();
        assert_eq!(indices, [-2, -1]);
        let (_, offset) = read_u32(&record, offset).unwrap();
        let (_, offset) = read_u32(&record, offset).unwrap();
        assert_eq!(offset, record.len());
    }

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
}
