//! End-to-end encrypted packets between the phone and its linked computer, when they go through the
//! relay (#430). The relay only sees a sequence of bytes.
//!
//! ChaCha20-Poly1305 (`aws-lc-rs`), a 256-bit key drawn by the PC and given to the phone on the
//! local network only, a random 96-bit nonce per packet. The associated data binds each packet to
//! the tunnel and its direction, and each response to the nonce of its request: the relay can
//! neither read, modify, nor replay a response on another request. Against the replay of a request,
//! it carries its date and the PC remembers recent nonces ([`Replay`]).
//!
//! Packet: base64 (nonce ‖ ciphertext ‖ tag).

use std::collections::{HashMap, VecDeque};

use aws_lc_rs::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

/// An older request (or one further ahead, with clocks out of step) is refused.
pub const MAX_SKEW_MS: i64 = 5 * 60 * 1000;
/// Request nonces remembered at most (well beyond 120 calls per minute over 10 min).
const MAX_SEEN: usize = 4_096;

pub type NonceBytes = [u8; NONCE_LEN];

/// Shared key of the tunnel.
#[derive(Clone)]
pub struct TunnelKey(std::sync::Arc<LessSafeKey>);

impl std::fmt::Debug for TunnelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TunnelKey(…)")
    }
}

/// New key in hexadecimal (64 characters).
pub fn new_key() -> String {
    hex(&rand::random::<[u8; 32]>())
}

/// New tunnel identifier or secret (128 bits in hexadecimal).
pub fn new_token() -> String {
    hex(&rand::random::<[u8; 16]>())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl TunnelKey {
    /// Key of 64 hexadecimal characters; `None` otherwise.
    pub fn parse(hex: &str) -> Option<Self> {
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let bytes: Vec<u8> = (0..32)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16))
            .collect::<Result<_, _>>()
            .ok()?;
        let key = UnboundKey::new(&CHACHA20_POLY1305, &bytes).ok()?;
        Some(Self(std::sync::Arc::new(LessSafeKey::new(key))))
    }

    fn seal(&self, aad: &[u8], plain: &[u8]) -> (String, NonceBytes) {
        let nonce: NonceBytes = rand::random();
        let mut data = plain.to_vec();
        // Can only fail beyond 256 GiB per packet.
        let sealed = self.0.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad),
            &mut data,
        );
        if sealed.is_err() {
            return (String::new(), nonce);
        }
        let mut packet = nonce.to_vec();
        packet.extend_from_slice(&data);
        (STANDARD.encode(packet), nonce)
    }

    fn open(&self, aad: &[u8], packet: &str) -> Option<(Vec<u8>, NonceBytes)> {
        let mut bytes = STANDARD.decode(packet.trim()).ok()?;
        if bytes.len() < NONCE_LEN {
            return None;
        }
        let mut data = bytes.split_off(NONCE_LEN);
        let nonce: NonceBytes = bytes.try_into().ok()?;
        let plain = self
            .0
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut data,
            )
            .ok()?;
        Some((plain.to_vec(), nonce))
    }
}

fn request_aad(tunnel: &str) -> Vec<u8> {
    format!("seaglass-relay-v1 request {tunnel}").into_bytes()
}

fn response_aad(tunnel: &str, request: &NonceBytes) -> Vec<u8> {
    let mut aad = format!("seaglass-relay-v1 response {tunnel} ").into_bytes();
    aad.extend_from_slice(request);
    aad
}

/// Request from the phone, in clear inside the packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Milliseconds since the Unix epoch.
    pub at: i64,
    /// `/api/…`.
    pub path: String,
    /// Owner code: the PC checks it as on the local network.
    pub code: String,
    pub body: String,
}

/// Response from the PC, in clear inside the packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

pub fn seal_request(key: &TunnelKey, tunnel: &str, request: &Request) -> (String, NonceBytes) {
    let plain = serde_json::to_vec(request).unwrap_or_default();
    key.seal(&request_aad(tunnel), &plain)
}

/// `None` if the packet cannot be read with this key (or was modified).
pub fn open_request(key: &TunnelKey, tunnel: &str, packet: &str) -> Option<(Request, NonceBytes)> {
    let (plain, nonce) = key.open(&request_aad(tunnel), packet)?;
    Some((serde_json::from_slice(&plain).ok()?, nonce))
}

pub fn seal_response(
    key: &TunnelKey,
    tunnel: &str,
    request: &NonceBytes,
    response: &Response,
) -> String {
    let plain = serde_json::to_vec(response).unwrap_or_default();
    key.seal(&response_aad(tunnel, request), &plain).0
}

pub fn open_response(
    key: &TunnelKey,
    tunnel: &str,
    request: &NonceBytes,
    packet: &str,
) -> Option<Response> {
    let (plain, _) = key.open(&response_aad(tunnel, request), packet)?;
    serde_json::from_slice(&plain).ok()
}

/// PC side: refuses a request that is too old or already seen.
#[derive(Default)]
pub struct Replay {
    seen: HashMap<NonceBytes, i64>,
    order: VecDeque<NonceBytes>,
}

impl Replay {
    /// True if the request dated `at`, with nonce `nonce`, is accepted (it is remembered).
    pub fn accept(&mut self, nonce: NonceBytes, at: i64, now: i64) -> bool {
        if (at - now).abs() > MAX_SKEW_MS {
            return false;
        }
        // Nonces that left the window are no longer needed: a request that old would be refused by
        // its date.
        while let Some(old) = self.order.front() {
            let stale = self.seen.get(old).is_none_or(|t| now - t > 2 * MAX_SKEW_MS);
            if !stale && self.order.len() < MAX_SEEN {
                break;
            }
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        if self.seen.contains_key(&nonce) {
            return false;
        }
        self.seen.insert(nonce, now);
        self.order.push_back(nonce);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TUNNEL: &str = "0123456789abcdef0123456789abcdef";

    fn key() -> TunnelKey {
        TunnelKey::parse(&new_key()).unwrap()
    }

    fn request() -> Request {
        Request {
            at: 1_000,
            path: "/api/plays/sync".into(),
            code: "AB23CDEF45GH".into(),
            body: "{\"x\":1}".into(),
        }
    }

    #[test]
    fn keys_are_checked() {
        assert!(TunnelKey::parse(&new_key()).is_some());
        assert!(TunnelKey::parse("abcd").is_none());
        assert!(TunnelKey::parse(&"z".repeat(64)).is_none());
        assert_eq!(new_token().len(), 32);
        assert_ne!(new_key(), new_key());
    }

    #[test]
    fn a_request_and_its_response_round_trip() {
        let key = key();
        let (packet, nonce) = seal_request(&key, TUNNEL, &request());
        assert!(!packet.contains("plays"), "le relais ne lit rien");
        let (opened, seen) = open_request(&key, TUNNEL, &packet).unwrap();
        assert_eq!(opened, request());
        assert_eq!(seen, nonce);
        let response = Response {
            status: 200,
            body: "ok".into(),
        };
        let sealed = seal_response(&key, TUNNEL, &nonce, &response);
        assert_eq!(open_response(&key, TUNNEL, &nonce, &sealed), Some(response));
    }

    #[test]
    fn tampering_and_wrong_keys_are_refused() {
        let key = key();
        let (packet, nonce) = seal_request(&key, TUNNEL, &request());
        assert!(open_request(&TunnelKey::parse(&new_key()).unwrap(), TUNNEL, &packet).is_none());
        assert!(open_request(&key, "autre-tunnel", &packet).is_none());
        let mut bytes = STANDARD.decode(&packet).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(open_request(&key, TUNNEL, &STANDARD.encode(bytes)).is_none());
        assert!(open_request(&key, TUNNEL, "not base64!").is_none());
        // A response only opens for its request, and not as a request.
        let response = Response {
            status: 200,
            body: "ok".into(),
        };
        let sealed = seal_response(&key, TUNNEL, &nonce, &response);
        assert!(open_response(&key, TUNNEL, &[0; NONCE_LEN], &sealed).is_none());
        assert!(open_request(&key, TUNNEL, &sealed).is_none());
    }

    #[test]
    fn replays_and_old_requests_are_refused() {
        let mut replay = Replay::default();
        let now = 10_000_000;
        assert!(replay.accept([1; NONCE_LEN], now, now));
        assert!(!replay.accept([1; NONCE_LEN], now, now + 1));
        assert!(replay.accept([2; NONCE_LEN], now - MAX_SKEW_MS, now));
        assert!(!replay.accept([3; NONCE_LEN], now - MAX_SKEW_MS - 1, now));
        // Memory stays bounded.
        for i in 0..(MAX_SEEN as u32 + 10) {
            let mut nonce = [0; NONCE_LEN];
            nonce[..4].copy_from_slice(&i.to_le_bytes());
            nonce[11] = 9;
            replay.accept(nonce, now, now);
        }
        assert!(replay.seen.len() <= MAX_SEEN + 1);
    }
}
