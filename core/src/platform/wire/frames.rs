//! RFC 6455 framing for a peer that does not keep to RFC 6455.
//!
//! Remastered's classic edge **masks its server-to-client frames**, which the
//! specification forbids and which `tungstenite` rejects outright
//! (`MaskedFrameFromServer`). It is the same strictness mismatch as its
//! `HTTP/1.0` upgrade response: the edge is not well-formed, and the retail
//! client does not mind. So the classic channel does its own framing, and
//! unmasks whatever it is sent rather than judging who sent it.
//!
//! Ported from `sc1-research`.

use std::io::{Read, Write};

use crate::{Error, Result};

const FIN: u8 = 0x80;
const MASKED: u8 = 0x80;
const RESERVED: u8 = 0x70;
const OPCODE: u8 = 0x0F;
const LENGTH: u8 = 0x7F;
/// sentinel lengths selecting the 16- and 64-bit extended length fields.
const LENGTH_16: u8 = 0x7E;
const LENGTH_64: u8 = 0x7F;
const MAX_CONTROL_PAYLOAD: usize = 125;
const READ_CHUNK: usize = 64 * 1024;

pub mod opcode {
    pub const TEXT: u8 = 1;
    pub const BINARY: u8 = 2;
    pub const CLOSE: u8 = 8;
    pub const PING: u8 = 9;
    pub const PONG: u8 = 10;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub fin: bool,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

pub fn encode_frame(payload: &[u8], opcode: u8, mask_key: [u8; 4]) -> Result<Vec<u8>> {
    if opcode >= 8 && payload.len() > MAX_CONTROL_PAYLOAD {
        return Err(frame_error(format!(
            "control frames may carry {MAX_CONTROL_PAYLOAD} bytes at most"
        )));
    }
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(FIN | opcode);
    let length = payload.len();
    if let Some(short) = u8::try_from(length).ok().filter(|short| *short < LENGTH_16) {
        frame.push(MASKED | short);
    } else if let Ok(length) = u16::try_from(length) {
        frame.push(MASKED | LENGTH_16);
        frame.extend_from_slice(&length.to_be_bytes());
    } else {
        frame.push(MASKED | LENGTH_64);
        frame.extend_from_slice(&(length as u64).to_be_bytes());
    }
    frame.extend_from_slice(&mask_key);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask_key[index & 3]),
    );
    Ok(frame)
}

/// `Ok(None)` means the buffer holds only a partial frame.
fn decode_frame(data: &[u8]) -> Result<Option<(Frame, usize)>> {
    let Some(&[first, second]) = data.get(..2) else {
        return Ok(None);
    };
    if first & RESERVED != 0 {
        return Err(frame_error("reserved frame bits are set"));
    }
    let fin = first & FIN != 0;
    let opcode = first & OPCODE;
    // masked or not is simply read; this edge masks what it sends, and refusing
    // that is what tungstenite does and why the classic channel cannot use it
    let masked = second & MASKED != 0;
    let mut offset = 2;
    let length = match second & LENGTH {
        LENGTH_16 => {
            let Some(bytes) = data.get(offset..offset + 2) else {
                return Ok(None);
            };
            offset += 2;
            u64::from(u16::from_be_bytes(bytes.try_into().expect("two bytes")))
        }
        LENGTH_64 => {
            let Some(bytes) = data.get(offset..offset + 8) else {
                return Ok(None);
            };
            offset += 8;
            let length = u64::from_be_bytes(bytes.try_into().expect("eight bytes"));
            if length >= 1 << 63 {
                return Err(frame_error("invalid 64-bit payload length"));
            }
            length
        }
        short => u64::from(short),
    };
    if opcode >= 8 && (!fin || length > MAX_CONTROL_PAYLOAD as u64) {
        return Err(frame_error("invalid control frame"));
    }
    let length =
        usize::try_from(length).map_err(|_| frame_error("payload length exceeds this platform"))?;

    let mask_key = if masked {
        let Some(bytes) = data.get(offset..offset + 4) else {
            return Ok(None);
        };
        offset += 4;
        Some(<[u8; 4]>::try_from(bytes).expect("four bytes"))
    } else {
        None
    };
    let Some(payload) = data.get(offset..offset + length) else {
        return Ok(None);
    };
    let payload = match mask_key {
        Some(key) => payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ key[index & 3])
            .collect(),
        None => payload.to_vec(),
    };
    Ok(Some((
        Frame {
            fin,
            opcode,
            payload,
        },
        offset + length,
    )))
}

/// frames read from and written to an already-upgraded stream.
pub struct Framing<S> {
    stream: S,
    buffer: Vec<u8>,
    scratch: Box<[u8]>,
}

impl<S: Read + Write> Framing<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            scratch: vec![0; READ_CHUNK].into_boxed_slice(),
        }
    }

    pub fn stream_ref(&self) -> &S {
        &self.stream
    }

    /// the SDK masks data frames with a zero key — legal, since any key is, and
    /// what the retail client does. Control frames get a random one.
    pub fn send_binary(&mut self, payload: &[u8]) -> Result<()> {
        self.send(payload, opcode::BINARY, [0; 4])
    }

    pub fn send_text(&mut self, payload: &str) -> Result<()> {
        self.send(payload.as_bytes(), opcode::TEXT, [0; 4])
    }

    pub fn send_pong(&mut self, payload: &[u8]) -> Result<()> {
        self.send(payload, opcode::PONG, random_mask())
    }

    pub fn send_close(&mut self) -> Result<()> {
        self.send(&[], opcode::CLOSE, random_mask())
    }

    fn send(&mut self, payload: &[u8], opcode: u8, mask_key: [u8; 4]) -> Result<()> {
        let frame = encode_frame(payload, opcode, mask_key)?;
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Frame> {
        loop {
            if let Some((frame, consumed)) = decode_frame(&self.buffer)? {
                self.buffer.drain(..consumed);
                return Ok(frame);
            }
            match self.stream.read(&mut self.scratch) {
                Ok(0) => return Err(frame_error("server closed the connection")),
                Ok(count) => {
                    let chunk = &self.scratch[..count];
                    self.buffer.extend_from_slice(chunk);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

fn random_mask() -> [u8; 4] {
    use rand::RngCore as _;
    let mut key = [0u8; 4];
    rand::rng().fill_bytes(&mut key);
    key
}

fn frame_error(message: impl Into<String>) -> Error {
    Error::Transport(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(payload: &[u8], opcode: u8, mask: [u8; 4]) -> Frame {
        let encoded = encode_frame(payload, opcode, mask).expect("encodable");
        let (frame, consumed) = decode_frame(&encoded).expect("valid").expect("complete");
        assert_eq!(consumed, encoded.len());
        frame
    }

    #[test]
    fn a_masked_frame_from_either_side_is_unmasked_the_same_way() {
        // the classic edge masks what it sends, which the specification forbids
        // and tungstenite refuses. the payload is recovered regardless of who
        // masked it, which is the whole reason this module exists.
        let frame = round_trip(b"hello", opcode::BINARY, [0x11, 0x22, 0x33, 0x44]);
        assert_eq!(frame.payload, b"hello");
        assert!(frame.fin);
        assert_eq!(frame.opcode, opcode::BINARY);
    }

    #[test]
    fn a_zero_mask_leaves_the_payload_verbatim_but_still_sets_the_mask_bit() {
        // what the SDK sends; the bit must be set even though the key is zero,
        // or the peer reads the header wrong
        let encoded = encode_frame(b"hello", opcode::BINARY, [0; 4]).expect("encodable");
        assert_eq!(encoded[1] & MASKED, MASKED);
        assert!(encoded.ends_with(b"hello"));
    }

    #[test]
    fn every_length_form_survives_the_round_trip() {
        for length in [0usize, 1, 125, 126, 127, 1024, 65535, 65536] {
            let payload = vec![0xA5; length];
            let frame = round_trip(&payload, opcode::BINARY, [1, 2, 3, 4]);
            assert_eq!(frame.payload.len(), length, "at length {length}");
        }
    }

    #[test]
    fn a_partial_frame_asks_for_more_rather_than_failing() {
        let encoded = encode_frame(&[0u8; 300], opcode::BINARY, [1, 2, 3, 4]).expect("encodable");
        for cut in [0, 1, 2, 4, 8, encoded.len() - 1] {
            assert!(
                decode_frame(&encoded[..cut])
                    .expect("not malformed")
                    .is_none(),
                "a {cut}-byte prefix should be incomplete, not an error"
            );
        }
    }

    #[test]
    fn malformed_frames_are_refused() {
        // reserved bits set
        assert!(decode_frame(&[0xF2, 0x00]).is_err());
        // a control frame that is fragmented
        assert!(decode_frame(&[0x08, 0x00]).is_err());
        // a control frame carrying more than it may
        assert!(encode_frame(&[0u8; 126], opcode::PING, [0; 4]).is_err());
    }
}
