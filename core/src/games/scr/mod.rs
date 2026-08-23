//! `StarCraft: Remastered`.
//!
//! Remastered signs in through the same Battle.net account service every
//! product does (see [`crate::platform::bgs`]) and then opens a second
//! websocket — the *classic* channel — which carries protobuf-lite RPC inside
//! a check-value envelope:
//!
//! ```text
//! tls record
//!   websocket frame (rfc 6455)
//!     check-value envelope (connection-dependent)
//!       uint16 big-endian header length
//!       classic.protocol.Header (protobuf lite)
//!       request or response body (protobuf lite)
//! ```
//!
//! None of `StarCraft II`'s BSN machinery applies here: the framing shape is
//! shared, the encoding is not.
//!
//! Recovered in `sc1-research` from Remastered build `1.23.10_2e031d5be4`.

pub mod aurora;
pub mod auth;
pub mod catalog;
pub mod chat;
pub mod client;
pub mod envelope;
pub mod gateway;
pub mod handoff;
pub mod probe;
pub mod profile;
pub mod rpc;
pub mod session;
pub mod user;

/// Where the classic RPC channel answers. Aurora returns only the load
/// balancer's root; the client appends this route itself.
pub const CLASSIC_RPC_PATH: &str = "/S1/v2/rpc/client";

/// The FourCCs and versions the retail client presents, recovered from build
/// `1.23.10_2e031d5be4` and its `libClientSdk.dylib`. The edge validates them,
/// so none of these is a guess.
///
/// `PROGRAM`, `PLATFORM`, and the application version also reach Battle.net
/// through [`crate::product::Product::Remastered`], which is what the shared
/// account layer signs in with; these are the same values as the classic
/// channel needs them.
pub const SESSION_TYPE: u32 = 0x4444_4354; // "DDCT"
pub const GAME_VERSION: &str = "1.23.10.13515";
pub const CLIENT_CAPABILITIES: u32 = 0x0003_0100;

/// Stable 20-byte install identity, sent as the base64 text below.
pub const CLIENT_IDENTITY: &[u8] = b"DJqHt+VTbDlhkzsfTvFlKrRHZjw=";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::Product;

    #[test]
    fn the_session_type_is_a_fourcc() {
        assert_eq!(SESSION_TYPE.to_be_bytes(), *b"DDCT");
    }

    #[test]
    fn the_client_identity_is_twenty_bytes_of_base64() {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let identity = std::str::from_utf8(CLIENT_IDENTITY).expect("ascii");
        assert_eq!(identity.len(), 28);
        assert_eq!(BASE64.decode(identity).expect("base64").len(), 20);
    }

    #[test]
    fn the_classic_channel_and_the_account_layer_name_the_same_product() {
        // the path carries the program code, and signing in uses the same one;
        // if these ever disagree the session opens against the wrong edge
        assert!(CLASSIC_RPC_PATH.contains(Product::Remastered.code()));
        assert_eq!(Product::Remastered.fourcc(), 0x5331);
        let logon = Product::Remastered.logon().expect("traced");
        assert_eq!(logon.platform, "Mc64");
        assert_eq!(logon.application_version, 65559);
    }
}
