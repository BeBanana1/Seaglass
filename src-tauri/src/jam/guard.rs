//! Code d'accès et limites par invité et par adresse : logique pure, horloge passée en
//! argument.
//!
//! En données mobiles, plusieurs invités sortent souvent par la même adresse (#256) : chaque
//! action est donc comptée par invité (jeton du navigateur, sinon l'adresse) aux seuils
//! ci-dessous, et par adresse avec un seuil `IP_FACTOR` fois plus large. Le jeton n'est pas
//! une identité forte (un navigateur en tire un autre quand il veut) : le plafond par
//! adresse reste la vraie borne.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

use rand::Rng;

/// Sans 0/O, 1/I : le code se lit à voix haute ou sur un écran de loin.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 6;
/// Essais ratés par adresse sur `FAILURE_WINDOW` avant que l'adresse soit bloquée ; le
/// blocage tombe quand les essais les plus anciens sortent de la fenêtre (#256).
pub const MAX_FAILURES: usize = 10;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
pub const ADDS_PER_MINUTE: usize = 10;
/// Assez pour la recherche pendant la saisie (une requête par pause de la frappe).
pub const SEARCHES_PER_MINUTE: usize = 60;
/// Lecture / pause (#222) : assez pour l'usage normal, pas pour en faire un jeu.
pub const CONTROLS_PER_MINUTE: usize = 6;
/// Retraits de ses propres ajouts (#223).
pub const REMOVES_PER_MINUTE: usize = 30;
/// Seuils d'une adresse = seuils d'un invité × ce facteur (#256).
pub const IP_FACTOR: usize = 5;
/// Adresses et invités suivis au plus ; les inactifs sont oubliés quand la table est pleine.
const MAX_ADDRESSES: usize = 1024;
const MAX_GUESTS: usize = 4096;
/// Titres proposés par les recherches de la Jam, les seuls que les invités peuvent ajouter.
const MAX_OFFERED: usize = 5000;
const WINDOW: Duration = Duration::from_secs(60);

/// Commandes du propriétaire par minute (télécommande, #336) : un curseur de volume ou de
/// position en envoie une par geste, jamais une rafale.
pub const COMMANDS_PER_MINUTE: usize = 120;

/// 32⁶ ≈ 1,07 milliard de codes.
pub fn new_code() -> String {
    code_of_len(CODE_LEN)
}

/// Code de `len` caractères de l'alphabet sans ambiguïté (5 bits chacun).
pub fn code_of_len(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Vrai si `code` n'a que des caractères de l'alphabet (code relu dans les réglages).
pub fn is_code(code: &str, len: usize) -> bool {
    code.len() == len && code.bytes().all(|b| ALPHABET.contains(&b))
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
    /// Commande du propriétaire (télécommande, #336).
    Command,
}

impl Action {
    fn per_minute(self) -> usize {
        match self {
            Self::Search => SEARCHES_PER_MINUTE,
            Self::Add => ADDS_PER_MINUTE,
            Self::Control => CONTROLS_PER_MINUTE,
            Self::Remove => REMOVES_PER_MINUTE,
            Self::Command => COMMANDS_PER_MINUTE,
        }
    }
}

/// Actions récentes d'un invité ou d'une adresse.
#[derive(Default)]
struct Counters {
    searches: VecDeque<Instant>,
    adds: VecDeque<Instant>,
    controls: VecDeque<Instant>,
    removes: VecDeque<Instant>,
    commands: VecDeque<Instant>,
}

impl Counters {
    fn times(&mut self, action: Action) -> &mut VecDeque<Instant> {
        match action {
            Action::Search => &mut self.searches,
            Action::Add => &mut self.adds,
            Action::Control => &mut self.controls,
            Action::Remove => &mut self.removes,
            Action::Command => &mut self.commands,
        }
    }

    /// Place pour une action de plus sous `max` (anciennes actions oubliées au passage).
    fn has_room(&mut self, action: Action, max: usize, now: Instant) -> bool {
        let times = self.times(action);
        while times
            .front()
            .is_some_and(|t| now.duration_since(*t) >= WINDOW)
        {
            times.pop_front();
        }
        times.len() < max
    }

    fn idle(&self, now: Instant) -> bool {
        [
            &self.searches,
            &self.adds,
            &self.controls,
            &self.removes,
            &self.commands,
        ]
        .iter()
        .all(|q| q.back().is_none_or(|t| now.duration_since(*t) >= WINDOW))
    }
}

#[derive(Default)]
struct Address {
    failures: VecDeque<Instant>,
    counters: Counters,
}

impl Address {
    fn prune_failures(&mut self, now: Instant) {
        while self
            .failures
            .front()
            .is_some_and(|t| now.duration_since(*t) >= FAILURE_WINDOW)
        {
            self.failures.pop_front();
        }
    }

    fn idle(&mut self, now: Instant) -> bool {
        self.prune_failures(now);
        self.failures.is_empty() && self.counters.idle(now)
    }
}

#[derive(Default)]
pub struct Limits {
    addresses: HashMap<IpAddr, Address>,
    guests: HashMap<String, Counters>,
}

impl Limits {
    /// Adresse bloquée (trop d'essais ratés récents) ou refusée (trop d'adresses actives).
    pub fn blocked(&mut self, ip: IpAddr, now: Instant) -> bool {
        match self.address(ip, now) {
            Some(a) => {
                a.prune_failures(now);
                a.failures.len() >= MAX_FAILURES
            }
            None => true,
        }
    }

    pub fn fail(&mut self, ip: IpAddr, now: Instant) {
        if let Some(a) = self.address(ip, now) {
            a.failures.push_back(now);
        }
    }

    /// Vrai si l'action est permise maintenant à l'invité `guest` (sa clé, `owner_key`)
    /// depuis `ip` ; elle est alors comptée pour les deux.
    pub fn allow(&mut self, ip: IpAddr, guest: &str, action: Action, now: Instant) -> bool {
        let max = action.per_minute();
        let ip_room = match self.address(ip, now) {
            Some(a) => a.counters.has_room(action, max * IP_FACTOR, now),
            None => false,
        };
        if !ip_room {
            return false;
        }
        let Some(counters) = self.guest(guest, now) else {
            return false;
        };
        if !counters.has_room(action, max, now) {
            return false;
        }
        counters.times(action).push_back(now);
        if let Some(a) = self.addresses.get_mut(&ip) {
            a.counters.times(action).push_back(now);
        }
        true
    }

    fn address(&mut self, ip: IpAddr, now: Instant) -> Option<&mut Address> {
        if !self.addresses.contains_key(&ip) && self.addresses.len() >= MAX_ADDRESSES {
            self.addresses.retain(|_, a| !a.idle(now));
            if self.addresses.len() >= MAX_ADDRESSES {
                return None;
            }
        }
        Some(self.addresses.entry(ip).or_default())
    }

    fn guest(&mut self, key: &str, now: Instant) -> Option<&mut Counters> {
        if !self.guests.contains_key(key) && self.guests.len() >= MAX_GUESTS {
            self.guests.retain(|_, c| !c.idle(now));
            if self.guests.len() >= MAX_GUESTS {
                return None;
            }
        }
        Some(self.guests.entry(key.to_owned()).or_default())
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
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));

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
    fn failures_block_the_address_for_a_while() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for i in 0..MAX_FAILURES - 1 {
            limits.fail(IP, t0 + Duration::from_secs(i as u64));
        }
        assert!(!limits.blocked(IP, t0));
        limits.fail(IP, t0 + Duration::from_secs(60));
        assert!(limits.blocked(IP, t0 + Duration::from_secs(60)));
        assert!(!limits.blocked(OTHER, t0));
        // Le premier essai sort de la fenêtre : l'adresse peut réessayer.
        assert!(limits.blocked(IP, t0 + FAILURE_WINDOW - Duration::from_secs(1)));
        assert!(!limits.blocked(IP, t0 + FAILURE_WINDOW));
    }

    #[test]
    fn adds_are_limited_per_guest_and_minute() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for _ in 0..ADDS_PER_MINUTE {
            assert!(limits.allow(IP, "lea", Action::Add, t0));
        }
        assert!(!limits.allow(IP, "lea", Action::Add, t0 + Duration::from_secs(30)));
        // Un autre invité derrière la même adresse a ses propres ajouts (#256).
        assert!(limits.allow(IP, "max", Action::Add, t0));
        // Les recherches et les commandes de lecture ont leur propre compteur.
        assert!(limits.allow(IP, "lea", Action::Search, t0));
        assert!(limits.allow(IP, "lea", Action::Control, t0));
        assert!(limits.allow(IP, "lea", Action::Add, t0 + WINDOW));
    }

    #[test]
    fn an_address_keeps_a_wider_ceiling() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        // Des jetons neufs à chaque ajout ne dépassent pas le plafond de l'adresse.
        for i in 0..ADDS_PER_MINUTE * IP_FACTOR {
            assert!(limits.allow(IP, &format!("k{i}"), Action::Add, t0));
        }
        assert!(!limits.allow(IP, "fresh", Action::Add, t0));
        assert!(limits.allow(OTHER, "fresh", Action::Add, t0));
        // Un refus de l'adresse ne compte pas pour l'invité.
        assert!(limits.allow(IP, "fresh", Action::Add, t0 + WINDOW));
    }

    #[test]
    fn idle_guests_are_forgotten_when_the_table_is_full() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for i in 0..MAX_GUESTS {
            limits.guests.insert(format!("k{i}"), Counters::default());
        }
        limits.guests.get_mut("k0").unwrap().adds.push_back(t0);
        assert!(limits.allow(IP, "new", Action::Search, t0 + Duration::from_secs(1)));
        assert!(limits.guests.contains_key("k0"));
        assert_eq!(limits.guests.len(), 2);
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
