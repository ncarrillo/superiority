use std::{fmt, net::IpAddr, str::FromStr};

use hmac::{Hmac, Mac};
use rand::TryRngCore;
use rc4::{KeyInit, Rc4, StreamCipher};
use rsa::{BigUint, Pkcs1v15Sign, RsaPublicKey, traits::PublicKeyParts};
use sha2::{Digest, Sha256, Sha512};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{Error, Result};

type HmacSha256 = Hmac<Sha256>;

const INBOUND_RC4_LABEL: [u8; 16] = [
    0x68, 0xe0, 0xc7, 0x2e, 0xdd, 0xd6, 0xd2, 0xf3, 0x1e, 0x5a, 0xb1, 0x55, 0xb1, 0x8b, 0x63, 0x1e,
];
const OUTBOUND_RC4_LABEL: [u8; 16] = [
    0xde, 0xa9, 0x65, 0xae, 0x54, 0x3a, 0x1e, 0x93, 0x9e, 0x69, 0x0c, 0xaa, 0x68, 0xde, 0x78, 0x39,
];

const THUMBPRINT_MODULUS_LE: [u8; 512] = hex_array_512(
    "c95435d383bf6622543f9e30f301c545942854e28cef54df37542c079534d25d\
     84ff3fe6d0fa5ed2cfe62fd44cfb4cd6911c0d90328a2adc92c3c5e7774e7b50\
     ac1e38da03c77732899c17cf7bd3872b7ad82cd198fee536545a593da9635a29\
     3dc322a9a55fdb9269b0ff7b4b8efcca69317d2dfceae629588f4653295a3ee6\
     8ca8a013b2151d60be840a76f4c9b54e4827d1c39d313012a2ecf1e4826e9c78\
     ba46a24856107821b8f7d150676bf78d0ec54ea49840075a4a92b152272c21d8\
     6b2e67fb36567ffdbb9af551bfa743e72409b8a009f33c5bf100c135d34f9aef\
     33f4f1de67ab28d7a750f4b103f788482668d626d6fc3d1b30f1a2800c17feeb\
     3016c035fe406d6378bb66b61d008bc6213bc855ff0ff32330b61ed4173604b7\
     f62ebc8c09c4c7c0d259fde67c1044b156d5f63e42c33b2239d733258882f07e\
     f1ea79e43eee1365dfd51bef79a68a815c31146febc348459870aa1cfdb7f2b6\
     fcd173cac05967b5950337af7e213d34eb1217b59c8dd6aeaa88ceb800bc277f\
     97a734281e68ca4836bf2d6e1b8f7bec3763df1c3ec3b30f39731167cac86f75\
     845294717c6c663e48dca687bfbcc39786158e44cdd3b144df84933d502041f3\
     abe588f96d342000d650be5afa939985b1272784d1c7b1fea3fa49f1fc028ebc\
     dacc4ded8749fd40cd50458ee1c30c4d2c1b3c9bcfb4b3696a2bf40dde83afde",
);

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SessionProof {
    output: [u8; 49],
    client_nonce: [u8; 16],
    server_nonce: [u8; 16],
    transport_key: [u8; 64],
    expected_server_proof: [u8; 32],
}

impl SessionProof {
    #[must_use]
    pub const fn output(&self) -> &[u8; 49] {
        &self.output
    }

    #[must_use]
    pub const fn client_nonce(&self) -> &[u8; 16] {
        &self.client_nonce
    }

    #[must_use]
    pub const fn server_nonce(&self) -> &[u8; 16] {
        &self.server_nonce
    }

    #[must_use]
    pub const fn transport_key(&self) -> &[u8; 64] {
        &self.transport_key
    }

    #[must_use]
    pub const fn expected_server_proof(&self) -> &[u8; 32] {
        &self.expected_server_proof
    }
}

impl fmt::Debug for SessionProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionProof(<redacted>)")
    }
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct TransportHandshake {
    client_context: [u8; 16],
    server_context: [u8; 16],
    response: [u8; 48],
    protected_secret: [u8; 64],
}

impl TransportHandshake {
    #[must_use]
    pub const fn client_context(&self) -> &[u8; 16] {
        &self.client_context
    }

    #[must_use]
    pub const fn server_context(&self) -> &[u8; 16] {
        &self.server_context
    }

    #[must_use]
    pub const fn response(&self) -> &[u8; 48] {
        &self.response
    }

    #[must_use]
    pub const fn protected_secret(&self) -> &[u8; 64] {
        &self.protected_secret
    }
}

impl fmt::Debug for TransportHandshake {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TransportHandshake(<redacted>)")
    }
}

pub struct Rc4State(Rc4);

impl Rc4State {
    pub fn new(key: &[u8]) -> Result<Self> {
        Rc4::new_from_slice(key)
            .map(Self)
            .map_err(|_| native_error("RC4 key length is outside 1..=256 bytes"))
    }

    pub fn apply(&mut self, data: &[u8]) -> Vec<u8> {
        let mut output = data.to_vec();
        self.0.apply_keystream(&mut output);
        output
    }

    pub fn apply_in_place(&mut self, data: &mut [u8]) {
        self.0.apply_keystream(data);
    }
}

impl fmt::Debug for Rc4State {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Rc4State(<redacted>)")
    }
}

pub fn transport_kdf64(
    key: &[u8],
    domain: u8,
    first_context: &[u8],
    second_context: &[u8],
) -> Result<[u8; 64]> {
    require_length(key, 64, "transport KDF key")?;
    require_length(first_context, 16, "transport KDF first context")?;
    require_length(second_context, 16, "transport KDF second context")?;
    let first = hmac_sha256(key, &[&[domain], first_context, second_context])?;
    let second = hmac_sha256(key, &[&[domain], second_context, first_context, &[domain]])?;
    let mut output = [0_u8; 64];
    output[..32].copy_from_slice(&first);
    output[32..].copy_from_slice(&second);
    Ok(output)
}

pub fn derive_session_auth_key(
    session_seed: &[u8],
    client_nonce: &[u8],
    server_nonce: &[u8],
) -> Result<[u8; 64]> {
    require_length(session_seed, 64, "GameUtilities session seed")?;
    require_length(client_nonce, 16, "native client challenge")?;
    require_length(server_nonce, 16, "native server challenge")?;
    let first = hmac_sha256(session_seed, &[b"\x00", client_nonce, server_nonce])?;
    let second = hmac_sha256(session_seed, &[b"\x01", server_nonce, client_nonce])?;
    let mut output = [0_u8; 64];
    output[..32].copy_from_slice(&first);
    output[32..].copy_from_slice(&second);
    Ok(output)
}

pub fn derive_transport_rc4_keys(protected_secret: &[u8]) -> Result<([u8; 32], [u8; 32])> {
    require_length(protected_secret, 64, "protected transport secret")?;
    Ok((
        hmac_sha256(protected_secret, &[&INBOUND_RC4_LABEL])?,
        hmac_sha256(protected_secret, &[&OUTBOUND_RC4_LABEL])?,
    ))
}

pub fn build_transport_handshake(
    transport_key: &[u8],
    server_context: &[u8],
) -> Result<TransportHandshake> {
    let mut client_context = [0_u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut client_context)
        .map_err(|error| native_error(format!("secure nonce generation failed: {error}")))?;
    build_transport_handshake_with_nonce(transport_key, server_context, client_context)
}

pub fn build_session_proof(session_seed: &[u8], server_nonce: &[u8]) -> Result<SessionProof> {
    let mut client_nonce = [0_u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut client_nonce)
        .map_err(|error| native_error(format!("secure nonce generation failed: {error}")))?;
    build_session_proof_with_nonce(session_seed, server_nonce, client_nonce)
}

pub fn thumbprint_context_for_peer(address: &str) -> Result<[u8; 16]> {
    let host = address.split_once('%').map_or(address, |(host, _)| host);
    match IpAddr::from_str(host)
        .map_err(|_| native_error("native peer address is not an IPv4 or IPv6 literal"))?
    {
        IpAddr::V4(address) => Ok(address.to_ipv6_mapped().octets()),
        IpAddr::V6(address) => Ok(address.octets()),
    }
}

#[must_use]
pub fn verify_thumbprint_proof(context: &[u8], signature: &[u8]) -> bool {
    if context.len() != 16 || signature.len() != 512 {
        return false;
    }
    let Ok(key) = RsaPublicKey::new(
        BigUint::from_bytes_le(&THUMBPRINT_MODULUS_LE),
        BigUint::from(65_537_u32),
    ) else {
        return false;
    };
    if key.size() != signature.len() {
        return false;
    }
    let digest = Sha512::digest([context, b"Thumbprint.IPv6"].concat());
    let signature = signature.iter().rev().copied().collect::<Vec<_>>();
    key.verify(Pkcs1v15Sign::new::<Sha512>(), &digest, &signature)
        .is_ok()
}

fn build_transport_handshake_with_nonce(
    transport_key: &[u8],
    server_context: &[u8],
    client_context: [u8; 16],
) -> Result<TransportHandshake> {
    require_length(transport_key, 64, "transport handshake key")?;
    require_length(server_context, 16, "transport server context")?;
    let server_context: [u8; 16] = server_context
        .try_into()
        .expect("length was validated above");
    let negotiation = transport_kdf64(transport_key, 0, &client_context, &server_context)?;
    let mut next_context = [0_u8; 16];
    next_context.copy_from_slice(&negotiation[32..48]);
    let protected_secret = transport_kdf64(transport_key, 2, &next_context, &server_context)?;
    let mut response = [0_u8; 48];
    response[..16].copy_from_slice(&next_context);
    response[16..].copy_from_slice(&negotiation[..32]);
    Ok(TransportHandshake {
        client_context,
        server_context,
        response,
        protected_secret,
    })
}

fn build_session_proof_with_nonce(
    session_seed: &[u8],
    server_nonce: &[u8],
    client_nonce: [u8; 16],
) -> Result<SessionProof> {
    require_length(session_seed, 64, "GameUtilities session seed")?;
    require_length(server_nonce, 16, "native server challenge")?;
    let server_nonce: [u8; 16] = server_nonce.try_into().expect("length was validated above");
    let transport_key = derive_session_auth_key(session_seed, &client_nonce, &server_nonce)?;
    let client_proof = hmac_sha256(&transport_key, &[b"\x00", &client_nonce, &server_nonce])?;
    let expected_server_proof =
        hmac_sha256(&transport_key, &[b"\x01", &server_nonce, &client_nonce])?;
    let mut output = [0_u8; 49];
    output[0] = 1;
    output[1..17].copy_from_slice(&client_nonce);
    output[17..].copy_from_slice(&client_proof);
    Ok(SessionProof {
        output,
        client_nonce,
        server_nonce,
        transport_key,
        expected_server_proof,
    })
}

fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> Result<[u8; 32]> {
    let mut digest =
        HmacSha256::new_from_slice(key).map_err(|_| native_error("HMAC key was rejected"))?;
    for part in parts {
        digest.update(part);
    }
    Ok(digest.finalize().into_bytes().into())
}

fn require_length(value: &[u8], expected: usize, label: &str) -> Result<()> {
    if value.len() == expected {
        Ok(())
    } else {
        Err(native_error(format!(
            "{label} is {} bytes; expected {expected}",
            value.len()
        )))
    }
}

fn native_error(message: impl Into<String>) -> Error {
    Error::Native(message.into())
}

const fn hex_array_512(value: &str) -> [u8; 512] {
    let bytes = value.as_bytes();
    let mut output = [0_u8; 512];
    let mut source = 0;
    let mut destination = 0;
    while destination < 512 {
        while source < bytes.len() && (bytes[source] == b' ' || bytes[source] == b'\n') {
            source += 1;
        }
        output[destination] = (hex_digit(bytes[source]) << 4) | hex_digit(bytes[source + 1]);
        source += 2;
        destination += 1;
    }
    output
}

const fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        b'A'..=b'F' => value - b'A' + 10,
        _ => panic!("invalid hexadecimal digit"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_proof_matches_recovered_hmac_order() {
        let seed = (0_u8..64).collect::<Vec<_>>();
        let server_nonce = (16_u8..32).collect::<Vec<_>>();
        let client_nonce: [u8; 16] = (32_u8..48).collect::<Vec<_>>().try_into().unwrap();
        let proof = build_session_proof_with_nonce(&seed, &server_nonce, client_nonce).unwrap();

        assert_eq!(
            hex::encode(proof.transport_key()),
            "680a8f22143bd8b198e251ebbbbd2404b3abf8e98726998e14bd1b20e1fb6927abd8d7432a79162e70ee6b88ac88f26cf705ffcd9d76ca02b131d70a95247c33"
        );
        assert_eq!(
            hex::encode(&proof.output()[17..]),
            "d4926776eac9c008aa5702a99bf7ea01928db94b76afdb1f23a6156d47bbdc1f"
        );
        assert_eq!(
            hex::encode(proof.expected_server_proof()),
            "ea649db50fff97f9691931772b87a3af161a6c5712ddce2cf6734249f92364cf"
        );
        assert!(!format!("{proof:?}").contains("680a8f22"));
    }

    #[test]
    fn transport_schedule_matches_recovered_vectors() {
        let key = (0_u8..64).collect::<Vec<_>>();
        let first = (0_u8..16).collect::<Vec<_>>();
        let second = (16_u8..32).collect::<Vec<_>>();
        assert_eq!(
            hex::encode(transport_kdf64(&key, 2, &first, &second).unwrap()),
            "a1fe9de83671a75280e4ab71fcc689cc83d96d2690c073738a8e9282742267bafb37ee1073eae6501d81b7e769d43240ad345a3c85703122576259b90829ea45"
        );
        let (inbound, outbound) = derive_transport_rc4_keys(&key).unwrap();
        assert_eq!(
            hex::encode(inbound),
            "5d179cef1b18de5f87e60f436a679e89ddea7ae7e92ec1a06c52df733ce124e2"
        );
        assert_eq!(
            hex::encode(outbound),
            "538690548cd9f6fa25c5204ef466f075757478decbfcc892be1e2c974c8637e3"
        );
    }

    #[test]
    fn rc4_uses_the_standard_stateful_stream() {
        let expected = hex::decode("bbf316e8d940af0ad3").unwrap();
        let plaintext = b"Plaintext";
        assert_eq!(Rc4State::new(b"Key").unwrap().apply(plaintext), expected);

        let mut fragmented = Rc4State::new(b"Key").unwrap();
        let mut actual = fragmented.apply(&plaintext[..2]);
        actual.extend(fragmented.apply(&plaintext[2..]));
        assert_eq!(actual, expected);
        assert!(!format!("{fragmented:?}").contains("Key"));
    }

    #[test]
    fn thumbprint_context_normalizes_ip_addresses() {
        assert_eq!(
            thumbprint_context_for_peer("192.0.2.10").unwrap(),
            hex::decode("00000000000000000000ffffc000020a")
                .unwrap()
                .as_slice()
        );
        assert_eq!(
            thumbprint_context_for_peer("2001:db8::1%en0").unwrap(),
            hex::decode("20010db8000000000000000000000001")
                .unwrap()
                .as_slice()
        );
        assert!(thumbprint_context_for_peer("native.example").is_err());
        assert!(!verify_thumbprint_proof(&[0; 16], &[0; 512]));
    }
}
