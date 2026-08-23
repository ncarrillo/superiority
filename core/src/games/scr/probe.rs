//! a one-shot diagnostic for the classic upgrade, behind `SUPERIORITY_SCR_PROBE`.
//!
//! The edge answers `HTTP/1.0 404 OK` — an empty header block and a reason
//! phrase that does not match its own status — for every request tried so far,
//! including the reference client's. That is not enough to tell which part of
//! the request it dislikes, and finding out one variable per run costs a live
//! sign-in each time. This asks several ways at once and reports what each got.
//!
//! It changes nothing: the connection proceeds, and fails, exactly as it would
//! have. Off unless the variable is set.

use std::{
    fmt::Write as _,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore as _;

use crate::games::scr::{CLASSIC_RPC_PATH, handoff::ClassicHandoff};

/// one way of asking, and what it is testing.
struct Variant {
    what: &'static str,
    path: String,
    authority: String,
    subprotocol: Option<&'static str>,
    origin: bool,
}

#[must_use]
pub fn run_is_wanted() -> bool {
    std::env::var_os("SUPERIORITY_SCR_PROBE").is_some()
}

/// tries each variant and returns one line per attempt.
#[must_use]
pub fn run(handoff: &ClassicHandoff, timeout: Duration) -> String {
    let host = handoff.host.as_str();
    let port = handoff.port;
    let with_port = format!("{host}:{port}");
    let named = handoff.path.clone();
    // the route the capture used, on the host the service now names
    let legacy = CLASSIC_RPC_PATH.to_owned();

    let variants = vec![
        Variant {
            what: "as we ask now",
            path: named.clone(),
            authority: with_port.clone(),
            subprotocol: None,
            origin: false,
        },
        Variant {
            what: "Host without the default port",
            path: named.clone(),
            authority: host.to_owned(),
            subprotocol: None,
            origin: false,
        },
        Variant {
            what: "the capture's route",
            path: legacy.clone(),
            authority: with_port.clone(),
            subprotocol: None,
            origin: false,
        },
        Variant {
            what: "the capture's route, Host without port",
            path: legacy,
            authority: host.to_owned(),
            subprotocol: None,
            origin: false,
        },
        Variant {
            what: "with the Front channel's subprotocol",
            path: named.clone(),
            authority: with_port.clone(),
            subprotocol: Some("v1.rpc.battle.net"),
            origin: false,
        },
        Variant {
            what: "with an Origin header",
            path: named.clone(),
            authority: with_port.clone(),
            subprotocol: None,
            origin: true,
        },
        Variant {
            what: "the site root",
            path: "/".to_owned(),
            authority: with_port,
            subprotocol: None,
            origin: false,
        },
    ];

    let mut report = String::new();
    for variant in variants {
        let outcome = attempt(host, port, &variant, timeout)
            .unwrap_or_else(|error| format!("<could not ask: {error}>"));
        let _ = write!(
            report,
            "\n[S1]   probe {:<40} {} -> {outcome}",
            variant.what, variant.path
        );
    }
    report
}

fn attempt(host: &str, port: u16, variant: &Variant, timeout: Duration) -> Result<String, String> {
    let stream = TcpStream::connect((host, port)).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    let connector = native_tls::TlsConnector::new().map_err(|error| error.to_string())?;
    let mut tls = connector
        .connect(host, stream)
        .map_err(|error| error.to_string())?;

    let mut nonce = [0u8; 16];
    rand::rng().fill_bytes(&mut nonce);
    let key = BASE64.encode(nonce);

    let mut request = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n",
        variant.path, variant.authority
    );
    if let Some(subprotocol) = variant.subprotocol {
        let _ = write!(request, "Sec-WebSocket-Protocol: {subprotocol}\r\n");
    }
    if variant.origin {
        let _ = write!(request, "Origin: https://{host}\r\n");
    }
    request.push_str("\r\n");
    tls.write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    tls.flush().map_err(|error| error.to_string())?;

    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match tls.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => head.push(byte[0]),
            Err(error) => return Err(error.to_string()),
        }
        if head.ends_with(b"\r\n\r\n")
            || head.len() > crate::platform::wire::websocket::MAX_HANDSHAKE_BYTES
        {
            break;
        }
    }
    let text = String::from_utf8_lossy(&head).replace("\r\n", " | ");
    Ok(text.trim_end_matches(" | ").to_owned())
}
