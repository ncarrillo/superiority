//! protobuf with no descriptor: a field walker and a field builder.
//!
//! `protobuf.rs` beside this reads BGS's frames through prost, which needs
//! generated types. Remastered's classic header has no descriptor to generate
//! from — its field numbers were read out of the SDK's own parser — so it is
//! walked field by field instead, and unknown fields are carried through
//! verbatim rather than dropped.
//!
//! Ported from `sc1-research`.

// starcraft's classic messages are generated with protobuf's lite runtime, so
// most carry no descriptor. this codec works at the wire-field level: it needs
// no schema and preserves fields it does not recognize.

use crate::{Error, Result};

pub const VARINT: u32 = 0;
pub const FIXED64: u32 = 1;
pub const LENGTH_DELIMITED: u32 = 2;
pub const FIXED32: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

impl Value<'_> {
    #[must_use]
    pub fn wire_type(&self) -> u32 {
        match self {
            Self::Varint(_) => VARINT,
            Self::Fixed64(_) => FIXED64,
            Self::Bytes(_) => LENGTH_DELIMITED,
            Self::Fixed32(_) => FIXED32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<'a> {
    pub number: u32,
    pub value: Value<'a>,
}

impl<'a> Field<'a> {
    #[must_use]
    pub fn varint(&self) -> Option<u64> {
        match self.value {
            Value::Varint(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn bytes(&self) -> Option<&'a [u8]> {
        match self.value {
            Value::Bytes(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn fixed32(&self) -> Option<u32> {
        match self.value {
            Value::Fixed32(value) => Some(value),
            _ => None,
        }
    }
}

// iteration stops at the first malformed field and yields the error, so
// collecting into Result surfaces truncation as a failure.
pub struct Fields<'a> {
    data: &'a [u8],
    offset: usize,
    failed: bool,
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<Field<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.offset >= self.data.len() {
            return None;
        }
        match self.read_field() {
            Ok(field) => Some(Ok(field)),
            Err(error) => {
                self.failed = true;
                Some(Err(error))
            }
        }
    }
}

impl<'a> Fields<'a> {
    fn read_field(&mut self) -> Result<Field<'a>> {
        let start = self.offset;
        let tag = self.read_varint()?;
        let number = u32::try_from(tag >> 3)
            .map_err(|_| protobuf_error(format!("field number at offset {start} exceeds u32")))?;
        if number == 0 {
            return Err(protobuf_error(format!("field zero at offset {start}")));
        }
        let value = match (tag & 7) as u32 {
            VARINT => Value::Varint(self.read_varint()?),
            FIXED64 => Value::Fixed64(u64::from_le_bytes(self.read_array::<8>(number)?)),
            LENGTH_DELIMITED => {
                let length = usize::try_from(self.read_varint()?).map_err(|_| {
                    protobuf_error(format!("field {number} length exceeds this platform"))
                })?;
                let end = self
                    .offset
                    .checked_add(length)
                    .filter(|end| *end <= self.data.len())
                    .ok_or_else(|| protobuf_error(format!("truncated bytes field {number}")))?;
                let bytes = &self.data[self.offset..end];
                self.offset = end;
                Value::Bytes(bytes)
            }
            FIXED32 => Value::Fixed32(u32::from_le_bytes(self.read_array::<4>(number)?)),
            other => {
                return Err(protobuf_error(format!(
                    "unsupported wire type {other} for field {number} at offset {start}"
                )));
            }
        };
        Ok(Field { number, value })
    }

    fn read_varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..70).step_by(7) {
            let byte = *self
                .data
                .get(self.offset)
                .ok_or_else(|| protobuf_error("truncated varint"))?;
            self.offset += 1;
            value |= u64::from(byte & 0x7F) << shift;
            if byte < 0x80 {
                if shift == 63 && byte > 1 {
                    return Err(protobuf_error("varint exceeds 64 bits"));
                }
                return Ok(value);
            }
        }
        Err(protobuf_error("varint exceeds ten bytes"))
    }

    fn read_array<const N: usize>(&mut self, number: u32) -> Result<[u8; N]> {
        let end = self.offset + N;
        let slice = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| protobuf_error(format!("truncated fixed field {number}")))?;
        self.offset = end;
        Ok(slice.try_into().expect("length checked above"))
    }
}

#[must_use]
pub fn fields(data: &[u8]) -> Fields<'_> {
    Fields {
        data,
        offset: 0,
        failed: false,
    }
}

#[must_use]
pub fn first_bytes(data: &[u8], number: u32) -> Option<&[u8]> {
    fields(data)
        .flatten()
        .find(|field| field.number == number)
        .and_then(|field| field.bytes())
}

#[must_use]
pub fn first_varint(data: &[u8], number: u32) -> Option<u64> {
    fields(data)
        .flatten()
        .find(|field| field.number == number)
        .and_then(|field| field.varint())
}

#[must_use]
pub fn first_fixed32(data: &[u8], number: u32) -> Option<u32> {
    fields(data)
        .flatten()
        .find(|field| field.number == number)
        .and_then(|field| field.fixed32())
}

#[derive(Debug, Default, Clone)]
pub struct Message(Vec<u8>);

impl Message {
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn varint(mut self, number: u32, value: u64) -> Self {
        self.push_tag(number, VARINT);
        push_varint(&mut self.0, value);
        self
    }

    #[must_use]
    pub fn bytes(mut self, number: u32, value: &[u8]) -> Self {
        self.push_tag(number, LENGTH_DELIMITED);
        push_varint(&mut self.0, value.len() as u64);
        self.0.extend_from_slice(value);
        self
    }

    #[must_use]
    pub fn field(self, field: &Field<'_>) -> Self {
        match field.value {
            Value::Varint(value) => self.varint(field.number, value),
            Value::Bytes(value) => self.bytes(field.number, value),
            Value::Fixed64(value) => self.fixed64(field.number, value),
            Value::Fixed32(value) => self.fixed32(field.number, value),
        }
    }

    #[must_use]
    pub fn fixed64(mut self, number: u32, value: u64) -> Self {
        self.push_tag(number, FIXED64);
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    #[must_use]
    pub fn fixed32(mut self, number: u32, value: u32) -> Self {
        self.push_tag(number, FIXED32);
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    fn push_tag(&mut self, number: u32, wire_type: u32) {
        debug_assert!(number > 0, "protobuf field numbers start at one");
        push_varint(&mut self.0, u64::from(number) << 3 | u64::from(wire_type));
    }
}

fn push_varint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push(u8::try_from(value & 0x7F).expect("masked to seven bits") | 0x80);
        value >>= 7;
    }
    output.push(u8::try_from(value).expect("loop exits below 0x80"));
}

fn protobuf_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_supported_wire_type() {
        let encoded = Message::new()
            .varint(1, 300)
            .bytes(2, b"ticket")
            .fixed32(3, 0xDEAD_BEEF)
            .fixed64(4, 0x0123_4567_89AB_CDEF)
            .into_vec();
        let decoded = fields(&encoded).collect::<Result<Vec<_>>>().expect("valid");
        assert_eq!(
            decoded,
            [
                Field {
                    number: 1,
                    value: Value::Varint(300)
                },
                Field {
                    number: 2,
                    value: Value::Bytes(b"ticket")
                },
                Field {
                    number: 3,
                    value: Value::Fixed32(0xDEAD_BEEF)
                },
                Field {
                    number: 4,
                    value: Value::Fixed64(0x0123_4567_89AB_CDEF)
                },
            ]
        );
        assert_eq!(first_fixed32(&encoded, 3), Some(0xDEAD_BEEF));
        assert_eq!(first_fixed32(&encoded, 1), None);
    }

    #[test]
    fn varints_use_the_minimum_byte_count() {
        assert_eq!(Message::new().varint(1, 0).into_vec(), [0x08, 0x00]);
        assert_eq!(Message::new().varint(1, 300).into_vec(), [0x08, 0xAC, 0x02]);
        assert_eq!(
            Message::new().varint(2, u64::MAX).into_vec(),
            [
                0x10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01
            ]
        );
    }

    #[test]
    fn truncated_and_malformed_messages_are_rejected() {
        assert!(
            fields(&[0x0A, 0x05, b'a'])
                .collect::<Result<Vec<_>>>()
                .is_err()
        );
        assert!(fields(&[0x00, 0x01]).collect::<Result<Vec<_>>>().is_err());
        assert!(fields(&[0x0B]).collect::<Result<Vec<_>>>().is_err());
        assert!(fields(&[0x08, 0x80]).collect::<Result<Vec<_>>>().is_err());
    }

    #[test]
    fn accessors_take_the_first_match() {
        let encoded = Message::new()
            .varint(1, 7)
            .bytes(2, b"first")
            .bytes(2, b"second")
            .into_vec();
        assert_eq!(first_varint(&encoded, 1), Some(7));
        assert_eq!(first_bytes(&encoded, 2), Some(b"first".as_slice()));
        assert_eq!(first_bytes(&encoded, 1), None);
        assert_eq!(first_varint(&encoded, 9), None);
    }
}
