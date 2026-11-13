//! Paquets chiffrés de bout en bout entre le téléphone et son ordinateur lié, quand ils
//! passent par le relais (#430). Le relais ne voit qu'une suite d'octets.
//!
//! ChaCha20-Poly1305 (`aws-lc-rs`), clé de 256 bits tirée par le PC et donnée au téléphone
//! sur le réseau local seulement, nonce aléatoire de 96 bits par paquet. Les données
//! associées lient chaque paquet au tunnel et à son sens, et chaque réponse au nonce de sa
//! requête : le relais ne peut ni lire, ni modifier, ni rejouer une réponse sur une autre
//! requête. Contre le rejeu d'une requête, elle porte sa date et le PC retient les nonces
//! récents ([`Replay`]).
//!
//! Paquet : base64 (nonce ‖ chiffré ‖ étiquette).

use std::collections::{HashMap, VecDeque};

use aws_lc_rs::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

/// Une requête plus vieille (ou plus en avance, horloges décalées) est refusée.
pub const MAX_SKEW_MS: i64 = 5 * 60 * 1000;
/// Nonces de requêtes retenus au plus (bien au-delà de 120 appels par minute sur 10 min).
const MAX_SEEN: usize = 4_096;

pub type NonceBytes = [u8; NONCE_LEN];

/// Clé partagée du tunnel.
#[derive(Clone)]
pub struct TunnelKey(std::sync::Arc<LessSafeKey>);

impl std::fmt::Debug for TunnelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TunnelKey(…)")
    }
}

/// Nouvelle clé en hexadécimal (64 caractères).
pub fn new_key() -> String {
    hex(&rand::random::<[u8; 32]>())
}

/// Nouvel identifiant ou secret de tunnel (128 bits en hexadécimal).
pub fn new_token() -> String {
    hex(&rand::random::<[u8; 16]>())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl TunnelKey {
    /// Clé de 64 caractères hexadécimaux ; `None` sinon.
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
        // Ne peut échouer qu'au-delà de 256 Gio par paquet.
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

/// Requête du téléphone, en clair à l'intérieur du paquet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Millisecondes depuis l'epoch Unix.
    pub at: i64,
    /// `/api/…`.
    pub path: String,
    /// Code de propriétaire : le PC le vérifie comme sur le réseau local.
    pub code: String,
    pub body: String,
}

/// Réponse du PC, en clair à l'intérieur du paquet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

pub fn seal_request(key: &TunnelKey, tunnel: &str, request: &Request) -> (String, NonceBytes) {
    let plain = serde_json::to_vec(request).unwrap_or_default();
    key.seal(&request_aad(tunnel), &plain)
}

/// `None` si le paquet n'est pas lisible avec cette clé (ou a été modifié).
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

/// Côté PC : refuse une requête trop vieille ou déjà vue.
#[derive(Default)]
pub struct Replay {
    seen: HashMap<NonceBytes, i64>,
    order: VecDeque<NonceBytes>,
}

impl Replay {
    /// Vrai si la requête datée `at`, de nonce `nonce`, est acceptée (elle est retenue).
    pub fn accept(&mut self, nonce: NonceBytes, at: i64, now: i64) -> bool {
        if (at - now).abs() > MAX_SKEW_MS {
            return false;
        }
        // Les nonces sortis de la fenêtre ne servent plus : une requête aussi vieille
        // serait refusée par sa date.
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
        assert!(open_request(&key, TUNNEL, "pas du base64 !").is_none());
        // Une réponse ne s'ouvre que pour sa requête, et pas comme une requête.
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
        // La mémoire reste bornée.
        for i in 0..(MAX_SEEN as u32 + 10) {
            let mut nonce = [0; NONCE_LEN];
            nonce[..4].copy_from_slice(&i.to_le_bytes());
            nonce[11] = 9;
            replay.accept(nonce, now, now);
        }
        assert!(replay.seen.len() <= MAX_SEEN + 1);
    }
}
