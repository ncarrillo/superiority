use std::io::{ErrorKind, Read, Write};

use crate::{
    Error, Result,
    bsn::bits::{BitReader, RoutingHeader},
    native::{
        crypto::{Rc4State, derive_transport_rc4_keys},
        protocol::{Protocol, Record},
    },
};

const READ_CHUNK_SIZE: usize = 4096;

pub struct RecordStream<S> {
    stream: S,
    protocol: Protocol,
    buffer: Vec<u8>,
    inbound_cipher: Option<Rc4State>,
    outbound_cipher: Option<Rc4State>,
}

impl<S> RecordStream<S> {
    #[must_use]
    pub fn new(stream: S, protocol: Protocol) -> Self {
        Self {
            stream,
            protocol,
            buffer: Vec::new(),
            inbound_cipher: None,
            outbound_cipher: None,
        }
    }

    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    #[must_use]
    pub const fn protection_enabled(&self) -> bool {
        self.inbound_cipher.is_some()
    }

    #[must_use]
    pub const fn protocol(&self) -> &Protocol {
        &self.protocol
    }

    #[must_use]
    pub const fn get_ref(&self) -> &S {
        &self.stream
    }

    pub const fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    #[must_use]
    pub fn into_inner(self) -> S {
        self.stream
    }

    pub fn enable_protection(&mut self, transport_key: &[u8]) -> Result<()> {
        if self.inbound_cipher.is_some() || self.outbound_cipher.is_some() {
            return Err(native_error(
                "native transport protection is already enabled",
            ));
        }
        let (inbound_key, outbound_key) = derive_transport_rc4_keys(transport_key)?;
        let mut inbound_cipher = Rc4State::new(&inbound_key)?;
        let outbound_cipher = Rc4State::new(&outbound_key)?;
        inbound_cipher.apply_in_place(&mut self.buffer);
        self.inbound_cipher = Some(inbound_cipher);
        self.outbound_cipher = Some(outbound_cipher);
        Ok(())
    }

    fn decode_buffer(&mut self) -> Result<Record> {
        let mut reader = BitReader::new(&self.buffer, None)?;
        let command_id = u8::try_from(reader.read(6)?).expect("six bits fit in u8");
        let service_explicit = reader.read(1)? != 0;
        if !service_explicit {
            let response = Self::decode_command_response(reader, command_id)?;
            self.buffer.drain(..response.byte_count);
            return Ok(response);
        }
        let service_slot = u8::try_from(reader.read(4)?).expect("four bits fit in u8");
        let header = RoutingHeader {
            command_id,
            service_slot: Some(service_slot),
            bit_count: reader.position(),
        };
        let payload_start = reader.position();
        let (type_id, value) = self
            .protocol
            .decode_incoming_from(&mut reader, header)
            .inspect_err(|error| {
                if !matches!(error, Error::IncompleteFrame(_))
                    && (std::env::var_os("SUPERIORITY_TRACE").is_some()
                    || std::env::var_os("SUPERIORITY_PARTY_TRACE").is_some()
                    )
                {
                    eprintln!(
                        "superiority: failed inbound route slot={:?} command={} at bit {}: {error:?}; buffer={}",
                        header.service_slot,
                        header.command_id,
                        reader.position(),
                        hex::encode(&self.buffer),
                    );
                }
            })?;
        let payload_bit_count = reader.position() - payload_start;
        let logical_bits = reader.position();
        let byte_count = reader.position().div_ceil(8);
        let padding = byte_count * 8 - reader.position();
        if padding != 0 && reader.read(padding)? != 0 {
            return Err(native_error("native record has non-zero padding"));
        }
        super::inspect::capture_incoming(
            &self.protocol,
            header,
            &self.buffer[..byte_count],
            logical_bits,
        );
        self.buffer.drain(..byte_count);
        Ok(Record {
            header,
            type_id,
            value,
            payload_bit_count,
            byte_count,
        })
    }

    fn decode_command_response(mut reader: BitReader<'_>, command_id: u8) -> Result<Record> {
        let payload_start = reader.position();
        let result = u16::try_from(reader.read(9)?).expect("nine bits fit in u16");
        let logical_bits = reader.position();
        let byte_count = logical_bits.div_ceil(8);
        Ok(Record {
            header: RoutingHeader {
                command_id,
                service_slot: None,
                bit_count: payload_start,
            },
            type_id: 0,
            value: crate::native::model::Payload::CommandResponse(
                crate::native::model::CommandResponse { command_id, result },
            ),
            payload_bit_count: logical_bits - payload_start,
            byte_count,
        })
    }
}

impl<S: Read + Write> RecordStream<S> {
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        if let Some(cipher) = &mut self.outbound_cipher {
            let protected = cipher.apply(data);
            self.stream.write_all(&protected)?;
        } else {
            self.stream.write_all(data)?;
        }
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Record> {
        loop {
            if self.buffer.len() >= 2 {
                match self.decode_buffer() {
                    Ok(record) => {
                        return Ok(record);
                    }
                    Err(Error::IncompleteFrame(_)) => {}
                    Err(Error::UnmappedNativeRoute { slot, command }) => {
                        if let Some(length) = unmapped_record_length(slot, command) {
                            if self.buffer.len() >= length {
                                self.buffer.drain(..length);
                                continue;
                            }
                        } else {
                            return Err(Error::UnmappedNativeRoute { slot, command });
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
            let mut chunk = [0_u8; READ_CHUNK_SIZE];
            let read = self.stream.read(&mut chunk)?;
            if read == 0 {
                return Err(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "native Battle.net connection closed",
                )
                .into());
            }
            let chunk = &mut chunk[..read];
            if let Some(cipher) = &mut self.inbound_cipher {
                cipher.apply_in_place(chunk);
            }
            self.buffer.extend_from_slice(chunk);
        }
    }
}

const fn unmapped_record_length(slot: u8, command: u8) -> Option<usize> {
    match (slot, command) {
        // the supported protocol emits s2mp/50 as a fixed 38-byte record.
        (13, 50) => Some(38),
        _ => None,
    }
}

fn native_error(message: impl Into<String>) -> Error {
    Error::Native(message.into())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
    };

    use super::*;
    use crate::{
        bsn::bits::BitWriter,
        native::{
            model::Payload,
            protocol::{CHAT_JOIN_NOTIFY_COMMAND, CHAT_SLOT},
        },
    };

    #[derive(Default)]
    struct MemoryStream {
        reads: VecDeque<Vec<u8>>,
        writes: Vec<u8>,
    }

    impl Read for MemoryStream {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let Some(mut chunk) = self.reads.pop_front() else {
                return Ok(0);
            };
            let read = output.len().min(chunk.len());
            output[..read].copy_from_slice(&chunk[..read]);
            if read < chunk.len() {
                chunk.drain(..read);
                self.reads.push_front(chunk);
            }
            Ok(read)
        }
    }

    impl Write for MemoryStream {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            self.writes.extend_from_slice(input);
            Ok(input.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn protection_uses_persistent_directional_cipher_states() {
        let protocol = Protocol::current().unwrap();
        let key = (0_u8..64).collect::<Vec<_>>();
        let (inbound_key, outbound_key) = derive_transport_rc4_keys(&key).unwrap();
        let buffered_plaintext = b"already protected";
        let buffered_ciphertext = Rc4State::new(&inbound_key)
            .unwrap()
            .apply(buffered_plaintext);
        let mut records = RecordStream::new(MemoryStream::default(), protocol);
        records.buffer.extend_from_slice(&buffered_ciphertext);

        records.send(b"plaintext marker").unwrap();
        records.enable_protection(&key).unwrap();
        records.send(b"first").unwrap();
        records.send(b"second").unwrap();

        let expected = Rc4State::new(&outbound_key).unwrap().apply(b"firstsecond");
        assert_eq!(records.buffer, buffered_plaintext);
        assert_eq!(&records.get_ref().writes[..16], b"plaintext marker");
        assert_eq!(&records.get_ref().writes[16..], expected);
        assert!(records.enable_protection(&key).is_err());
    }

    fn named_join(name: &str, channel_index: u8, member_handle: u32, token: u32) -> Vec<u8> {
        let mut writer = BitWriter::new();
        writer
            .write(u64::from(CHAT_JOIN_NOTIFY_COMMAND), 6)
            .unwrap();
        writer.write(1, 1).unwrap();
        writer.write(u64::from(CHAT_SLOT), 4).unwrap();
        writer.write(0, 1).unwrap();
        writer.write(u64::from(member_handle), 32).unwrap();
        writer.write(u64::from(channel_index), 3).unwrap();
        writer.write(0x1111_1111, 32).unwrap();
        writer.write(0x2222_2222, 32).unwrap();
        writer.write(1, 4).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(0, 16).unwrap();
        writer.write(0, 29).unwrap();
        writer.write(0, 2).unwrap();
        writer.write(name.len() as u64, 7).unwrap();
        writer.write_bytes(name.as_bytes(), true).unwrap();
        writer.write(0, 1).unwrap();
        writer.write(0, 1).unwrap();
        writer.write(1, 1).unwrap();
        writer.write(u64::from(token), 32).unwrap();
        writer.align().unwrap();
        writer.into_bytes()
    }

    #[test]
    fn named_chat_join_preserves_the_following_record_boundary() {
        let first = named_join("Op Test2", 2, 0x1020_3040, 0x1122_3344);
        let second = named_join("Another Room", 3, 0x5060_7080, 0x5566_7788);
        let mut chunk = first;
        chunk.extend_from_slice(&second);
        let protocol = Protocol::current().unwrap();
        let stream = MemoryStream {
            reads: VecDeque::from([chunk]),
            writes: Vec::new(),
        };
        let mut records = RecordStream::new(stream, protocol);

        let first = records.receive().unwrap();
        let second = records.receive().unwrap();
        let Payload::ChatJoin(first) = first.value else {
            panic!("expected first named chat join");
        };
        let Payload::ChatJoin(second) = second.value else {
            panic!("expected second named chat join");
        };
        assert_eq!(first.channel_index, Some(2));
        assert_eq!(first.token, Some(0x1122_3344));
        assert_eq!(second.channel_index, Some(3));
        assert_eq!(second.token, Some(0x5566_7788));
        assert_eq!(records.buffered_bytes(), 0);
    }

    #[test]
    fn command_response_preserves_the_following_record_boundary() {
        let first = named_join("General", 0, 0x1020_3040, 0x1122_3344);
        let response = [0x09, 0x01];
        let second = named_join("Party", 6, 0x5060_7080, 0x5566_7788);
        let mut chunk = first;
        chunk.extend_from_slice(&response);
        chunk.extend_from_slice(&second);
        let protocol = Protocol::current().unwrap();
        let stream = MemoryStream {
            reads: VecDeque::from([chunk]),
            writes: Vec::new(),
        };
        let mut records = RecordStream::new(stream, protocol);

        let first = records.receive().unwrap();
        let response = records.receive().unwrap();
        let second = records.receive().unwrap();
        assert_eq!(first.header.service_slot, Some(CHAT_SLOT));
        assert_eq!(first.header.bit_count, 11);
        assert_eq!(response.header.service_slot, None);
        assert_eq!(response.header.command_id, 9);
        assert_eq!(response.header.bit_count, 7);
        assert_eq!(response.payload_bit_count, 9);
        assert_eq!(response.byte_count, 2);
        assert_eq!(
            response.value,
            Payload::CommandResponse(crate::native::model::CommandResponse {
                command_id: 9,
                result: 1,
            })
        );
        assert_eq!(second.header.service_slot, Some(CHAT_SLOT));
        assert_eq!(second.header.bit_count, 11);
        let Payload::ChatJoin(second) = second.value else {
            panic!("expected compact party chat join");
        };
        assert_eq!(second.channel_index, Some(6));
        assert_eq!(records.buffered_bytes(), 0);
    }

    #[test]
    fn command_response_can_be_the_first_record() {
        let protocol = Protocol::current().unwrap();
        let stream = MemoryStream {
            reads: VecDeque::from([vec![0x09, 0x00]]),
            writes: Vec::new(),
        };
        let mut records = RecordStream::new(stream, protocol);

        let response = records.receive().unwrap();
        assert_eq!(response.header.service_slot, None);
        assert_eq!(response.header.command_id, 9);
        assert_eq!(response.payload_bit_count, 9);
        assert_eq!(records.buffered_bytes(), 0);
    }

    #[test]
    fn command_response_accepts_the_observed_party_status_result() {
        let protocol = Protocol::current().unwrap();
        let stream = MemoryStream {
            reads: VecDeque::from([vec![0x09, 0x03]]),
            writes: Vec::new(),
        };
        let mut records = RecordStream::new(stream, protocol);

        let response = records.receive().unwrap();
        assert_eq!(response.payload_bit_count, 9);
        assert_eq!(response.byte_count, 2);
        assert_eq!(
            response.value,
            Payload::CommandResponse(crate::native::model::CommandResponse {
                command_id: 9,
                result: 3,
            })
        );
        assert_eq!(records.buffered_bytes(), 0);
    }

    #[test]
    fn command_response_uses_every_bit_after_the_route_header() {
        let protocol = Protocol::current().unwrap();
        let stream = MemoryStream {
            reads: VecDeque::from([vec![0x89, 0xff]]),
            writes: Vec::new(),
        };
        let mut records = RecordStream::new(stream, protocol);

        let response = records.receive().unwrap();
        assert_eq!(response.payload_bit_count, 9);
        assert_eq!(response.byte_count, 2);
        assert_eq!(
            response.value,
            Payload::CommandResponse(crate::native::model::CommandResponse {
                command_id: 9,
                result: 0x1ff,
            })
        );
    }
}
