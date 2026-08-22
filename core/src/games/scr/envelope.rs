//! `Rpc.WebSocket.UseCheckValue`: a feedback XOR transform over every classic
//! payload. Its seed is folded from the 16-byte `Sec-WebSocket-Key` nonce, and
//! each four-byte block after the first derives its key from the previous
//! encoded word.
//!
//! Ported from `sc1-research`, whose vectors came from a paired capture of the
//! retail client.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::{Error, Result, platform::wire::websocket::PayloadTransform};

/// The constant the SDK mixes the product code with to reach its initial
/// connection state.
const CHECK_VALUE_MASK: u32 = 0x1083_1105;

/// Where the fold starts, which the current SDK derives from the product rather
/// than holding as a constant.
///
/// This was `5` — a value read off a capture — until it was traced to
/// `(product << 7) ^ mask`. The two agree for many nonces, because the first
/// four bytes are folded in with `|`: a nonce whose low bytes already carry the
/// bits where the bases differ produces the same seed either way. That is why
/// the wrong base worked intermittently, and why the capture-derived test below
/// passed while it was wrong.
///
/// A wrong seed yields a payload the edge cannot decode, and `AuthSession` then
/// goes unanswered.
#[must_use]
pub const fn check_value_base(product: u32) -> u32 {
    (product << 7) ^ CHECK_VALUE_MASK
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Encode,
    Decode,
}

#[must_use]
pub fn fold_nonce(nonce: &[u8; 16], product: u32) -> u32 {
    fold_from(nonce, check_value_base(product))
}

fn fold_from(nonce: &[u8; 16], initial: u32) -> u32 {
    nonce
        .iter()
        .enumerate()
        .fold(initial, |value, (index, &byte)| {
            // the sdk folds each byte as signed, so bytes at or above 0x80
            // sign-extend across the remaining lanes before the shift.
            let shifted = i32::from(byte.cast_signed()).cast_unsigned() << ((index & 3) * 8);
            if index < 4 {
                value | shifted
            } else {
                value ^ shifted
            }
        })
}

#[derive(Debug, Clone, Copy)]
pub struct CheckValueEnvelope {
    seed: u32,
}

impl CheckValueEnvelope {
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Derives the envelope from the nonce a connection handshook with — see
    /// [`crate::platform::wire::websocket::RpcSocket::handshake_key`] — and the
    /// product whose channel it is.
    pub fn from_websocket_key(websocket_key: &str, product: u32) -> Result<Self> {
        let nonce = BASE64
            .decode(websocket_key)
            .map_err(|_| envelope_error("Sec-WebSocket-Key is not valid base64"))?;
        let nonce: [u8; 16] = nonce.as_slice().try_into().map_err(|_| {
            envelope_error(format!(
                "Sec-WebSocket-Key decoded to {} bytes; expected 16",
                nonce.len()
            ))
        })?;
        Ok(Self::new(fold_nonce(&nonce, product)))
    }

    #[must_use]
    pub const fn seed(self) -> u32 {
        self.seed
    }

    fn transform(self, payload: &[u8], direction: Direction) -> Vec<u8> {
        let mut output = payload.to_vec();

        let mut key = self.seed;
        for lane in 0..output.len().min(4) {
            key = key.rotate_left(1);
            output[lane] ^= key.to_le_bytes()[lane];
        }

        let mut offset = 4;
        while offset < output.len() {
            // both directions read the encoded previous word: the encoder from
            // its output, the decoder from its still-encoded input.
            let window: [u8; 4] = match direction {
                Direction::Encode => &output[offset - 4..offset],
                Direction::Decode => &payload[offset - 4..offset],
            }
            .try_into()
            .expect("four bytes precede every later block");
            let previous = u32::from_le_bytes(window);

            // only the low five bits survive the mask, so narrowing is exact.
            let low_offset = u32::try_from(offset & 31).expect("masked to five bits");
            key = previous.rotate_right(!(low_offset ^ previous) & 31);
            for lane in 0..(output.len() - offset).min(4) {
                key = key.rotate_left(1);
                output[offset] ^= key.to_le_bytes()[lane];
                offset += 1;
            }
        }
        output
    }
}

impl PayloadTransform for CheckValueEnvelope {
    fn encode(&self, payload: &[u8]) -> Vec<u8> {
        self.transform(payload, Direction::Encode)
    }

    fn decode(&self, payload: &[u8]) -> Vec<u8> {
        self.transform(payload, Direction::Decode)
    }
}

fn envelope_error(message: impl Into<String>) -> Error {
    Error::Transport(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::Product;

    // nonce from the paired 2026-08-01 capture, with the seed the retail
    // client folded from it.
    const CAPTURED_KEY: &str = "yiq2IfascyqFu4qK8HfE8Q==";
    const CAPTURED_SEED: u32 = 0x513d_604c;

    // first four plaintext bytes of that connection's AuthSession frame: a
    // 68-byte header length followed by field 1 of service 0x17CDFF07.
    const CAPTURED_FIRST_BLOCK: [u8; 4] = [0x00, 0x44, 0x08, 0x87];

    fn captured_nonce() -> [u8; 16] {
        BASE64
            .decode(CAPTURED_KEY)
            .expect("base64")
            .as_slice()
            .try_into()
            .expect("16 bytes")
    }

    #[test]
    fn reproduces_the_captured_connection_seed() {
        let envelope =
            CheckValueEnvelope::from_websocket_key(CAPTURED_KEY, Product::Remastered.fourcc())
                .expect("valid key");
        assert_eq!(envelope.seed(), CAPTURED_SEED);
        assert_eq!(
            envelope.encode(&CAPTURED_FIRST_BLOCK),
            [0x98, 0xc5, 0xe3, 0x94],
            "must reproduce the block the retail client put on the wire"
        );
    }

    #[test]
    fn the_base_is_derived_from_the_product() {
        // traced to the SDK. Warcraft III reaches its own base by the same
        // formula, which is what identified it as a formula at all
        assert_eq!(check_value_base(Product::Remastered.fourcc()), 0x10AA_8985);
        assert_eq!(check_value_base(Product::Warcraft3.fourcc()), 0x10A8_8885);
        assert_ne!(
            check_value_base(Product::Remastered.fourcc()),
            check_value_base(Product::Warcraft3.fourcc())
        );
    }

    #[test]
    fn a_wrong_base_hides_behind_some_nonces_and_not_others() {
        // this is why the base was wrong for so long. the first four bytes fold
        // in with `|`, so a nonce already carrying the bits where two bases
        // differ produces the same seed from either — the captured nonce above
        // is one of those, which is why the test that reproduces it passed
        // while the base was `5`.
        let captured = captured_nonce();
        assert_eq!(
            fold_from(&captured, 5),
            fold_from(&captured, check_value_base(Product::Remastered.fourcc())),
            "this nonce cannot tell the two bases apart"
        );

        // one that can: nothing is OR-ed in, so the base survives intact
        let bare = [0u8; 16];
        assert_eq!(fold_from(&bare, 5), 5);
        assert_eq!(
            fold_from(&bare, check_value_base(Product::Remastered.fourcc())),
            0x10AA_8985
        );
    }

    #[test]
    fn folding_from_zero_does_not_match_the_sdk() {
        // the seed starts from the sdk's connection state, not from nothing;
        // the two differ by three bits and the edge rejects the wrong one
        let from_zero = fold_from(&captured_nonce(), 0);
        assert_eq!(from_zero, 0x513d_6049);
        assert_eq!(
            CheckValueEnvelope::new(from_zero).encode(&CAPTURED_FIRST_BLOCK),
            [0x92, 0xc5, 0xe3, 0x94]
        );
    }

    #[test]
    fn round_trips_payloads_across_block_boundaries() {
        let envelope =
            CheckValueEnvelope::from_websocket_key(CAPTURED_KEY, Product::Remastered.fourcc())
                .expect("valid key");
        for length in [0usize, 1, 3, 4, 5, 8, 17, 64, 255, 310] {
            let payload: Vec<u8> = (0..length)
                .map(|index| {
                    u8::try_from(index % 256)
                        .expect("masked to one byte")
                        .wrapping_mul(7)
                        .wrapping_add(3)
                })
                .collect();
            let encoded = envelope.encode(&payload);
            assert_eq!(encoded.len(), payload.len());
            assert_eq!(envelope.decode(&encoded), payload, "length {length}");
        }
    }

    #[test]
    fn rejects_keys_that_are_not_sixteen_byte_nonces() {
        assert!(
            CheckValueEnvelope::from_websocket_key("not base64!", Product::Remastered.fourcc())
                .is_err()
        );
        assert!(
            CheckValueEnvelope::from_websocket_key(
                &BASE64.encode([0u8; 8]),
                Product::Remastered.fourcc()
            )
            .is_err()
        );
    }
}
