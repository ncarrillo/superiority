pub const HOST: &str = "us.actual.battle.net";
pub const PORT: u16 = 1119;
pub const BGS_SUBPROTOCOL: &str = "jsonrpc.bgs.v3.19.battle.net";

pub const TITLE_ID: u32 = 22_323;
pub const PLATFORM: &str = "Mac";
pub const LOCALE: &str = "enUS";
pub const APPLICATION_VERSION: u32 = 131_072;
pub const GAME_VERSION: &str = "2.0.4.23745";
pub const PLATFORM_FOURCC: u32 = 0x4D63_3634; // Mc64
pub const LOCALE_FOURCC: u32 = 0x656E_5553; // enUS
pub const SESSION_TYPE: u32 = 0x4444_4354; // DDCT
pub const CLIENT_CAPABILITIES: u32 = 0x0003_0100;
pub const CLASSIC_RPC_PATH: &str = "/W3/v2/rpc/client";

pub const CONNECTION_SERVICE: u32 = fnv1a("bnet.protocol.connection.ConnectionService");
pub const AUTHENTICATION_SERVICE: u32 =
    fnv1a("bnet.protocol.authentication.v2.client.AuthenticationService");
pub const AUTHENTICATION_LISTENER: u32 =
    fnv1a("bnet.protocol.authentication.v2.client.AuthenticationListener");
pub const GAME_UTILITIES_SERVICE: u32 =
    fnv1a("bnet.protocol.game_utilities.v2.client.GameUtilities");

pub mod authentication {
    pub const LOGON: u32 = 1;
    pub const VERIFY_AUTH_TOKEN: u32 = 2;
    pub const GENERATE_AUTH_TOKEN: u32 = 3;
}

pub mod authentication_listener {
    pub const ON_LOGON_COMPLETE: u32 = 1;
    pub const ON_LOGON_QUEUE_UPDATE: u32 = 2;
    pub const ON_LOGON_QUEUE_END: u32 = 3;
    pub const ON_EXTERNAL_CHALLENGE: u32 = 4;
}

#[must_use]
pub const fn fnv1a(value: &str) -> u32 {
    let bytes = value.as_bytes();
    let mut hash = 0x811c_9dc5_u32;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        index += 1;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovered_service_hashes_and_profile_are_stable() {
        assert_eq!(CONNECTION_SERVICE, 0x6544_6991);
        assert_eq!(AUTHENTICATION_SERVICE, 0xC02F_8216);
        assert_eq!(AUTHENTICATION_LISTENER, 0x9DA8_116B);
        assert_eq!(GAME_UTILITIES_SERVICE, 0x5DBB_51C2);
        assert_eq!(TITLE_ID.to_be_bytes()[2..], *b"W3");
        assert_eq!(PLATFORM_FOURCC.to_be_bytes(), *b"Mc64");
    }
}
