//! asking Battle.net where Remastered's classic server is, and being handed a
//! one-time ticket to reach it.
//!
//! This is the same `GameUtilities.ProcessClientRequest` call `StarCraft II`
//! makes — the attribute names are shared — but the payload inside is
//! `classic.protocol.v1.aurora.ConnectToServerRequest`, and what comes back is
//! a websocket URL rather than a native host and port.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use prost::Message as _;
use url::Url;

use crate::{
    Error, Result,
    games::scr::{CLASSIC_RPC_PATH, GAME_VERSION},
    platform::{
        bgs::{
            SecretBytes,
            generated::bgs::protocol::{
                Attribute, Variant,
                game_utilities::v1::{ClientRequest, ClientResponse},
            },
        },
        wire::raw::{self as protobuf, Message},
    },
    product::Product,
};

const REQUEST_TYPE: &str = "classic.protocol.v1.aurora.ConnectToServerRequest";
const RESPONSE_TYPE: &str = "classic.protocol.v1.aurora.ConnectToServerResponse";

/// where the classic channel is, and the ticket that opens it.
#[derive(Clone, Debug)]
pub struct ClassicHandoff {
    pub url: String,
    pub host: String,
    pub port: u16,
    /// the route to ask for, which is always [`CLASSIC_RPC_PATH`].
    ///
    /// Whatever path Aurora's URL carries is **metadata, not a route**. It has
    /// named `/v1/rpc/client` and, in the capture this was recovered from,
    /// nothing at all; the retail client ignores both and asks for its own
    /// product route. Appending to the URL's path produced
    /// `/v1/rpc/client/S1/v2/rpc/client` and using it verbatim produced
    /// `/v1/rpc/client` — the edge answered `HTTP/1.0 404 OK` to each.
    pub path: String,
    pub ticket: SecretBytes,
    /// every field the `ConnectToServer` payload carried, for the trace. Only
    /// two of them are read; this is what the rest were.
    pub shape: String,
}

impl ClassicHandoff {
    pub fn from_url(url: &str, ticket: SecretBytes) -> Result<Self> {
        let parsed =
            Url::parse(url).map_err(|_| handoff_error(format!("invalid classic URL {url:?}")))?;
        if parsed.scheme() != "wss" {
            return Err(handoff_error(format!("classic URL {url:?} is not wss")));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| handoff_error(format!("classic URL {url:?} has no host")))?;
        if parsed.path().is_empty() {
            return Err(handoff_error(format!("classic URL {url:?} has no path")));
        }
        // the service names the whole route now — `/v1/rpc/client` — where the
        // capture this was recovered from returned only the balancer root and
        // left the client to append its own. Appending to a URL that already
        // carries a path asks for `/v1/rpc/client/S1/v2/rpc/client`, which is a
        // 404. So the route is only supplied when the URL does not have one.
        let mut path = CLASSIC_RPC_PATH.to_owned();
        if let Some(query) = parsed.query() {
            path.push('?');
            path.push_str(query);
        }
        Ok(Self {
            url: url.to_owned(),
            host: host.to_owned(),
            port: parsed.port().unwrap_or(443),
            path,
            ticket,
            shape: String::new(),
        })
    }
}

/// the request that asks for a classic server.
#[must_use]
pub fn connect_to_server_request() -> ClientRequest {
    let payload = Message::new()
        .varint(2, u64::from(Product::Remastered.fourcc()))
        .bytes(3, GAME_VERSION.as_bytes())
        .varint(4, u64::from(platform_fourcc()))
        .into_vec();
    ClientRequest {
        attribute: vec![
            attribute_string("client_request", REQUEST_TYPE),
            attribute_string("protobuf", &BASE64.encode(payload)),
            attribute_string("server_instance", "Release"),
        ],
        ..ClientRequest::default()
    }
}

pub fn parse_connect_to_server(body: &[u8]) -> Result<ClassicHandoff> {
    let response = ClientResponse::decode(body)?;
    // the edge picks the variant per attribute — StarCraft II's `session_key`
    // arrives as bytes and its `address` as a string on this same call — so a
    // payload is read as either raw bytes or the base64 text of the same bytes
    let bytes = |wanted: &str| -> Option<Vec<u8>> {
        response.attribute.iter().find_map(|attribute| {
            (attribute.name == wanted).then_some(())?;
            if let Some(blob) = attribute.value.blob_value.as_ref() {
                return Some(blob.clone());
            }
            let text = attribute.value.string_value.as_ref()?;
            BASE64.decode(text).ok()
        })
    };
    // text is read the other way round: a type name is a string, and only
    // falls back to reading a blob as UTF-8 — never through base64, which a
    // plain name is not
    let text = |wanted: &str| -> Option<String> {
        response.attribute.iter().find_map(|attribute| {
            (attribute.name == wanted).then_some(())?;
            if let Some(value) = attribute.value.string_value.as_ref() {
                return Some(value.clone());
            }
            String::from_utf8(attribute.value.blob_value.clone()?).ok()
        })
    };
    // what actually came back, for when it is not what was expected
    let named = || -> String {
        response
            .attribute
            .iter()
            .map(|attribute| attribute.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let response_type = text("client_response");
    if response_type.as_deref() != Some(RESPONSE_TYPE) {
        return Err(handoff_error(format!(
            "unexpected classic response type {response_type:?} (attributes: {})",
            named()
        )));
    }
    let raw = bytes("protobuf").ok_or_else(|| {
        handoff_error(format!(
            "ConnectToServer response has no protobuf payload (attributes: {})",
            named()
        ))
    })?;
    let url = protobuf::first_bytes(&raw, 1)
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .ok_or_else(|| handoff_error("ConnectToServer response has no endpoint URL"))?;
    let ticket = protobuf::first_bytes(&raw, 2)
        .ok_or_else(|| handoff_error("ConnectToServer response has no ticket"))?;
    ClassicHandoff::from_url(&url, SecretBytes::new(ticket.to_vec())?)
}

/// `Mc64`, which Remastered presents on both the account layer and this one.
pub(super) fn platform_fourcc() -> u32 {
    let platform = Product::Remastered
        .logon()
        .expect("Remastered has a traced logon profile")
        .platform
        .as_bytes();
    platform
        .iter()
        .fold(0u32, |value, byte| (value << 8) | u32::from(*byte))
}

fn attribute_string(name: &str, value: &str) -> Attribute {
    Attribute {
        name: name.into(),
        value: Variant {
            string_value: Some(value.into()),
            ..Variant::default()
        },
    }
}

fn handoff_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_names_a_server_and_the_client_names_the_route() {
        // whatever path Aurora carries is metadata. it has said
        // `/v1/rpc/client`, and said nothing at all; the route is the retail
        // one either way
        let ticket = || SecretBytes::new(b"ticket".to_vec()).expect("ticket");
        for url in [
            "wss://ord1a-s1-rclient.classic.blizzard.com/v1/rpc/client",
            "wss://us1-s1-rclient-ext-lb.classic.blizzard.com/",
            "wss://classic.example.invalid/some/other/thing",
        ] {
            let handoff = ClassicHandoff::from_url(url, ticket()).expect("valid url");
            assert_eq!(handoff.path, CLASSIC_RPC_PATH, "for {url}");
        }

        let handoff = ClassicHandoff::from_url(
            "wss://ord1a-s1-rclient.classic.blizzard.com/v1/rpc/client",
            ticket(),
        )
        .expect("valid url");
        assert_eq!(handoff.host, "ord1a-s1-rclient.classic.blizzard.com");
        assert_eq!(handoff.port, 443);

        // a query is the server's, so it survives onto the route
        let with_query = ClassicHandoff::from_url(
            "wss://classic.example.invalid:1119/v1/rpc/client?token=abc",
            ticket(),
        )
        .expect("valid url");
        assert_eq!(with_query.path, format!("{CLASSIC_RPC_PATH}?token=abc"));
        assert_eq!(with_query.port, 1119);
    }

    #[test]
    fn rejects_urls_the_classic_channel_cannot_use() {
        let ticket = || SecretBytes::new(b"t".to_vec()).expect("ticket");
        assert!(ClassicHandoff::from_url("ws://insecure.invalid/", ticket()).is_err());
        assert!(ClassicHandoff::from_url("not a url", ticket()).is_err());
    }

    #[test]
    fn the_request_names_remastered_and_its_build() {
        let request = connect_to_server_request();
        let names: Vec<&str> = request
            .attribute
            .iter()
            .map(|attribute| attribute.name.as_str())
            .collect();
        assert_eq!(names, ["client_request", "protobuf", "server_instance"]);

        let payload = BASE64
            .decode(request.attribute[1].value.string_value.clone().unwrap())
            .expect("base64");
        assert_eq!(protobuf::first_varint(&payload, 2), Some(0x5331));
        assert_eq!(
            protobuf::first_bytes(&payload, 3),
            Some(GAME_VERSION.as_bytes())
        );
        assert_eq!(protobuf::first_varint(&payload, 4), Some(0x4D63_3634));
    }

    #[test]
    fn a_payload_is_read_as_bytes_or_as_the_base64_of_the_same_bytes() {
        // the edge chooses the variant per attribute; reading only the string
        // one made a perfectly good response look like it had no payload
        let inner = Message::new()
            .bytes(1, b"wss://classic.example.invalid/")
            .bytes(2, b"ticket")
            .into_vec();

        let as_blob = ClientResponse {
            attribute: vec![
                attribute_string("client_response", RESPONSE_TYPE),
                Attribute {
                    name: "protobuf".into(),
                    value: Variant {
                        blob_value: Some(inner.clone()),
                        ..Variant::default()
                    },
                },
            ],
        };
        let handoff = parse_connect_to_server(&as_blob.encode_to_vec()).expect("blob payload");
        assert_eq!(handoff.host, "classic.example.invalid");
        assert_eq!(handoff.ticket.expose(), b"ticket");

        let as_text = ClientResponse {
            attribute: vec![
                attribute_string("client_response", RESPONSE_TYPE),
                attribute_string("protobuf", &BASE64.encode(&inner)),
            ],
        };
        let same = parse_connect_to_server(&as_text.encode_to_vec()).expect("base64 payload");
        assert_eq!(same.host, handoff.host);
        assert_eq!(same.ticket.expose(), handoff.ticket.expose());
    }

    #[test]
    fn a_response_that_carries_no_payload_says_what_it_did_carry() {
        // debugging this blind is the difference between one round trip and ten
        let response = ClientResponse {
            attribute: vec![
                attribute_string("client_response", RESPONSE_TYPE),
                attribute_string("error_message", "something went wrong"),
            ],
        };
        let error = parse_connect_to_server(&response.encode_to_vec())
            .expect_err("no payload")
            .to_string();
        assert!(error.contains("error_message"), "{error}");
    }

    #[test]
    fn a_response_of_the_wrong_type_is_refused() {
        let wrong = ClientResponse {
            attribute: vec![attribute_string("client_response", "something.else")],
        };
        assert!(parse_connect_to_server(&wrong.encode_to_vec()).is_err());
    }
}
