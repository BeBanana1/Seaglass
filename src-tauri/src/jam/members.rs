//! Guests who added tracks during the Jam (#288): the host sees them and can ban them.
//!
//! A guest is recognized by their **key**: their browser's token, or their address for a
//! page without a token (`owner_key`). This is not a strong identity: a banned guest can
//! come back in a private window. On the local network, a ban also blocks their address
//! (one device = one address); online, it does not: on mobile data, several guests often
//! go out through the same address (#256).

use std::collections::HashSet;
use std::net::IpAddr;

use serde::Serialize;

/// Guests kept: beyond that, the oldest one who is not banned is forgotten.
const MAX_MEMBERS: usize = 256;

/// A guest's key: their token, otherwise their address (prefix that cannot be a token,
/// so no token removes the tracks of a page without a token).
pub fn owner_key(token: Option<&str>, ip: IpAddr) -> String {
    token.map_or_else(|| format!("ip:{ip}"), str::to_owned)
}

#[derive(Debug, Clone)]
struct Member {
    id: u32,
    key: String,
    name: Option<String>,
    ip: IpAddr,
    added: u32,
    banned: bool,
}

/// Guest as the host's UI shows them; the key does not leave the module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestInfo {
    pub id: u32,
    pub name: Option<String>,
    /// Tracks added during the Jam.
    pub added: u32,
    pub banned: bool,
}

#[derive(Debug, Default)]
pub struct Members {
    /// In order of arrival.
    list: Vec<Member>,
    next_id: u32,
    /// Banned addresses (local network only).
    banned_ips: HashSet<IpAddr>,
}

impl Members {
    /// Records an addition of `count` tracks; the latest first name and address win.
    pub fn added(&mut self, key: &str, name: Option<String>, ip: IpAddr, count: u32) {
        if let Some(m) = self.list.iter_mut().find(|m| m.key == key) {
            if name.is_some() {
                m.name = name;
            }
            m.ip = ip;
            m.added = m.added.saturating_add(count);
            return;
        }
        if self.list.len() >= MAX_MEMBERS
            && let Some(i) = self.list.iter().position(|m| !m.banned)
        {
            self.list.remove(i);
        }
        self.next_id = self.next_id.wrapping_add(1);
        self.list.push(Member {
            id: self.next_id,
            key: key.to_owned(),
            name,
            ip,
            added: count,
            banned: false,
        });
    }

    /// True if the key or the address is banned.
    pub fn is_banned(&self, key: &str, ip: IpAddr) -> bool {
        self.banned_ips.contains(&ip) || self.list.iter().any(|m| m.banned && m.key == key)
    }

    /// Bans or readmits a guest; `ban_ip` also blocks their address. Returns their key.
    pub fn set_banned(&mut self, id: u32, banned: bool, ban_ip: bool) -> Option<String> {
        let member = self.list.iter_mut().find(|m| m.id == id)?;
        member.banned = banned;
        let (key, ip) = (member.key.clone(), member.ip);
        if ban_ip && banned {
            self.banned_ips.insert(ip);
        } else if !banned && !self.list.iter().any(|m| m.banned && m.ip == ip) {
            // An address stays blocked as long as another banned guest uses it.
            self.banned_ips.remove(&ip);
        }
        Some(key)
    }

    pub fn list(&self) -> Vec<GuestInfo> {
        self.list
            .iter()
            .map(|m| GuestInfo {
                id: m.id,
                name: m.name.clone(),
                added: m.added,
                banned: m.banned,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const A: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    const B: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));

    #[test]
    fn members_are_counted_and_renamed() {
        let mut members = Members::default();
        members.added("aa", Some("Léa".into()), A, 1);
        members.added("aa", None, A, 30);
        members.added("bb", Some("Max".into()), B, 1);
        let list = members.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name.as_deref(), Some("Léa"));
        assert_eq!(list[0].added, 31);
        assert_ne!(list[0].id, list[1].id);
    }

    #[test]
    fn a_ban_blocks_the_key_and_on_request_the_address() {
        let mut members = Members::default();
        members.added("aa", None, A, 1);
        members.added("bb", None, A, 1);
        let id = members.list()[0].id;

        // Online: the key only, the other guest at the same address gets through.
        assert_eq!(members.set_banned(id, true, false).as_deref(), Some("aa"));
        assert!(members.is_banned("aa", B));
        assert!(!members.is_banned("bb", A));

        // Local network: the address too, whatever the key.
        members.set_banned(id, true, true);
        assert!(members.is_banned("cc", A));

        members.set_banned(id, false, true);
        assert!(!members.is_banned("aa", A));
        assert_eq!(members.set_banned(999, true, true), None);
    }

    #[test]
    fn banned_members_are_kept_when_the_list_is_full() {
        let mut members = Members::default();
        members.added("first", None, A, 1);
        let id = members.list()[0].id;
        members.set_banned(id, true, false);
        for i in 0..MAX_MEMBERS {
            members.added(&format!("k{i}"), None, B, 1);
        }
        assert_eq!(members.list().len(), MAX_MEMBERS);
        assert!(members.is_banned("first", B));
        assert!(owner_key(None, A).starts_with("ip:"));
        assert_eq!(owner_key(Some("ab"), A), "ab");
    }
}
