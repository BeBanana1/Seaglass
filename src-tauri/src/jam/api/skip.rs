//! Vote des invités pour passer le titre en cours (#433), si l'hôte le permet.
//!
//! Seuil fixe choisi par l'hôte (`JamRules::skip_votes`) : l'hôte ne sait pas combien
//! d'invités regardent la page, un seuil relatif aux « présents » serait faux. Un vote par
//! invité (jeton) et par titre ; sur le réseau local, un vote par adresse aussi (un appareil
//! qui ouvre des onglets privés ne vote qu'une fois). Par le relais, plusieurs invités en
//! données mobiles partagent une adresse : le jeton seul compte, les limites par adresse
//! bornent l'abus. Le vote nomme le titre visé : un vote parti juste avant un changement
//! de titre ne passe pas le suivant.

use std::collections::HashSet;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::{Backend, Core, backend_error};
use crate::jam::guard::Action;
use crate::jam::http::Response;

/// Votes du titre en cours, oubliés au changement de titre.
#[derive(Debug, Default)]
pub struct SkipVotes {
    track: Option<u64>,
    keys: HashSet<String>,
    ips: HashSet<IpAddr>,
    /// Seuil atteint pour ce titre : il est en train d'être passé, plus aucun vote.
    passed: bool,
}

/// Résultat d'un vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub votes: usize,
    /// Ce vote a atteint le seuil : passer le titre (une seule fois par titre).
    pub pass: bool,
}

impl SkipVotes {
    fn sync(&mut self, track: u64) {
        if self.track != Some(track) {
            *self = Self {
                track: Some(track),
                ..Self::default()
            };
        }
    }

    /// Votes pour `track` ; ceux d'un autre titre sont oubliés.
    pub fn count(&mut self, track: u64) -> usize {
        self.sync(track);
        self.keys.len()
    }

    /// Compte le vote de `key` (et de son adresse, si `ip`) pour `track`. Un second vote du
    /// même invité ou de la même adresse ne change rien.
    pub fn vote(&mut self, track: u64, key: &str, ip: Option<IpAddr>, needed: u8) -> Tally {
        self.sync(track);
        let fresh_ip = ip.is_none_or(|ip| !self.ips.contains(&ip));
        if !self.passed && fresh_ip && self.keys.insert(key.to_owned()) {
            if let Some(ip) = ip {
                self.ips.insert(ip);
            }
            if self.keys.len() >= usize::from(needed) {
                self.passed = true;
                return Tally {
                    votes: self.keys.len(),
                    pass: true,
                };
            }
        }
        Tally {
            votes: self.keys.len(),
            pass: false,
        }
    }
}

/// Vote vu par les invités : compte et seuil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestSkip {
    pub votes: usize,
    pub needed: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkipBody {
    track_id: u64,
}

impl Core {
    /// Vote du titre en cours pour l'état des invités, `None` sans la règle.
    pub(super) fn skip_view(&self, track: Option<u64>, game: bool) -> Option<GuestSkip> {
        let rules = self.rules();
        if !rules.skip_vote || game {
            return None;
        }
        let track = track?;
        let votes = self.skip_votes.lock().ok()?.count(track);
        Some(GuestSkip {
            votes,
            needed: rules.skip_votes,
        })
    }

    /// `POST api/skip {guest, trackId}` : vote pour passer `trackId`, s'il est encore en
    /// cours. Le vote qui atteint le seuil passe le titre.
    pub(super) async fn skip<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let rules = self.rules();
        if !rules.skip_vote {
            return Response::error(403, "skip");
        }
        let Ok(body) = serde_json::from_slice::<SkipBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.allow(ip, key, Action::Vote) {
            return Response::error(429, "slow");
        }
        let snapshot = match backend.state().await {
            Ok(snapshot) => snapshot,
            Err(e) => return backend_error(&e),
        };
        if snapshot.jam_game().is_some() {
            return Response::error(409, "game");
        }
        let now = snapshot.now.as_ref().map(|t| t.id.0);
        if now != Some(body.track_id) {
            return Response::error(409, "changed");
        }
        let ip = self.ban_ips.then_some(ip);
        let tally = match self.skip_votes.lock() {
            Ok(mut votes) => votes.vote(body.track_id, key, ip, rules.skip_votes),
            Err(_) => return Response::error(502, "backend"),
        };
        if tally.pass
            && let Err(e) = backend.skip_by_vote().await
        {
            return backend_error(&e);
        }
        self.refresh.notify_one();
        Response::json(
            200,
            &serde_json::json!({
                "votes": tally.votes,
                "needed": rules.skip_votes,
                "skipped": tally.pass,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAN: Option<IpAddr> = Some(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 20)));
    const OTHER: Option<IpAddr> = Some(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 21)));

    #[test]
    fn the_vote_that_reaches_the_threshold_passes_once() {
        let mut votes = SkipVotes::default();
        assert_eq!(
            votes.vote(7, "a", None, 2),
            Tally {
                votes: 1,
                pass: false
            }
        );
        // Même invité : rien ne change.
        assert_eq!(votes.vote(7, "a", None, 2).votes, 1);
        assert_eq!(
            votes.vote(7, "b", None, 2),
            Tally {
                votes: 2,
                pass: true
            }
        );
        // Seuil déjà atteint : le titre est en train d'être passé.
        assert!(!votes.vote(7, "c", None, 2).pass);
    }

    #[test]
    fn votes_are_forgotten_when_the_track_changes() {
        let mut votes = SkipVotes::default();
        votes.vote(7, "a", None, 3);
        votes.vote(7, "b", None, 3);
        assert_eq!(votes.count(7), 2);
        assert_eq!(votes.count(8), 0);
        assert_eq!(votes.vote(8, "a", None, 3).votes, 1);
    }

    #[test]
    fn on_the_local_network_one_device_votes_once() {
        let mut votes = SkipVotes::default();
        votes.vote(7, "onglet-1", LAN, 3);
        // Autre jeton (navigation privée), même téléphone : ignoré.
        assert_eq!(votes.vote(7, "onglet-2", LAN, 3).votes, 1);
        assert_eq!(votes.vote(7, "autre", OTHER, 3).votes, 2);
    }
}
