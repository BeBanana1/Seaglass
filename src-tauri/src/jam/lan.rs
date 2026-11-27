//! The PC's address on the local network, given to phones (local Jam, remote control).
//!
//! The default route is not enough: with a VPN, it goes through the tunnel and would give
//! the VPN's address (#425). On Windows, we thus list the network adapters and keep a
//! private IPv4 address of an active adapter with a gateway, excluding tunnels.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

/// IPv4 address of a candidate network adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Candidate {
    ip: Ipv4Addr,
    /// IPv4 metric of the adapter: the smallest is the one Windows prefers.
    metric: u32,
}

/// The PC's address on the local network, `None` if none was found.
pub(super) fn lan_ip() -> Option<IpAddr> {
    let routed = routed_ip();
    #[cfg(windows)]
    let candidates = adapters::candidates();
    #[cfg(not(windows))]
    let candidates = Vec::new();
    choose(routed, &candidates)
}

/// Pure choice: the default route's address if it is on the local network, otherwise the
/// adapter with the smallest metric, otherwise the default route (no adapter read).
fn choose(routed: Option<IpAddr>, candidates: &[Candidate]) -> Option<IpAddr> {
    if candidates.is_empty() {
        return routed;
    }
    if let Some(IpAddr::V4(ip)) = routed
        && candidates.iter().any(|c| c.ip == ip)
    {
        return routed;
    }
    candidates
        .iter()
        .min_by_key(|c| c.metric)
        .map(|c| IpAddr::V4(c.ip))
}

/// Source address of the default route (UDP `connect`: nothing is sent).
fn routed_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// Private address (RFC 1918), excluding 169.254 link-local (adapter without DHCP).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_lan(ip: Ipv4Addr) -> bool {
    ip.is_private()
}

/// Adapter types ruled out: loopback, PPP, tunnels (WireGuard declares itself as
/// `IF_TYPE_PROP_VIRTUAL`, 53).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_tunnel(if_type: u32) -> bool {
    matches!(if_type, 23 | 24 | 53 | 108 | 131)
}

#[cfg(windows)]
mod adapters {
    use std::net::Ipv4Addr;

    use windows::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
        GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    use super::{Candidate, is_lan, is_tunnel};

    const ERROR_SUCCESS: u32 = 0;
    const ERROR_BUFFER_OVERFLOW: u32 = 111;

    /// Private IPv4 addresses of active adapters with a gateway, excluding tunnels.
    pub(super) fn candidates() -> Vec<Candidate> {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS
            | GAA_FLAG_SKIP_ANYCAST
            | GAA_FLAG_SKIP_MULTICAST
            | GAA_FLAG_SKIP_DNS_SERVER;
        // Buffer in u64: the structures require an alignment of 8.
        let mut size: u32 = 16 * 1024;
        for _ in 0..3 {
            let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
            let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            // SAFETY: `buffer` is at least `size` bytes, aligned on 8.
            let result = unsafe {
                GetAdaptersAddresses(u32::from(AF_INET.0), flags, None, Some(first), &mut size)
            };
            match result {
                ERROR_SUCCESS => {
                    // SAFETY: list filled by the call, which lives in `buffer`.
                    return unsafe { read(first) };
                }
                ERROR_BUFFER_OVERFLOW => continue,
                code => {
                    tracing::debug!("unreadable network adapters: error {code}");
                    return Vec::new();
                }
            }
        }
        Vec::new()
    }

    /// Walks the linked list filled by `GetAdaptersAddresses`.
    ///
    /// # Safety
    /// `first` must point to a valid list, alive during the call.
    unsafe fn read(first: *const IP_ADAPTER_ADDRESSES_LH) -> Vec<Candidate> {
        let mut found = Vec::new();
        let mut adapter = first;
        while let Some(a) = unsafe { adapter.as_ref() } {
            adapter = a.Next;
            if a.OperStatus != IfOperStatusUp
                || is_tunnel(a.IfType)
                || a.FirstGatewayAddress.is_null()
            {
                continue;
            }
            let mut unicast = a.FirstUnicastAddress;
            while let Some(u) = unsafe { unicast.as_ref() } {
                unicast = u.Next;
                let sockaddr = u.Address.lpSockaddr;
                if sockaddr.is_null() || unsafe { (*sockaddr).sa_family } != AF_INET {
                    continue;
                }
                let sin = unsafe { &*sockaddr.cast::<SOCKADDR_IN>() };
                let b = unsafe { sin.sin_addr.S_un.S_un_b };
                let ip = Ipv4Addr::new(b.s_b1, b.s_b2, b.s_b3, b.s_b4);
                if is_lan(ip) {
                    found.push(Candidate {
                        ip,
                        metric: a.Ipv4Metric,
                    });
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(ip: [u8; 4], metric: u32) -> Candidate {
        Candidate {
            ip: Ipv4Addr::from(ip),
            metric,
        }
    }

    fn v4(ip: [u8; 4]) -> Option<IpAddr> {
        Some(IpAddr::V4(Ipv4Addr::from(ip)))
    }

    #[test]
    fn keeps_the_routed_address_when_it_is_on_the_lan() {
        let cards = [
            candidate([192, 168, 1, 105], 35),
            candidate([10, 0, 0, 2], 5),
        ];
        assert_eq!(
            choose(v4([192, 168, 1, 105]), &cards),
            v4([192, 168, 1, 105])
        );
    }

    #[test]
    fn skips_the_vpn_address_of_the_default_route() {
        // ProtonVPN: default route on 10.2.0.2, home adapter on 192.168.1.105.
        let cards = [candidate([192, 168, 1, 105], 35)];
        assert_eq!(choose(v4([10, 2, 0, 2]), &cards), v4([192, 168, 1, 105]));
    }

    #[test]
    fn prefers_the_smallest_metric() {
        let cards = [
            candidate([192, 168, 1, 5], 50),
            candidate([192, 168, 0, 7], 25),
        ];
        assert_eq!(choose(v4([10, 2, 0, 2]), &cards), v4([192, 168, 0, 7]));
    }

    #[test]
    fn falls_back_to_the_default_route() {
        assert_eq!(choose(v4([10, 2, 0, 2]), &[]), v4([10, 2, 0, 2]));
        assert_eq!(choose(None, &[]), None);
    }

    #[test]
    fn tunnels_and_link_local_are_left_out() {
        assert!(is_tunnel(53));
        assert!(is_tunnel(131));
        assert!(!is_tunnel(6));
        assert!(!is_tunnel(71));
        assert!(!is_lan(Ipv4Addr::new(169, 254, 1, 1)));
        assert!(!is_lan(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(is_lan(Ipv4Addr::new(172, 16, 0, 1)));
    }

    /// Address chosen on this machine: `cargo test lan_ip_here -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lan_ip_here() {
        println!("default route: {:?}", routed_ip());
        println!("chosen address: {:?}", lan_ip());
    }
}
