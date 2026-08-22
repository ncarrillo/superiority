use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoutingHeader {
    pub command_id: u8,
    pub service_slot: Option<u8>,
    pub bit_count: usize,
}

impl RoutingHeader {
    #[must_use]
    pub const fn has_service(self) -> bool {
        self.service_slot.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_count: usize,
    position: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], bit_count: Option<usize>) -> Result<Self> {
        let bit_count = bit_count.unwrap_or(data.len() * 8);
        if bit_count > data.len() * 8 {
            return Err(bit_error("bit count is outside the supplied buffer"));
        }
        Ok(Self {
            data,
            bit_count,
            position: 0,
        })
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bit_count - self.position
    }

    #[must_use]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn set_position(&mut self, position: usize) -> Result<()> {
        if position > self.bit_count {
            return Err(Error::IncompleteFrame(format!(
                "seek to bit {position}, only {} buffered",
                self.bit_count
            )));
        }
        self.position = position;
        Ok(())
    }

    pub fn read(&mut self, width: usize) -> Result<u64> {
        if width > 64 {
            return Err(bit_error(
                "integer fields wider than 64 bits are unsupported",
            ));
        }
        if width > self.remaining() {
            return Err(Error::IncompleteFrame(format!(
                "need {width} bits at position {}, only {} remain",
                self.position,
                self.remaining()
            )));
        }
        let mut value = 0_u64;
        let mut remaining = width;
        while remaining > 0 {
            let byte_index = self.position / 8;
            let bit_offset = self.position % 8;
            let take = remaining.min(8 - bit_offset);
            let mask = (1_u16 << take) - 1;
            let chunk = (u16::from(self.data[byte_index]) >> bit_offset) & mask;
            value = (value << take) | u64::from(chunk);
            self.position += take;
            remaining -= take;
        }
        Ok(value)
    }

    pub fn align(&mut self) -> Result<usize> {
        let skipped = self.position.wrapping_neg() & 7;
        if skipped > 0 {
            self.read(skipped)?;
        }
        Ok(skipped)
    }

    pub fn read_raw(&mut self, bit_count: usize) -> Result<Vec<u8>> {
        let mut writer = BitWriter::new();
        for _ in 0..bit_count {
            writer.write(self.read(1)?, 1)?;
        }
        Ok(writer.into_bytes())
    }

    pub fn read_bytes(&mut self, byte_count: usize, aligned: bool) -> Result<Vec<u8>> {
        if aligned {
            self.align()?;
        }
        if self.position.is_multiple_of(8) {
            let start = self.position / 8;
            let end = start
                .checked_add(byte_count)
                .ok_or_else(|| bit_error("byte count overflows"))?;
            if end * 8 > self.bit_count {
                return Err(Error::IncompleteFrame(format!(
                    "need {} bits at position {}, only {} remain",
                    byte_count * 8,
                    self.position,
                    self.remaining()
                )));
            }
            self.position = end * 8;
            return Ok(self.data[start..end].to_vec());
        }
        self.read_raw(byte_count * 8)
    }
}

#[derive(Clone, Debug, Default)]
pub struct BitWriter {
    data: Vec<u8>,
    position: usize,
}

impl BitWriter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    pub fn write(&mut self, value: u64, width: usize) -> Result<()> {
        if width > 64 {
            return Err(bit_error(
                "integer fields wider than 64 bits are unsupported",
            ));
        }
        if width < 64 && value >= (1_u64 << width) {
            return Err(bit_error(format!(
                "value {value} does not fit in {width} bits"
            )));
        }
        self.ensure_capacity(self.position + width);
        let mut remaining = width;
        while remaining > 0 {
            let byte_index = self.position / 8;
            let bit_offset = self.position % 8;
            let take = remaining.min(8 - bit_offset);
            let shift = remaining - take;
            let mask = u8::try_from((1_u16 << take) - 1).expect("at most eight bits");
            let chunk =
                u8::try_from((value >> shift) & u64::from(mask)).expect("masked to one byte");
            self.data[byte_index] &= !(mask << bit_offset);
            self.data[byte_index] |= chunk << bit_offset;
            self.position += take;
            remaining -= take;
        }
        Ok(())
    }

    pub fn write_raw(&mut self, data: &[u8], bit_count: Option<usize>) -> Result<()> {
        let bit_count = bit_count.unwrap_or(data.len() * 8);
        if bit_count > data.len() * 8 {
            return Err(bit_error("bit count is outside the supplied buffer"));
        }
        self.ensure_capacity(self.position + bit_count);
        for source_position in 0..bit_count {
            let bit = (data[source_position / 8] >> (source_position & 7)) & 1;
            if bit != 0 {
                self.data[self.position / 8] |= 1 << (self.position & 7);
            }
            self.position += 1;
        }
        Ok(())
    }

    pub fn align(&mut self) -> Result<usize> {
        let padding = self.position.wrapping_neg() & 7;
        if padding > 0 {
            self.write(0, padding)?;
        }
        Ok(padding)
    }

    pub fn write_bytes(&mut self, data: &[u8], aligned: bool) -> Result<()> {
        if aligned {
            self.align()?;
        }
        if self.position.is_multiple_of(8) {
            self.ensure_capacity(self.position + data.len() * 8);
            let start = self.position / 8;
            self.data[start..start + data.len()].copy_from_slice(data);
            self.position += data.len() * 8;
            return Ok(());
        }
        self.write_raw(data, None)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.position.div_ceil(8)]
    }

    #[must_use]
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.data.truncate(self.position.div_ceil(8));
        self.data
    }

    fn ensure_capacity(&mut self, bit_count: usize) {
        self.data
            .resize(bit_count.div_ceil(8).max(self.data.len()), 0);
    }
}

pub fn encode_routing_header(command_id: u8, service_slot: Option<u8>) -> Result<(Vec<u8>, usize)> {
    if command_id > 0x3f {
        return Err(bit_error("command id must fit in 6 bits"));
    }
    if service_slot.is_some_and(|slot| slot > 0x0f) {
        return Err(bit_error("service slot must fit in 4 bits"));
    }
    let mut writer = BitWriter::new();
    writer.write(u64::from(command_id), 6)?;
    writer.write(u64::from(service_slot.is_some()), 1)?;
    if let Some(slot) = service_slot {
        writer.write(u64::from(slot), 4)?;
    }
    let bit_count = writer.position();
    Ok((writer.into_bytes(), bit_count))
}

pub fn decode_routing_header(data: &[u8], bit_count: Option<usize>) -> Result<RoutingHeader> {
    let mut reader = BitReader::new(data, bit_count)?;
    let command_id = u8::try_from(reader.read(6)?).expect("six bits fit in u8");
    let service_slot = if reader.read(1)? != 0 {
        Some(u8::try_from(reader.read(4)?).expect("four bits fit in u8"))
    } else {
        None
    };
    Ok(RoutingHeader {
        command_id,
        service_slot,
        bit_count: reader.position(),
    })
}

pub fn concat_bits(parts: &[(&[u8], usize)]) -> Result<(Vec<u8>, usize)> {
    let mut writer = BitWriter::new();
    for &(data, bit_count) in parts {
        writer.write_raw(data, Some(bit_count))?;
    }
    let bit_count = writer.position();
    Ok((writer.into_bytes(), bit_count))
}

pub fn strip_routing_header(
    data: &[u8],
    bit_count: Option<usize>,
) -> Result<(RoutingHeader, Vec<u8>, usize)> {
    let total_bits = bit_count.unwrap_or(data.len() * 8);
    let header = decode_routing_header(data, Some(total_bits))?;
    let payload_bits = total_bits - header.bit_count;
    let mut reader = BitReader::new(data, Some(total_bits))?;
    reader.set_position(header.bit_count)?;
    Ok((header, reader.read_raw(payload_bits)?, payload_bits))
}

fn bit_error(message: impl Into<String>) -> Error {
    Error::BsnWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_byte_value_uses_high_chunk_first() {
        let mut writer = BitWriter::new();
        writer.write(0, 6).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(14, 4).unwrap();
        assert_eq!(writer.position(), 11);
        assert_eq!(writer.as_bytes(), [0xc0, 0x06]);

        let mut reader = BitReader::new(writer.as_bytes(), Some(writer.position())).unwrap();
        assert_eq!(reader.read(6).unwrap(), 0);
        assert_eq!(reader.read(1).unwrap(), 1);
        assert_eq!(reader.read(4).unwrap(), 14);
    }

    #[test]
    fn observed_routing_headers_decode() {
        for (data, command_id, slot) in [
            ([0x40, 0x0c], 0, 4),
            ([0x41, 0x05], 1, 5),
            ([0xc0, 0x06], 0, 14),
        ] {
            let header = decode_routing_header(&data, None).unwrap();
            assert_eq!(header.command_id, command_id);
            assert_eq!(header.service_slot, Some(slot));
            assert_eq!(header.bit_count, 11);
        }
    }

    #[test]
    fn payload_round_trips_across_header_boundary() {
        let (header, header_bits) = encode_routing_header(22, Some(5)).unwrap();
        let payload = [0xd3, 0x5a];
        let (framed, framed_bits) = concat_bits(&[(&header, header_bits), (&payload, 13)]).unwrap();
        let (decoded, stripped, stripped_bits) =
            strip_routing_header(&framed, Some(framed_bits)).unwrap();
        assert_eq!(decoded.command_id, 22);
        assert_eq!(decoded.service_slot, Some(5));
        assert_eq!(stripped_bits, 13);
        assert_eq!(stripped[0], payload[0]);
        assert_eq!(stripped[1] & 0x1f, payload[1] & 0x1f);
    }

    #[test]
    fn aligned_bytes_round_trip() {
        let mut writer = BitWriter::new();
        writer.write(5, 3).unwrap();
        assert_eq!(writer.align().unwrap(), 5);
        writer.write_bytes(b"abc", true).unwrap();

        let mut reader = BitReader::new(writer.as_bytes(), Some(writer.position())).unwrap();
        assert_eq!(reader.read(3).unwrap(), 5);
        assert_eq!(reader.read_bytes(3, true).unwrap(), b"abc");
        assert_eq!(reader.remaining(), 0);
    }
}
