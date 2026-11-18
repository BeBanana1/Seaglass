//! Notifications des nouvelles sorties des artistes suivis (#435), désactivées par défaut.
//!
//! Après chaque lecture du fil en fond (`feed::spawn_watch` : au lancement puis toutes les
//! 6 h), les albums pas vus, sortis depuis moins de `RECENT_DAYS` jours et pas encore
//! notifiés donnent une notification du système : une sortie ouvre son album, plusieurs
//! ouvrent le fil. Les clés notifiées sont gardées par compte (réglage `feed_notified`).
//! Sans ensemble pour ce compte (premier passage après l'activation, autre compte), il est
//! créé avec les sorties du moment, sans rien notifier : pas de rafale à l'activation.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

use super::feed::{SeenSet, item_key, mark_all};
use super::link::LinkTarget;
use super::play_stats::blocking;
use crate::i18n::{tr, trf};
use crate::integrations::notify::{self, Notice};
use crate::store::Store;
use crate::tidal::models::{AlbumSummary, FeedItem, MediaItem};

const NOTIFIED_KEY: &str = "feed_notified";
/// Une sortie plus ancienne n'est plus « nouvelle » : le fil remonte à 3 mois.
const RECENT_DAYS: i64 = 3;
const DAY_MS: i64 = 86_400_000;
/// Artistes cités dans une notification groupée.
const MAX_ARTISTS: usize = 3;

/// Interrupteur des Réglages, reporté par `UiSettings::apply_to_backend`.
static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Notifie les nouvelles sorties de `items` (fil déjà marqué vu ou non), si le réglage est
/// activé. Les erreurs ne font que passer au journal : c'est une tâche de fond.
pub async fn check(app: &AppHandle, store: Option<&Arc<Store>>, user: u64, items: &[FeedItem]) {
    if !enabled() {
        return;
    }
    let Some(store) = store else { return };
    let saved = match blocking(store, |s| s.get_setting::<SeenSet>(NOTIFIED_KEY)).await {
        Ok(saved) => saved,
        Err(e) => {
            // Illisible : traité comme un premier passage, sans notifier.
            tracing::warn!("{NOTIFIED_KEY} illisible : {e}");
            None
        }
    };
    let picked = pick(items, saved, user, super::feed::now_ms());
    if let Some(save) = picked.save
        && let Err(e) = blocking(store, move |s| s.set_setting(NOTIFIED_KEY, &save)).await
    {
        // Pas enregistré : on ne notifie pas, sinon la même sortie reviendrait dans 6 h.
        tracing::warn!("{NOTIFIED_KEY} non enregistré : {e}");
        return;
    }
    if let Some(notice) = notice(&picked.fresh) {
        notify::show(app, notice).await;
    }
}

/// Résultat de [`pick`] : sorties à notifier, et ensemble à enregistrer s'il change.
#[derive(Debug, PartialEq)]
struct Picked<'a> {
    fresh: Vec<&'a AlbumSummary>,
    save: Option<SeenSet>,
}

fn pick(items: &[FeedItem], saved: Option<SeenSet>, user: u64, now_ms: i64) -> Picked<'_> {
    let since = now_ms - RECENT_DAYS * DAY_MS;
    let candidates: Vec<&AlbumSummary> = items
        .iter()
        .filter(|i| !i.seen && i.occurred_at.is_some_and(|at| at >= since))
        .filter_map(|i| match &i.item {
            MediaItem::Album(album) => Some(album),
            _ => None,
        })
        .collect();
    let key = |a: &AlbumSummary| item_key(&MediaItem::Album(a.clone()));
    match saved.filter(|s| s.user == user) {
        None => Picked {
            fresh: Vec::new(),
            save: Some(mark_all(
                None,
                user,
                candidates.iter().map(|a| key(a)).collect(),
            )),
        },
        Some(saved) => {
            let known: HashSet<&str> = saved.keys.iter().map(String::as_str).collect();
            let fresh: Vec<&AlbumSummary> = candidates
                .into_iter()
                .filter(|a| !known.contains(key(a).as_str()))
                .collect();
            let save = (!fresh.is_empty())
                .then(|| mark_all(Some(saved), user, fresh.iter().map(|a| key(a)).collect()));
            Picked { fresh, save }
        }
    }
}

/// Texte et page de la notification, dans la langue de l'app.
fn notice(fresh: &[&AlbumSummary]) -> Option<Notice> {
    match fresh {
        [] => None,
        [album] => Some(Notice {
            title: tr("Nouvelle sortie", "New release").to_owned(),
            body: match artist(album) {
                Some(name) => format!("{name} · {}", album.title),
                None => album.title.clone(),
            },
            target: LinkTarget::Album {
                id: album.id,
                track: None,
            },
        }),
        many => {
            let mut names: Vec<&str> = Vec::new();
            for name in many.iter().filter_map(|a| artist(a)) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            let shown = names.len().min(MAX_ARTISTS);
            let mut body = names[..shown].join(", ");
            if names.len() > shown {
                body.push('…');
            }
            Some(Notice {
                title: trf!("{} nouvelles sorties", "{} new releases", many.len()),
                body,
                target: LinkTarget::Feed,
            })
        }
    }
}

fn artist(album: &AlbumSummary) -> Option<&str> {
    album.artists.first().map(|a| a.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{ArtistRef, MixSummary};

    const NOW: i64 = 1_000 * DAY_MS;

    fn album(id: u64, artist: &str, at: i64, seen: bool) -> FeedItem {
        FeedItem {
            occurred_at: Some(at),
            seen,
            item: MediaItem::Album(AlbumSummary {
                id,
                title: format!("Album {id}"),
                artists: vec![ArtistRef {
                    id,
                    name: artist.to_owned(),
                }],
                cover_url: None,
                cover_large_url: None,
                color: None,
                release_date: None,
                number_of_tracks: None,
                album_type: None,
                explicit: false,
                ai: false,
                hires: false,
            }),
        }
    }

    fn set(user: u64, keys: &[&str]) -> SeenSet {
        SeenSet {
            user,
            keys: keys.iter().map(|k| (*k).to_owned()).collect(),
        }
    }

    fn ids(picked: &Picked<'_>) -> Vec<u64> {
        picked.fresh.iter().map(|a| a.id).collect()
    }

    #[test]
    fn first_pass_seeds_without_notifying() {
        let items = vec![album(1, "A", NOW, false), album(2, "B", NOW, true)];
        let picked = pick(&items, None, 7, NOW);
        assert!(picked.fresh.is_empty());
        assert_eq!(picked.save, Some(set(7, &["album-1"])));
        // Autre compte : même chose.
        let picked = pick(&items, Some(set(8, &["album-9"])), 7, NOW);
        assert!(picked.fresh.is_empty());
        assert_eq!(picked.save, Some(set(7, &["album-1"])));
    }

    #[test]
    fn notifies_only_new_unseen_recent_albums() {
        let mut items = vec![
            album(1, "A", NOW - DAY_MS, false),
            // Déjà notifiée, vue, trop ancienne, sans date.
            album(2, "B", NOW, false),
            album(3, "C", NOW, true),
            album(4, "D", NOW - 4 * DAY_MS, false),
            FeedItem {
                occurred_at: None,
                ..album(5, "E", NOW, false)
            },
        ];
        // Un mix pas vu n'est pas une sortie.
        items.push(FeedItem {
            occurred_at: Some(NOW),
            seen: false,
            item: MediaItem::Mix(MixSummary {
                id: "m".to_owned(),
                title: String::new(),
                subtitle: None,
                image_url: None,
                mix_type: None,
            }),
        });
        let picked = pick(&items, Some(set(7, &["album-2"])), 7, NOW);
        assert_eq!(ids(&picked), [1]);
        assert_eq!(picked.save, Some(set(7, &["album-1", "album-2"])));
    }

    #[test]
    fn nothing_new_saves_nothing() {
        let items = vec![album(1, "A", NOW, false)];
        let picked = pick(&items, Some(set(7, &["album-1"])), 7, NOW);
        assert_eq!(
            picked,
            Picked {
                fresh: Vec::new(),
                save: None
            }
        );
    }

    #[test]
    fn notice_for_one_or_many() {
        let items = [
            album(1, "A", NOW, false),
            album(2, "B", NOW, false),
            album(3, "A", NOW, false),
            album(4, "C", NOW, false),
            album(5, "D", NOW, false),
        ];
        let albums: Vec<&AlbumSummary> = items
            .iter()
            .filter_map(|i| match &i.item {
                MediaItem::Album(a) => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(notice(&[]), None);
        let one = notice(&albums[..1]).unwrap();
        assert_eq!(one.body, "A · Album 1");
        assert_eq!(one.target, LinkTarget::Album { id: 1, track: None });
        let two = notice(&albums[..3]).unwrap();
        assert_eq!(two.body, "A, B");
        assert_eq!(two.target, LinkTarget::Feed);
        let all = notice(&albums).unwrap();
        assert_eq!(all.body, "A, B, C…");
    }
}
