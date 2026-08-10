use std::{
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use prost::Message as _;
use tungstenite::{
    ClientRequestBuilder, Message, WebSocket, client_tls_with_config, http::Uri,
    protocol::WebSocketConfig, stream::MaybeTlsStream,
};

use crate::{
    Error, Result,
    wire::protobuf::{RpcFrame, RpcHeader},
};

const PROTOCOL: &str = "v1.rpc.battle.net";
const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

pub struct RpcSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl RpcSocket {
    pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self> {
        let address = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| transport_error("Battle.net hostname resolved to no addresses"))?;
        let stream = TcpStream::connect_timeout(&address, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.set_nodelay(true)?;

        let uri = format!("wss://{host}:{port}/")
            .parse::<Uri>()
            .map_err(|error| transport_error(format!("invalid BGS URI: {error}")))?;
        let request = ClientRequestBuilder::new(uri).with_sub_protocol(PROTOCOL);
        let config = WebSocketConfig::default()
            .write_buffer_size(0)
            .max_message_size(Some(MAX_MESSAGE_SIZE))
            .max_frame_size(Some(MAX_MESSAGE_SIZE));
        let (socket, response) = client_tls_with_config(request, stream, Some(config), None)
            .map_err(|error| transport_error(format!("BGS websocket handshake failed: {error}")))?;
        let selected = response
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|value| value.to_str().ok());
        if selected != Some(PROTOCOL) {
            return Err(transport_error(
                "Battle.net selected an unexpected websocket subprotocol",
            ));
        }
        Ok(Self { socket })
    }

    pub fn send(&mut self, header: &RpcHeader, body: &[u8]) -> Result<()> {
        let encoded_header = header.encode_to_vec();
        let header_length = u16::try_from(encoded_header.len())
            .map_err(|_| wire_error("RPC header exceeds the 16-bit frame prefix"))?;
        let mut payload = Vec::with_capacity(2 + encoded_header.len() + body.len());
        payload.extend_from_slice(&header_length.to_be_bytes());
        payload.extend_from_slice(&encoded_header);
        payload.extend_from_slice(body);
        self.socket.send(Message::binary(payload))?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<RpcFrame> {
        loop {
            match self.socket.read()? {
                Message::Binary(message) => return decode_bgs_message(&message),
                Message::Close(_) => {
                    return Err(transport_error("BGS websocket received a close frame"));
                }
                Message::Text(_) => {
                    return Err(wire_error("BGS websocket received a text message"));
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    pub fn close(&mut self) -> Result<()> {
        self.socket.close(None)?;
        Ok(())
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        let stream = match self.socket.get_ref() {
            MaybeTlsStream::Plain(stream) => stream,
            MaybeTlsStream::NativeTls(stream) => stream.get_ref(),
            _ => return Err(transport_error("unsupported BGS TLS backend")),
        };
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        Ok(())
    }
}

fn decode_bgs_message(message: &[u8]) -> Result<RpcFrame> {
    if message.len() < 2 {
        return Err(wire_error("BGS websocket message has no header prefix"));
    }
    let header_length = usize::from(u16::from_be_bytes([message[0], message[1]]));
    if header_length == 0 || message.len() < 2 + header_length {
        return Err(wire_error("BGS websocket header is truncated"));
    }
    let header = RpcHeader::decode(&message[2..2 + header_length])?;
    let body = message[2 + header_length..].to_vec();
    if header
        .size
        .is_some_and(|size| usize::try_from(size).expect("u32 fits in usize") != body.len())
    {
        return Err(wire_error("BGS websocket body length mismatch"));
    }
    Ok(RpcFrame { header, body })
}

fn wire_error(message: impl Into<String>) -> Error {
    Error::BgsWire(message.into())
}

fn transport_error(message: impl Into<String>) -> Error {
    Error::Transport(message.into())
}

#[cfg(test)]
mod tests {
    use crate::wire::protobuf::RpcHeader;

    use super::*;

    #[test]
    fn bgs_adapter_decodes_one_binary_message() {
        let header = RpcHeader {
            service_id: 0xfe,
            token: 7,
            size: Some(3),
            ..RpcHeader::default()
        };
        let encoded = header.encode_to_vec();
        let mut message = Vec::new();
        message.extend_from_slice(&u16::try_from(encoded.len()).unwrap().to_be_bytes());
        message.extend_from_slice(&encoded);
        message.extend_from_slice(b"rpc");
        assert_eq!(
            decode_bgs_message(&message).unwrap(),
            RpcFrame {
                header,
                body: b"rpc".to_vec(),
            }
        );
    }

    #[test]
    fn bgs_adapter_rejects_mismatched_body_size() {
        let header = RpcHeader {
            service_id: 0xfe,
            token: 7,
            size: Some(4),
            ..RpcHeader::default()
        };
        let encoded = header.encode_to_vec();
        let mut message = Vec::new();
        message.extend_from_slice(&u16::try_from(encoded.len()).unwrap().to_be_bytes());
        message.extend_from_slice(&encoded);
        message.extend_from_slice(b"rpc");
        assert!(decode_bgs_message(&message).is_err());
    }
}
