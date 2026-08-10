use std::io::{Read, Write};

use prost::Message;

use crate::{Error, Result, bgs::generated::bgs::protocol::Header};

const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

pub type RpcHeader = Header;

#[derive(Clone, Debug, PartialEq)]
pub struct RpcFrame {
    pub header: RpcHeader,
    pub body: Vec<u8>,
}

#[must_use]
pub fn service_hash(name: &str) -> u32 {
    name.as_bytes().iter().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

pub fn encode_frame(header: &RpcHeader, body: &[u8]) -> Result<Vec<u8>> {
    let mut framed_header = header.clone();
    if !body.is_empty() || header.size.is_some() {
        framed_header.size = Some(
            u32::try_from(body.len())
                .map_err(|_| wire_error("RPC body exceeds the protocol's uint32 size"))?,
        );
    }
    let encoded_header = framed_header.encode_to_vec();
    let header_length = u16::try_from(encoded_header.len())
        .map_err(|_| wire_error("RPC header exceeds the 16-bit frame prefix"))?;
    let mut output = Vec::with_capacity(2 + encoded_header.len() + body.len());
    output.extend_from_slice(&header_length.to_be_bytes());
    output.extend_from_slice(&encoded_header);
    output.extend_from_slice(body);
    Ok(output)
}

pub fn write_frame(stream: &mut impl Write, header: &RpcHeader, body: &[u8]) -> Result<()> {
    stream.write_all(&encode_frame(header, body)?)?;
    Ok(())
}

pub fn read_frame(stream: &mut impl Read) -> Result<RpcFrame> {
    read_frame_with_limit(stream, DEFAULT_MAX_BODY_SIZE)
}

pub fn read_frame_with_limit(stream: &mut impl Read, max_body_size: usize) -> Result<RpcFrame> {
    let mut length = [0_u8; 2];
    stream.read_exact(&mut length)?;
    let header_length = usize::from(u16::from_be_bytes(length));
    if header_length == 0 {
        return Err(wire_error("RPC frame has an empty header"));
    }
    let mut encoded_header = vec![0; header_length];
    stream.read_exact(&mut encoded_header)?;
    let header = RpcHeader::decode(encoded_header.as_slice())?;
    let body_length = header.size.unwrap_or_default() as usize;
    if body_length > max_body_size {
        return Err(wire_error(format!(
            "RPC body length {body_length} exceeds limit {max_body_size}"
        )));
    }
    let mut body = vec![0; body_length];
    stream.read_exact(&mut body)?;
    Ok(RpcFrame { header, body })
}

fn wire_error(message: impl Into<String>) -> Error {
    Error::BgsWire(message.into())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn known_service_hashes_match() {
        assert_eq!(
            service_hash("bnet.protocol.connection.ConnectionService"),
            0x6544_6991
        );
        assert_eq!(
            service_hash("bnet.protocol.authentication.AuthenticationServer"),
            0x0dec_fc01
        );
        assert_eq!(
            service_hash("bnet.protocol.game_utilities.GameUtilities"),
            0x3fc1_274d
        );
    }

    #[test]
    fn initial_connect_frame_is_exact() {
        use crate::bgs::generated::bgs::protocol::connection::v1::ConnectRequest;

        let header = RpcHeader {
            service_id: 0,
            method_id: Some(1),
            token: 0,
            service_hash: Some(service_hash("bnet.protocol.connection.ConnectionService")),
            ..RpcHeader::default()
        };
        let request = ConnectRequest {
            use_bindless_rpc: Some(true),
            ..ConnectRequest::default()
        };
        assert_eq!(
            encode_frame(&header, &request.encode_to_vec()).unwrap(),
            hex("000d08001001180028025d916944651801")
        );
    }

    #[test]
    fn frame_round_trips() {
        let header = RpcHeader {
            service_id: 0,
            method_id: Some(1),
            token: 0,
            ..RpcHeader::default()
        };
        let body = [0x18, 0x01];
        let encoded = encode_frame(&header, &body).unwrap();
        assert_eq!(
            read_frame(&mut Cursor::new(encoded)).unwrap(),
            RpcFrame {
                header: RpcHeader {
                    size: Some(2),
                    ..header
                },
                body: body.to_vec(),
            }
        );
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
