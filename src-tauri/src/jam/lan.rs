//! Adresse du PC sur le réseau local, donnée aux téléphones (Jam locale, télécommande).
//!
//! La route par défaut ne suffit pas : avec un VPN, elle passe par le tunnel et donnerait
//! l'adresse du VPN (#425). Sous Windows, on liste donc les cartes réseau et on garde une
//! adresse IPv4 privée d'une carte active avec une passerelle, hors tunnels.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

/// Adresse IPv4 d'une carte réseau candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Candidate {
    ip: Ipv4Addr,
    /// Métrique IPv4 de la carte : la plus petite est la préférée de Windows.
    metric: u32,
}

/// Adresse du PC sur le réseau local, `None` si aucune n'a été trouvée.
pub(super) fn lan_ip() -> Option<IpAddr> {
    let routed = routed_ip();
    #[cfg(windows)]
    let candidates = adapters::candidates();
    #[cfg(not(windows))]
    let candidates = Vec::new();
    choose(routed, &candidates)
}

/// Choix pur : l'adresse de la route par défaut si elle est sur le réseau local, sinon la
/// carte de plus petite métrique, sinon la route par défaut (aucune carte lue).
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

/// Adresse source de la route par défaut (`connect` UDP : rien n'est envoyé).
fn routed_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// Adresse privée (RFC 1918), hors liens locaux 169.254 (carte sans DHCP).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_lan(ip: Ipv4Addr) -> bool {
    ip.is_private()
}

/// Types de cartes écartés : boucle locale, PPP, tunnels (WireGuard se déclare
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

    /// Adresses IPv4 privées des cartes actives avec une passerelle, hors tunnels.
    pub(super) fn candidates() -> Vec<Candidate> {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS
            | GAA_FLAG_SKIP_ANYCAST
            | GAA_FLAG_SKIP_MULTICAST
            | GAA_FLAG_SKIP_DNS_SERVER;
        // Tampon en u64 : les structures demandent un alignement de 8.
        let mut size: u32 = 16 * 1024;
        for _ in 0..3 {
            let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
            let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            // SAFETY : `buffer` fait au moins `size` octets, aligné sur 8.
            let result = unsafe {
                GetAdaptersAddresses(u32::from(AF_INET.0), flags, None, Some(first), &mut size)
            };
            match result {
                ERROR_SUCCESS => {
                    // SAFETY : liste remplie par l'appel, qui vit dans `buffer`.
                    return unsafe { read(first) };
                }
                ERROR_BUFFER_OVERFLOW => continue,
                code => {
                    tracing::debug!("cartes réseau illisibles : erreur {code}");
                    return Vec::new();
                }
            }
        }
        Vec::new()
    }

    /// Parcourt la liste chaînée remplie par `GetAdaptersAddresses`.
    ///
    /// # Safety
    /// `first` doit pointer sur une liste valide, vivante pendant l'appel.
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
        // ProtonVPN : route par défaut sur 10.2.0.2, carte de la maison en 192.168.1.105.
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

    /// Adresse retenue sur cette machine : `cargo test lan_ip_here -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lan_ip_here() {
        println!("route par défaut : {:?}", routed_ip());
        println!("adresse retenue : {:?}", lan_ip());
    }
}
