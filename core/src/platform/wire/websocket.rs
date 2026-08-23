use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use prost::Message as _;
use rand::RngCore as _;
use sha1::{Digest as _, Sha1};
use tungstenite::{
    ClientRequestBuilder, Message, WebSocket, client::IntoClientRequest, client_tls_with_config,
    http::Uri, protocol::WebSocketConfig, stream::MaybeTlsStream,
};

use crate::{
    Error, Result,
    native::inspect::{
        Direction, capture_bgs, capture_http_request, capture_http_response, capture_invalid_bgs,
    },
    wire::frames::{Framing, opcode},
    wire::protobuf::{RpcFrame, RpcHeader},
};

const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
pub(crate) const MAX_HANDSHAKE_BYTES: usize = 64 * 1024;
/// RFC 6455's magic string, which the accept digest is built over.
const WEBSOCKET_ACCEPT_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// How a connection is framed once it is upgraded.
///
/// Two peers, two levels of conformance. The account channels are well-formed
/// and `tungstenite` handles them. Remastered's classic edge masks its
/// server-to-client frames, which the specification forbids and `tungstenite`
/// refuses outright, so that one is framed here — see [`super::frames`].
enum Transport {
    Strict(Box<WebSocket<MaybeTlsStream<TcpStream>>>),
    Classic(Box<Framing<native_tls::TlsStream<TcpStream>>>),
}

impl Transport {
    /// Both transports speak in `tungstenite::Message` so the socket above does
    /// not have to know which one it has.
    fn send(&mut self, message: Message) -> Result<()> {
        match self {
            Self::Strict(socket) => {
                socket.send(message)?;
                Ok(())
            }
            Self::Classic(framing) => match message {
                Message::Binary(payload) => framing.send_binary(&payload),
                Message::Text(payload) => framing.send_text(&payload),
                Message::Pong(payload) => framing.send_pong(&payload),
                Message::Close(_) => framing.send_close(),
                other => Err(wire_error(format!(
                    "the classic channel cannot send {other:?}"
                ))),
            },
        }
    }

    fn read(&mut self) -> Result<Message> {
        match self {
            Self::Strict(socket) => Ok(socket.read()?),
            Self::Classic(framing) => {
                let frame = framing.receive()?;
                Ok(match frame.opcode {
                    opcode::BINARY => Message::binary(frame.payload),
                    opcode::TEXT => Message::text(
                        String::from_utf8(frame.payload)
                            .map_err(|_| wire_error("text frame is not valid UTF-8"))?,
                    ),
                    opcode::CLOSE => Message::Close(None),
                    // answered here rather than surfaced: a keepalive is not an
                    // event any caller above wants to handle
                    opcode::PING => {
                        framing.send_pong(&frame.payload)?;
                        Message::Pong(frame.payload.into())
                    }
                    opcode::PONG => Message::Pong(frame.payload.into()),
                    other => return Err(wire_error(format!("unexpected frame opcode {other}"))),
                })
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        match self {
            Self::Strict(socket) => {
                socket.close(None)?;
                Ok(())
            }
            Self::Classic(framing) => framing.send_close(),
        }
    }

    fn tcp(&self) -> Result<&TcpStream> {
        match self {
            Self::Strict(socket) => match socket.get_ref() {
                MaybeTlsStream::Plain(stream) => Ok(stream),
                MaybeTlsStream::NativeTls(stream) => Ok(stream.get_ref()),
                _ => Err(transport_error("unsupported BGS TLS backend")),
            },
            Self::Classic(framing) => Ok(framing.stream_ref().get_ref()),
        }
    }
}

/// Which endpoint a socket is opened against. Battle.net serves more than one
/// on the same host and port: the account service answers at `/` under the
/// protobuf RPC subprotocol, and a product's own channel answers at its own
/// path — Remastered's classic RPC is `/S1/v2/rpc/client`.
///
/// The path is borrowed rather than `'static`: Remastered's is not known until
/// Aurora names the server, and it can carry a query string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketProfile<'a> {
    pub path: &'a str,
    /// `None` asks for no subprotocol at all, which is not the same as asking
    /// for one and being given nothing.
    pub subprotocol: Option<&'a str>,
    /// Accept an upgrade response that is a good `101` but not a well-formed
    /// HTTP/1.1 one.
    ///
    /// Remastered's classic edge answers with a status line tungstenite refuses
    /// outright — "HTTP version must be 1.1 or higher" — while being an
    /// otherwise valid upgrade, digest and all. Retail's own client only ever
    /// checks the status code and the accept digest, so this checks the same
    /// two things and hands the upgraded stream to tungstenite for framing.
    pub lenient_upgrade: bool,
}

impl SocketProfile<'static> {
    /// The account service: protobuf RPC at the root.
    pub const BGS: Self = Self {
        path: "/",
        subprotocol: Some("v1.rpc.battle.net"),
        lenient_upgrade: false,
    };
}

/// A transform sitting between the RPC framing and the wire. Remastered's
/// classic channel wraps every payload in a feedback XOR keyed off the
/// handshake nonce; the account service wraps nothing.
pub trait PayloadTransform: Send {
    fn encode(&self, payload: &[u8]) -> Vec<u8>;
    fn decode(&self, payload: &[u8]) -> Vec<u8>;
}

pub struct RpcSocket {
    socket: Transport,
    /// the `Sec-WebSocket-Key` this connection handshook with. kept because a
    /// transform may be keyed off it, and tungstenite generates it internally —
    /// it is only readable from the request we built.
    handshake_key: String,
    transform: Option<Box<dyn PayloadTransform>>,
    /// how long one answer may take. the socket's read timeout only bounds a
    /// single read, and a keepalive is a read: a service that pings while
    /// answering nothing resets that clock forever. this bounds the answer.
    patience: Duration,
}

impl RpcSocket {
    pub fn connect(
        host: &str,
        port: u16,
        timeout: Duration,
        profile: SocketProfile<'_>,
    ) -> Result<Self> {
        let address = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| transport_error("Battle.net hostname resolved to no addresses"))?;
        let stream = TcpStream::connect_timeout(&address, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.set_nodelay(true)?;

        let uri = format!("wss://{host}:{port}{}", profile.path)
            .parse::<Uri>()
            .map_err(|error| transport_error(format!("invalid BGS URI: {error}")))?;
        let request_url = uri.to_string();
        let mut builder = ClientRequestBuilder::new(uri);
        if let Some(subprotocol) = profile.subprotocol {
            builder = builder.with_sub_protocol(subprotocol);
        }
        let request = builder
            .into_client_request()
            .map_err(|error| transport_error(format!("invalid BGS request: {error}")))?;
        // read back the nonce tungstenite generated: a payload transform may be
        // derived from it, and this is the only place it is visible
        let handshake_key = request
            .headers()
            .get("sec-websocket-key")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| transport_error("websocket request carried no Sec-WebSocket-Key"))?
            .to_owned();
        capture_http_request("GET", &request_url, &http_headers(request.headers()), &[]);
        let config = WebSocketConfig::default()
            .write_buffer_size(0)
            .max_message_size(Some(MAX_MESSAGE_SIZE))
            .max_frame_size(Some(MAX_MESSAGE_SIZE));
        if profile.lenient_upgrade {
            let (tls, handshake_key) = upgrade_leniently(host, port, stream, profile)?;
            return Ok(Self {
                socket: Transport::Classic(Box::new(Framing::new(tls))),
                handshake_key,
                transform: None,
                patience: timeout,
            });
        }
        let (socket, response) = client_tls_with_config(request, stream, Some(config), None)
            .map_err(|error| transport_error(format!("BGS websocket handshake failed: {error}")))?;
        let selected = response
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|value| value.to_str().ok());
        capture_http_response(
            "GET",
            &request_url,
            response.status().as_u16(),
            &http_headers(response.headers()),
            &[],
        );
        if selected != profile.subprotocol {
            return Err(transport_error(
                "Battle.net selected an unexpected websocket subprotocol",
            ));
        }
        Ok(Self {
            socket: Transport::Strict(Box::new(socket)),
            handshake_key,
            transform: None,
            patience: timeout,
        })
    }

    /// The `Sec-WebSocket-Key` this connection handshook with, for a transform
    /// that is keyed off the nonce.
    #[must_use]
    pub fn handshake_key(&self) -> &str {
        &self.handshake_key
    }

    /// Put a transform between the framing and the wire. Applies to every frame
    /// in both directions from here on, so it is installed before anything is
    /// sent.
    pub fn set_transform(&mut self, transform: Box<dyn PayloadTransform>) {
        self.transform = Some(transform);
    }

    pub fn send(&mut self, header: &RpcHeader, body: &[u8]) -> Result<()> {
        let encoded_header = header.encode_to_vec();
        let header_length = u16::try_from(encoded_header.len())
            .map_err(|_| wire_error("RPC header exceeds the 16-bit frame prefix"))?;
        let mut payload = Vec::with_capacity(2 + encoded_header.len() + body.len());
        payload.extend_from_slice(&header_length.to_be_bytes());
        payload.extend_from_slice(&encoded_header);
        payload.extend_from_slice(body);
        capture_bgs(Direction::Outgoing, header, body, &payload);
        self.send_raw(&payload)
    }

    /// One payload the caller framed itself. Remastered's classic channel
    /// frames its own, so it goes out through here rather than through the
    /// BGS header path — the transform still applies either way.
    pub fn send_raw(&mut self, payload: &[u8]) -> Result<()> {
        let wire = match &self.transform {
            Some(transform) => transform.encode(payload),
            None => payload.to_vec(),
        };
        self.socket.send(Message::binary(wire))?;
        Ok(())
    }

    /// Sends one text frame.
    ///
    /// Remastered's account layer speaks JSON over text frames rather than
    /// protobuf over binary ones, so it is the one caller of this. No transform
    /// applies: that is the classic channel's envelope, and this is not it.
    pub fn send_text(&mut self, payload: &str) -> Result<()> {
        self.socket.send(Message::text(payload))?;
        Ok(())
    }

    /// Reads one frame as text, whether it arrived as a text frame or a binary
    /// one.
    ///
    /// Aurora sends the same JSON both ways — the retail client reads either
    /// and does not distinguish — so refusing binary here refuses perfectly
    /// good messages.
    pub fn receive_text(&mut self) -> Result<String> {
        let deadline = Instant::now() + self.patience;
        loop {
            if Instant::now() >= deadline {
                return Err(transport_error(
                    "Battle.net held the connection open without answering",
                ));
            }
            match self.socket.read()? {
                Message::Text(message) => return Ok(message.to_string()),
                Message::Binary(message) => {
                    return String::from_utf8(message.into())
                        .map_err(|_| wire_error("websocket binary message is not valid UTF-8"));
                }
                Message::Close(_) => {
                    return Err(transport_error("websocket received a close frame"));
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    /// One payload, transformed but not parsed, for a caller that does its own
    /// framing. Ping and pong are answered by the socket and skipped here.
    pub fn receive_raw(&mut self) -> Result<Vec<u8>> {
        let deadline = Instant::now() + self.patience;
        loop {
            if Instant::now() >= deadline {
                return Err(transport_error(
                    "Battle.net held the connection open without answering",
                ));
            }
            match self.socket.read()? {
                Message::Binary(message) => {
                    return Ok(match &self.transform {
                        Some(transform) => transform.decode(&message),
                        None => message.into(),
                    });
                }
                Message::Close(_) => {
                    return Err(transport_error("websocket received a close frame"));
                }
                Message::Text(_) => {
                    return Err(wire_error("websocket received a text message"));
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    pub fn receive(&mut self) -> Result<RpcFrame> {
        // every answer is one we asked for, so none of them may take longer
        // than the socket's own patience — keepalives in between do not buy the
        // service more time
        let deadline = Instant::now() + self.patience;
        loop {
            if Instant::now() >= deadline {
                return Err(transport_error(
                    "Battle.net held the connection open without answering",
                ));
            }
            match self.socket.read()? {
                Message::Binary(message) => {
                    let message = match &self.transform {
                        Some(transform) => transform.decode(&message),
                        None => message.into(),
                    };
                    match decode_bgs_message(&message) {
                        Ok(frame) => {
                            capture_bgs(Direction::Incoming, &frame.header, &frame.body, &message);
                            return Ok(frame);
                        }
                        Err(error) => {
                            capture_invalid_bgs(Direction::Incoming, &message, &error.to_string());
                            return Err(error);
                        }
                    }
                }
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
        self.socket.close()
    }

    /// a handle another thread can use to end a read that is not going to
    /// return. the socket's read timeout does not survive the TLS layer — a
    /// blocked read stays blocked — so the only way to give up on a silent
    /// service is to take the connection out from under it.
    pub fn interrupt(&self) -> Result<SocketInterrupt> {
        Ok(SocketInterrupt(self.socket.tcp()?.try_clone()?))
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        let stream = self.socket.tcp()?;
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        Ok(())
    }
}

fn http_headers(headers: &tungstenite::http::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value
                    .to_str()
                    .map_or_else(|_| "<binary>".to_owned(), str::to_owned),
            )
        })
        .collect()
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

/// Ends a blocked read on a socket from another thread. Cutting the connection
/// makes the read fail at once instead of waiting on a service that has stopped
/// answering.
pub struct SocketInterrupt(TcpStream);

impl SocketInterrupt {
    pub fn cut(&self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

/// Performs the websocket upgrade by hand, then hands the upgraded stream to
/// tungstenite for framing.
///
/// Only the two things that matter are checked: the status is `101`, and the
/// `Sec-WebSocket-Accept` digest proves the server really did upgrade this
/// request rather than answering something else. Tungstenite additionally
/// insists the status line be well-formed HTTP/1.1, which Remastered's classic
/// edge does not satisfy — and retail's own client does not ask it to.
fn upgrade_leniently(
    host: &str,
    port: u16,
    stream: TcpStream,
    profile: SocketProfile<'_>,
) -> Result<(native_tls::TlsStream<TcpStream>, String)> {
    let connector = native_tls::TlsConnector::new()
        .map_err(|error| transport_error(format!("TLS setup failed: {error}")))?;
    let mut tls = connector
        .connect(host, stream)
        .map_err(|error| transport_error(format!("TLS handshake failed: {error}")))?;

    let mut nonce = [0u8; 16];
    rand::rng().fill_bytes(&mut nonce);
    let client_key = BASE64.encode(nonce);

    let path = profile.path;
    // the port is included even when it is the default: that is what the
    // retail client sends, and this handshake matches it rather than what is
    // merely legal
    let mut request = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {client_key}\r\n\
         Sec-WebSocket-Version: 13\r\n"
    );
    if let Some(subprotocol) = profile.subprotocol {
        request.push_str("Sec-WebSocket-Protocol: ");
        request.push_str(subprotocol);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    tls.write_all(request.as_bytes())?;
    tls.flush()?;

    // Read exactly the head and not one byte more.
    //
    // a byte over-read here is the first byte of the first frame, and there is
    // nowhere to put it back: the framing below starts from an empty buffer, so
    // losing one byte misaligns every frame after it and the first thing it
    // decodes is garbage. Hence one byte at a time, straight from the stream —
    // no `BufReader`, which would read ahead and drop what it had buffered.
    let mut head = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        if tls.read(&mut byte)? == 0 {
            return Err(transport_error("server closed during the upgrade"));
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > MAX_HANDSHAKE_BYTES {
            return Err(transport_error(format!(
                "upgrade headers exceed {MAX_HANDSHAKE_BYTES} bytes"
            )));
        }
    }

    let text = String::from_utf8_lossy(&head).into_owned();
    let mut lines = text.split("\r\n");
    let status = lines.next().unwrap_or_default();
    if status.split(' ').nth(1) != Some("101") {
        // the whole head: debugging an upgrade from a status line alone is
        // guesswork, and every guess costs a round trip through a live service
        let head = text.replace("\r\n", " | ");
        return Err(transport_error(format!(
            "classic upgrade rejected: {status} (response: {head})"
        )));
    }
    let header = |name: &str| -> Option<String> {
        lines.clone().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };

    let mut digest = Sha1::new();
    digest.update(client_key.as_bytes());
    digest.update(WEBSOCKET_ACCEPT_GUID);
    if header("sec-websocket-accept").as_deref() != Some(&BASE64.encode(digest.finalize())) {
        return Err(transport_error(
            "classic edge returned an invalid Sec-WebSocket-Accept",
        ));
    }
    if header("sec-websocket-protocol").as_deref() != profile.subprotocol {
        return Err(transport_error(
            "classic edge selected an unexpected websocket subprotocol",
        ));
    }

    Ok((tls, client_key))
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
