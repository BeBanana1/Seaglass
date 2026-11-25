//! Per-address limits (sliding window) and comparison helpers.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::net::{IpAddr, Ipv6Addr};
use std::time::{Duration, Instant};

use rand::RngCore;

/// At most `max` events per address over `window`.
pub struct Rate {
    max: usize,
    window: Duration,
    hits: HashMap<IpAddr, VecDeque<Instant>>,
}

/// Most addresses tracked: beyond that, a new address is refused until the next sweep.
const MAX_ADDRESSES: usize = 50_000;

impl Rate {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: HashMap::new(),
        }
    }

    /// True if the event is allowed; it is then counted.
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

    /// Forgets the addresses without a recent event.
    pub fn prune(&mut self, now: Instant) {
        let window = self.window;
        self.hits
            .retain(|_, hits| hits.back().is_some_and(|t| now.duration_since(*t) < window));
    }
}

/// The address every limit counts: an IPv4 as is (also when seen as `::ffff:a.b.c.d`), an
/// IPv6 reduced to its /64 prefix. A provider gives each customer at least a /64: counted
/// one by one, its addresses would let a single client slip past every per-address limit,
/// or fill the tables, which then refuse every new address.
pub fn network(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & (u128::MAX << 64))),
        },
    }
}

/// 128 random bits in hexadecimal.
pub fn random_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Constant-time comparison (host key, resume secret).
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Code typed by a guest: case, spaces and dashes ignored (like the app).
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
    fn an_ipv6_network_counts_as_one_address() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let prefix = ip("2001:db8:1:2::");
        assert_eq!(network(ip("2001:db8:1:2:aaaa:bbbb:cccc:dddd")), prefix);
        assert_eq!(network(ip("2001:db8:1:2::1")), prefix);
        assert_ne!(network(ip("2001:db8:1:3::1")), prefix);
        assert_eq!(network(ip("::ffff:192.0.2.7")), ip("192.0.2.7"));
        assert_eq!(network(IP), IP);

        let mut rate = Rate::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(rate.allow(network(ip("2001:db8:1:2::1")), t0));
        assert!(rate.allow(network(ip("2001:db8:1:2::2")), t0));
        assert!(!rate.allow(network(ip("2001:db8:1:2::3")), t0));
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
