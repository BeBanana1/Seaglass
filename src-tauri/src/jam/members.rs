//! Invités qui ont ajouté des titres pendant la Jam (#288) : l'hôte les voit et peut les
//! exclure.
//!
//! Un invité est reconnu par sa **clé** : le jeton de son navigateur, ou son adresse pour une
//! page sans jeton (`owner_key`). Ce n'est pas une identité forte : un invité exclu peut
//! revenir en navigation privée. Sur le réseau local, l'exclusion bloque aussi son adresse
//! (un appareil = une adresse) ; en ligne, non : en données mobiles, plusieurs invités
//! sortent souvent par la même adresse (#256).

use std::collections::HashSet;
use std::net::IpAddr;

use serde::Serialize;

/// Invités retenus : au-delà, le plus ancien qui n'est pas exclu est oublié.
const MAX_MEMBERS: usize = 256;

/// Clé d'un invité : son jeton, sinon son adresse (préfixe qui ne peut pas être un jeton,
/// donc aucun jeton ne retire les titres d'une page sans jeton).
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

/// Invité tel que l'UI de l'hôte l'affiche ; la clé ne sort pas du module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestInfo {
    pub id: u32,
    pub name: Option<String>,
    /// Titres ajoutés pendant la Jam.
    pub added: u32,
    pub banned: bool,
}

#[derive(Debug, Default)]
pub struct Members {
    /// Dans l'ordre d'arrivée.
    list: Vec<Member>,
    next_id: u32,
    /// Adresses exclues (réseau local seulement).
    banned_ips: HashSet<IpAddr>,
}

impl Members {
    /// Note un ajout de `count` titres ; le dernier prénom et la dernière adresse l'emportent.
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

    /// Vrai si la clé ou l'adresse est exclue.
    pub fn is_banned(&self, key: &str, ip: IpAddr) -> bool {
        self.banned_ips.contains(&ip) || self.list.iter().any(|m| m.banned && m.key == key)
    }

    /// Exclut ou réadmet un invité ; `ban_ip` bloque aussi son adresse. Renvoie sa clé.
    pub fn set_banned(&mut self, id: u32, banned: bool, ban_ip: bool) -> Option<String> {
        let member = self.list.iter_mut().find(|m| m.id == id)?;
        member.banned = banned;
        let (key, ip) = (member.key.clone(), member.ip);
        if ban_ip && banned {
            self.banned_ips.insert(ip);
        } else if !banned && !self.list.iter().any(|m| m.banned && m.ip == ip) {
            // Une adresse reste bloquée tant qu'un autre invité exclu l'utilise.
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

        // En ligne : la clé seulement, l'autre invité de la même adresse passe.
        assert_eq!(members.set_banned(id, true, false).as_deref(), Some("aa"));
        assert!(members.is_banned("aa", B));
        assert!(!members.is_banned("bb", A));

        // Réseau local : l'adresse aussi, quelle que soit la clé.
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
