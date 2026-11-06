//! Code d'accès et limites par adresse : logique pure, horloge passée en argument.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

use rand::Rng;

/// Sans 0/O, 1/I : le code se lit à voix haute ou sur un écran de loin.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 6;
/// Essais ratés avant que l'adresse soit bloquée jusqu'à la fin de la Jam.
pub const MAX_FAILURES: u32 = 10;
pub const ADDS_PER_MINUTE: usize = 10;
/// Assez pour la recherche pendant la saisie (une requête par pause de la frappe).
pub const SEARCHES_PER_MINUTE: usize = 60;
/// Lecture / pause (#222) : assez pour l'usage normal, pas pour en faire un jeu.
pub const CONTROLS_PER_MINUTE: usize = 6;
/// Retraits de ses propres ajouts (#223).
pub const REMOVES_PER_MINUTE: usize = 30;
/// Au-delà, une nouvelle adresse est refusée (réseau local : jamais atteint en pratique).
const MAX_CLIENTS: usize = 1024;
/// Titres proposés par les recherches de la Jam, les seuls que les invités peuvent ajouter.
const MAX_OFFERED: usize = 5000;
const WINDOW: Duration = Duration::from_secs(60);

/// 32⁶ ≈ 1,07 milliard de codes.
pub fn new_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Compare un code saisi (casse, espaces et tirets ignorés) en temps constant.
pub fn code_matches(expected: &str, given: &str) -> bool {
    let given: Vec<u8> = given
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'-')
        .map(|b| b.to_ascii_uppercase())
        .collect();
    let expected = expected.as_bytes();
    if given.len() != expected.len() {
        return false;
    }
    expected
        .iter()
        .zip(&given)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Search,
    Add,
    Control,
    Remove,
}

#[derive(Default)]
struct Client {
    failures: u32,
    searches: VecDeque<Instant>,
    adds: VecDeque<Instant>,
    controls: VecDeque<Instant>,
    removes: VecDeque<Instant>,
}

#[derive(Default)]
pub struct Limits {
    clients: HashMap<IpAddr, Client>,
}

impl Limits {
    /// Adresse bloquée (trop d'essais ratés) ou refusée (trop d'adresses).
    pub fn blocked(&self, ip: IpAddr) -> bool {
        match self.clients.get(&ip) {
            Some(c) => c.failures >= MAX_FAILURES,
            None => self.clients.len() >= MAX_CLIENTS,
        }
    }

    pub fn fail(&mut self, ip: IpAddr) {
        if let Some(c) = self.client(ip) {
            c.failures += 1;
        }
    }

    /// Vrai si l'action est permise maintenant ; elle est alors comptée.
    pub fn allow(&mut self, ip: IpAddr, action: Action, now: Instant) -> bool {
        let Some(c) = self.client(ip) else {
            return false;
        };
        let (times, max) = match action {
            Action::Search => (&mut c.searches, SEARCHES_PER_MINUTE),
            Action::Add => (&mut c.adds, ADDS_PER_MINUTE),
            Action::Control => (&mut c.controls, CONTROLS_PER_MINUTE),
            Action::Remove => (&mut c.removes, REMOVES_PER_MINUTE),
        };
        while times
            .front()
            .is_some_and(|t| now.duration_since(*t) >= WINDOW)
        {
            times.pop_front();
        }
        if times.len() >= max {
            return false;
        }
        times.push_back(now);
        true
    }

    fn client(&mut self, ip: IpAddr) -> Option<&mut Client> {
        if !self.clients.contains_key(&ip) && self.clients.len() >= MAX_CLIENTS {
            return None;
        }
        Some(self.clients.entry(ip).or_default())
    }
}

/// Titres sortis des recherches de la Jam : un invité ne peut ajouter qu'eux, pas
/// n'importe quel identifiant.
#[derive(Default)]
pub struct Offered {
    ids: HashSet<u64>,
    order: VecDeque<u64>,
}

impl Offered {
    pub fn note(&mut self, ids: impl IntoIterator<Item = u64>) {
        for id in ids {
            if self.ids.insert(id) {
                self.order.push_back(id);
            }
        }
        while self.order.len() > MAX_OFFERED {
            if let Some(old) = self.order.pop_front() {
                self.ids.remove(&old);
            }
        }
    }

    pub fn contains(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 20));

    #[test]
    fn codes_use_the_unambiguous_alphabet() {
        let code = new_code();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        assert!(!code.contains('O') && !code.contains('0'));
    }

    #[test]
    fn code_comparison_ignores_case_spaces_and_dashes() {
        assert!(code_matches("AB23CD", "ab2-3cd"));
        assert!(code_matches("AB23CD", " AB 23 CD "));
        assert!(!code_matches("AB23CD", "AB23CE"));
        assert!(!code_matches("AB23CD", "AB23C"));
        assert!(!code_matches("AB23CD", ""));
    }

    #[test]
    fn address_is_blocked_after_too_many_failures() {
        let mut limits = Limits::default();
        for _ in 0..MAX_FAILURES - 1 {
            limits.fail(IP);
        }
        assert!(!limits.blocked(IP));
        limits.fail(IP);
        assert!(limits.blocked(IP));
        let other = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 21));
        assert!(!limits.blocked(other));
    }

    #[test]
    fn adds_are_limited_per_minute() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for _ in 0..ADDS_PER_MINUTE {
            assert!(limits.allow(IP, Action::Add, t0));
        }
        assert!(!limits.allow(IP, Action::Add, t0 + Duration::from_secs(30)));
        // Les recherches et les commandes de lecture ont leur propre compteur.
        assert!(limits.allow(IP, Action::Search, t0));
        assert!(limits.allow(IP, Action::Control, t0));
        assert!(limits.allow(IP, Action::Add, t0 + WINDOW));
    }

    #[test]
    fn offered_ids_are_bounded() {
        let mut offered = Offered::default();
        offered.note(0..(MAX_OFFERED as u64 + 10));
        assert!(!offered.contains(0));
        assert!(offered.contains(MAX_OFFERED as u64 + 9));
        offered.note([5]);
        assert_eq!(offered.order.len(), MAX_OFFERED);
    }
}
