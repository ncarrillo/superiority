// battle.net interleaves its own callbacks with the responses a client is
// waiting for, and expects every callback to be acknowledged with an empty
// response carrying the original token. every wait here drains and
// acknowledges callbacks while watching for the response it wants.

use std::time::{Duration, Instant};

use rand::RngCore as _;

use crate::{
    Error, Result,
    games::scr::{
        catalog,
        envelope::CheckValueEnvelope,
        handoff::ClassicHandoff,
        rpc::{Frame, Header},
    },
    platform::wire::websocket::{RpcSocket, SocketProfile},
    product::Product,
};

// recovered from the retail sdk profile; the edge validates it.
pub const DEFAULT_ROUTING_ID: u32 = 2_525_111_537;

#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub service_id: u32,
    pub method_id: u32,
    pub body: &'a [u8],
    // when present, echoed on the first callback acknowledgement, matching the
    // retail client.
    pub trace: Option<&'a [u8]>,
}

impl<'a> Request<'a> {
    #[must_use]
    pub fn new(service_id: u32, method_id: u32, body: &'a [u8]) -> Self {
        Self {
            service_id,
            method_id,
            body,
            trace: None,
        }
    }

    #[must_use]
    pub fn with_trace(mut self, trace: &'a [u8]) -> Self {
        self.trace = Some(trace);
        self
    }
}

#[must_use]
pub fn request_trace() -> Vec<u8> {
    let mut raw = [0u8; 16];
    rand::rng().fill_bytes(&mut raw);
    let group = |bytes: &[u8]| {
        bytes
            .iter()
            .fold(0u64, |value, byte| value << 8 | u64::from(*byte))
    };
    format!(
        "RT-{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
        group(&raw[..4]),
        group(&raw[4..6]),
        group(&raw[6..8]),
        group(&raw[8..10]),
        group(&raw[10..])
    )
    .into_bytes()
}

pub trait Callbacks {
    fn on_request(&mut self, frame: &Frame);
}

impl<F: FnMut(&Frame)> Callbacks for F {
    fn on_request(&mut self, frame: &Frame) {
        self(frame);
    }
}

pub struct Ignore;

impl Callbacks for Ignore {
    fn on_request(&mut self, _frame: &Frame) {}
}

pub struct ClassicClient {
    socket: RpcSocket,
    next_token: u32,
    routing_id: u32,
}

impl ClassicClient {
    /// Opens the channel the game service named. The handoff carries the
    /// server, the route, and the ticket — none of it is known before the
    /// account layer asks.
    pub fn connect(handoff: &ClassicHandoff, timeout: Duration) -> Result<Self> {
        let mut socket = RpcSocket::connect(
            &handoff.host,
            handoff.port,
            timeout,
            SocketProfile {
                path: &handoff.path,
                subprotocol: None,
                // this edge's upgrade response is not one tungstenite will
                // accept, though it is a valid 101
                lenient_upgrade: true,
            },
        )?;
        // the check value folds from the nonce this very handshake sent, so it
        // can only be built once the socket exists — and it has to be in place
        // before the first frame goes out
        let envelope = CheckValueEnvelope::from_websocket_key(
            socket.handshake_key(),
            Product::Remastered.fourcc(),
        )?;
        socket.set_transform(Box::new(envelope));
        Ok(Self {
            socket,
            next_token: 1,
            routing_id: DEFAULT_ROUTING_ID,
        })
    }

    #[must_use]
    pub fn routing_id(&self) -> u32 {
        self.routing_id
    }

    pub fn set_routing_id(&mut self, routing_id: u32) {
        self.routing_id = routing_id;
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.socket.set_timeout(timeout)
    }

    pub fn interrupt(&self) -> Result<crate::platform::wire::websocket::SocketInterrupt> {
        self.socket.interrupt()
    }

    pub fn send(&mut self, request: &Request<'_>) -> Result<u32> {
        let token = self.next_token;
        self.next_token = self.next_token.wrapping_add(1);
        let header = Header {
            service_id: request.service_id,
            method_id: request.method_id,
            token,
            routing_id: Some(self.routing_id),
            object_id: Some(0),
            is_response: Some(false),
            request_trace: request.trace.map(<[u8]>::to_vec),
            ..Header::default()
        };
        trace_rpc("->", &header, request.body.len());
        self.transmit(&header, request.body)?;
        Ok(token)
    }

    pub fn call(
        &mut self,
        request: &Request<'_>,
        timeout: Duration,
        callbacks: &mut impl Callbacks,
    ) -> Result<Frame> {
        let token = self.send(request)?;
        self.await_response(token, request, timeout, callbacks)
    }

    fn await_response(
        &mut self,
        token: u32,
        request: &Request<'_>,
        timeout: Duration,
        callbacks: &mut impl Callbacks,
    ) -> Result<Frame> {
        let deadline = Instant::now() + timeout;
        let mut ack_trace = request.trace;
        loop {
            let frame = self.receive(deadline)?;
            let header = &frame.header;
            if header.is_response() {
                if header.token == token
                    && header.service_id == request.service_id
                    && header.method_id == request.method_id
                {
                    return Ok(frame);
                }
                continue;
            }
            callbacks.on_request(&frame);
            self.acknowledge(&frame, ack_trace)?;
            ack_trace = None;
        }
    }

    pub fn pump(&mut self, timeout: Duration, callbacks: &mut impl Callbacks) -> Result<Frame> {
        let deadline = Instant::now() + timeout;
        loop {
            let frame = self.receive(deadline)?;
            if frame.header.is_response() {
                continue;
            }
            callbacks.on_request(&frame);
            self.acknowledge(&frame, None)?;
            return Ok(frame);
        }
    }

    fn acknowledge(&mut self, request: &Frame, trace: Option<&[u8]>) -> Result<()> {
        let header = Header {
            service_id: request.header.service_id,
            method_id: request.header.method_id,
            token: request.header.token,
            routing_id: request.header.routing_id.or(Some(self.routing_id)),
            object_id: request.header.object_id,
            is_response: Some(true),
            request_trace: trace.map(<[u8]>::to_vec),
            ..Header::default()
        };
        self.transmit(&header, &[])
    }

    fn transmit(&mut self, header: &Header, body: &[u8]) -> Result<()> {
        let rpc = Frame::encode(header, body)?;
        self.socket.send_raw(&rpc)
    }

    fn receive(&mut self, deadline: Instant) -> Result<Frame> {
        if Instant::now() >= deadline {
            return Err(classic_error(
                "timed out waiting for a Battle.net RPC response",
            ));
        }
        let frame = Frame::decode(&self.socket.receive_raw()?)?;
        trace_rpc("<-", &frame.header, frame.body.len());
        Ok(frame)
    }

    pub fn close(&mut self) -> Result<()> {
        self.socket.close()
    }
}

fn trace_rpc(direction: &str, header: &Header, body_len: usize) {
    if std::env::var_os("SUPERIORITY_TRACE").is_none() {
        return;
    }
    let name = catalog::rpc_name(header.service_id, header.method_id)
        .unwrap_or_else(|| format!("{:08x}.{:08x}", header.service_id, header.method_id));
    eprintln!(
        "superiority: [S1] {direction} {name} token={} response={} body={body_len}",
        header.token,
        header.is_response(),
    );
}

fn classic_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_traces_use_the_sdk_shape() {
        let text = String::from_utf8(request_trace()).expect("ascii");
        assert!(text.starts_with("RT-"));
        let groups: Vec<usize> = text[3..].split('-').map(str::len).collect();
        assert_eq!(groups, [8, 4, 4, 4, 12]);
        assert!(
            text[3..]
                .chars()
                .all(|character| character == '-' || character.is_ascii_hexdigit())
        );
        assert!(!text.chars().any(|character| character.is_ascii_lowercase()));
        assert_ne!(request_trace(), request_trace());
    }
}
