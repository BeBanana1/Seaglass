//! Limites par adresse (fenêtre glissante) et outils de comparaison.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use rand::RngCore;

/// Au plus `max` événements par adresse sur `window`.
pub struct Rate {
    max: usize,
    window: Duration,
    hits: HashMap<IpAddr, VecDeque<Instant>>,
}

/// Adresses suivies au plus : au-delà, une nouvelle adresse est refusée jusqu'au ménage.
const MAX_ADDRESSES: usize = 50_000;

impl Rate {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: HashMap::new(),
        }
    }

    /// Vrai si l'événement est permis ; il est alors compté.
    pub fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        if !self.hits.contains_key(&ip) && self.hits.len() >= MAX_ADDRESSES {
            return false;
        }
        let hits = self.hits.entry(ip).or_default();
        while hits
            .front()
            .is_some_and(|t| now.duration_since(*t) >= self.window)
        {
            hits.pop_front();
        }
        if hits.len() >= self.max {
            return false;
        }
        hits.push_back(now);
        true
    }

    /// Oublie les adresses sans événement récent.
    pub fn prune(&mut self, now: Instant) {
        let window = self.window;
        self.hits
            .retain(|_, hits| hits.back().is_some_and(|t| now.duration_since(*t) < window));
    }
}

/// 128 bits aléatoires en hexadécimal.
pub fn random_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Comparaison en temps constant (clé d'hôte, secret de reprise).
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Code saisi par un invité : casse, espaces et tirets ignorés (comme l'app).
pub fn code_matches(expected: &str, given: &str) -> bool {
    let given: String = given
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    !expected.is_empty() && same(expected, &given)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    #[test]
    fn rate_is_a_sliding_window() {
        let mut rate = Rate::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(rate.allow(IP, t0));
        assert!(rate.allow(IP, t0));
        assert!(!rate.allow(IP, t0 + Duration::from_secs(59)));
        assert!(rate.allow(IP, t0 + Duration::from_secs(60)));
        rate.prune(t0 + Duration::from_secs(200));
        assert!(rate.hits.is_empty());
    }

    #[test]
    fn codes_and_secrets() {
        assert!(code_matches("AB23CD", "ab2-3 cd"));
        assert!(!code_matches("AB23CD", "AB23CE"));
        assert!(!code_matches("", ""));
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert_eq!(random_token().len(), 32);
    }
}
