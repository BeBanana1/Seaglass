//! Fil d'actualité (#14) et nouveautés des artistes suivis (#415).
//!
//! L'état « vu » est tenu en local (réglage `feed_seen`, par compte), pas chez TIDAL : le fil
//! est complété de sorties que TIDAL n'y met pas (`tidal/releases.rs`). Ouvrir la page marque
//! tout comme vu (`mark_feed_seen`). Le nombre de sorties pas encore vues part dans
//! l'événement `feed-unseen` (pastille de la navigation) : après chaque lecture du fil, et
//! toutes les 6 h depuis une tâche de fond.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use super::play_stats::blocking;
use super::{AppState, CmdResult, err};
use crate::content::filtered;
use crate::store::Store;
use crate::tidal::TidalClient;
use crate::tidal::models::{FeedItem, MediaItem};

const SEEN_KEY: &str = "feed_seen";
const UNSEEN_EVENT: &str = "feed-unseen";
/// Clés gardées : bien plus que les ~100 éléments d'un fil.
const MAX_SEEN: usize = 1000;
/// À la première lecture, une sortie plus ancienne compte comme déjà vue.
const FIRST_RUN_DAYS: i64 = 14;
const DAY_MS: i64 = 86_400_000;
/// Première lecture en fond après le lancement, puis intervalle entre deux.
const WATCH_DELAY: Duration = Duration::from_secs(20);
const WATCH_EVERY: Duration = Duration::from_secs(6 * 3600);
/// Nouvel essai quand la session n'est pas prête ou que la lecture a échoué.
const WATCH_RETRY: Duration = Duration::from_secs(5 * 60);

/// Clés du dernier fil lu, pour `mark_feed_seen`.
#[derive(Default)]
pub struct FeedState(Mutex<Vec<String>>);

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct SeenSet {
    pub(super) user: u64,
    /// Plus récentes d'abord.
    pub(super) keys: Vec<String>,
}

/// Sorties récentes des artistes suivis et mixes mensuels, `seen` selon l'état local.
#[tauri::command]
pub async fn feed(
    app: AppHandle,
    state: State<'_, AppState>,
    feed: State<'_, FeedState>,
) -> CmdResult<Vec<FeedItem>> {
    let (items, unseen) = load(&state.tidal, state.store.as_ref(), &feed).await?;
    emit_unseen(&app, unseen);
    Ok(items)
}

/// Marque comme vus tous les éléments du dernier fil lu.
#[tauri::command]
pub async fn mark_feed_seen(
    app: AppHandle,
    state: State<'_, AppState>,
    feed: State<'_, FeedState>,
) -> CmdResult<()> {
    let keys = feed.0.lock().map(|k| k.clone()).unwrap_or_default();
    if let (Some(store), Some(user)) = (state.store.as_ref(), user_id(&state.tidal).await) {
        blocking(store, move |store| {
            let saved = store.get_setting::<SeenSet>(SEEN_KEY)?;
            store.set_setting(SEEN_KEY, &mark_all(saved, user, keys))
        })
        .await?;
    }
    emit_unseen(&app, 0);
    Ok(())
}

/// Lit le fil en fond au lancement puis toutes les 6 h, pour la pastille et les
/// notifications des nouvelles sorties (`feed_alerts`, #435).
pub fn spawn_watch(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(WATCH_DELAY).await;
        loop {
            let state = app.state::<AppState>();
            let feed = app.state::<FeedState>();
            let wait = match load(&state.tidal, state.store.as_ref(), &feed).await {
                Ok((items, unseen)) => {
                    emit_unseen(&app, unseen);
                    if let Some(user) = user_id(&state.tidal).await {
                        super::feed_alerts::check(&app, state.store.as_ref(), user, &items).await;
                    }
                    WATCH_EVERY
                }
                Err(e) => {
                    tracing::debug!("fil d'actualité en fond : {e}");
                    WATCH_RETRY
                }
            };
            tokio::time::sleep(wait).await;
        }
    });
}

async fn load(
    tidal: &Arc<TidalClient>,
    store: Option<&Arc<Store>>,
    feed: &FeedState,
) -> CmdResult<(Vec<FeedItem>, usize)> {
    let user = user_id(tidal).await.ok_or_else(|| err("session absente"))?;
    let mut items = filtered(tidal.feed_with_releases(now_ms()).await.map_err(err)?);
    if let Some(store) = store {
        let saved = blocking(store, |s| s.get_setting::<SeenSet>(SEEN_KEY)).await;
        // Réglage illisible : traité comme une première lecture.
        let saved = saved.unwrap_or_else(|e| {
            tracing::warn!("{SEEN_KEY} illisible : {e}");
            None
        });
        if let Some(seed) = apply_seen(&mut items, saved, user, now_ms())
            && let Err(e) = blocking(store, move |s| s.set_setting(SEEN_KEY, &seed)).await
        {
            tracing::warn!("{SEEN_KEY} non enregistré : {e}");
        }
    }
    if let Ok(mut keys) = feed.0.lock() {
        *keys = items.iter().map(|i| item_key(&i.item)).collect();
    }
    let unseen = items.iter().filter(|i| !i.seen).count();
    Ok((items, unseen))
}

async fn user_id(tidal: &TidalClient) -> Option<u64> {
    Some(tidal.current_session().await.ok()??.user_id)
}

fn emit_unseen(app: &AppHandle, unseen: usize) {
    if let Err(e) = app.emit(UNSEEN_EVENT, unseen) {
        tracing::warn!("émission de {UNSEEN_EVENT} impossible : {e}");
    }
}

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Clé stable d'un élément (un album et un mix peuvent partager un identifiant).
pub(super) fn item_key(item: &MediaItem) -> String {
    match item {
        MediaItem::Album(a) => format!("album-{}", a.id),
        MediaItem::Mix(m) => format!("mix-{}", m.id),
        MediaItem::Artist(a) => format!("artist-{}", a.id),
        MediaItem::Playlist(p) => format!("playlist-{}", p.uuid),
    }
}

/// Applique l'état local à `items`. Sans état pour ce compte (première lecture, autre
/// compte), part de l'état de TIDAL et considère comme vu tout ce qui a plus de
/// `FIRST_RUN_DAYS` jours (ou pas de date) : renvoie alors l'ensemble à enregistrer.
fn apply_seen(
    items: &mut [FeedItem],
    saved: Option<SeenSet>,
    user: u64,
    now_ms: i64,
) -> Option<SeenSet> {
    match saved {
        Some(saved) if saved.user == user => {
            let keys: HashSet<&str> = saved.keys.iter().map(String::as_str).collect();
            for entry in items.iter_mut() {
                entry.seen = keys.contains(item_key(&entry.item).as_str());
            }
            None
        }
        _ => {
            let old = now_ms - FIRST_RUN_DAYS * DAY_MS;
            let mut keys = Vec::new();
            for entry in items.iter_mut() {
                entry.seen = entry.seen || entry.occurred_at.is_none_or(|at| at < old);
                if entry.seen {
                    keys.push(item_key(&entry.item));
                }
            }
            Some(SeenSet { user, keys })
        }
    }
}

/// Ajoute `keys` en tête de l'ensemble du compte (remplacé s'il était à un autre compte),
/// sans doublon, en gardant les `MAX_SEEN` plus récentes.
pub(super) fn mark_all(saved: Option<SeenSet>, user: u64, keys: Vec<String>) -> SeenSet {
    let old = saved
        .filter(|s| s.user == user)
        .map(|s| s.keys)
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let keys = keys
        .into_iter()
        .chain(old)
        .filter(|k| seen.insert(k.clone()))
        .take(MAX_SEEN)
        .collect();
    SeenSet { user, keys }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumSummary, MixSummary};

    const NOW: i64 = 1_000 * DAY_MS;

    fn album(id: u64, at: Option<i64>, seen: bool) -> FeedItem {
        FeedItem {
            occurred_at: at,
            seen,
            item: MediaItem::Album(AlbumSummary {
                id,
                title: String::new(),
                artists: Vec::new(),
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

    fn mix(id: &str) -> FeedItem {
        FeedItem {
            occurred_at: Some(NOW),
            seen: false,
            item: MediaItem::Mix(MixSummary {
                id: id.to_owned(),
                title: String::new(),
                subtitle: None,
                image_url: None,
                mix_type: None,
            }),
        }
    }

    fn seen(items: &[FeedItem]) -> Vec<bool> {
        items.iter().map(|i| i.seen).collect()
    }

    fn set(user: u64, keys: &[&str]) -> SeenSet {
        SeenSet {
            user,
            keys: keys.iter().map(|k| (*k).to_owned()).collect(),
        }
    }

    #[test]
    fn first_run_starts_from_tidal_and_age() {
        let mut items = vec![
            album(1, Some(NOW - DAY_MS), false),
            album(2, Some(NOW - DAY_MS), true),
            album(3, Some(NOW - 15 * DAY_MS), false),
            album(4, None, false),
        ];
        let seed = apply_seen(&mut items, None, 7, NOW);
        assert_eq!(seen(&items), [false, true, true, true]);
        assert_eq!(seed, Some(set(7, &["album-2", "album-3", "album-4"])));
    }

    #[test]
    fn saved_state_replaces_tidal_state() {
        let mut items = vec![
            album(1, Some(NOW), true),
            album(2, Some(NOW), false),
            mix("1"),
        ];
        let seed = apply_seen(&mut items, Some(set(7, &["album-2"])), 7, NOW);
        assert_eq!(seed, None);
        // Un mix et un album de même id ne se confondent pas.
        assert_eq!(seen(&items), [false, true, false]);
    }

    #[test]
    fn other_account_starts_over() {
        let mut items = vec![album(1, Some(NOW), true)];
        let seed = apply_seen(&mut items, Some(set(8, &[])), 7, NOW);
        assert_eq!(seed, Some(set(7, &["album-1"])));
        assert_eq!(seen(&items), [true]);
    }

    #[test]
    fn mark_all_prepends_without_duplicates() {
        let keys = vec!["a".to_owned(), "b".to_owned()];
        assert_eq!(
            mark_all(Some(set(7, &["b", "c"])), 7, keys.clone()),
            set(7, &["a", "b", "c"])
        );
        assert_eq!(mark_all(Some(set(8, &["c"])), 7, keys), set(7, &["a", "b"]));
        let many: Vec<String> = (0..MAX_SEEN + 5).map(|i| i.to_string()).collect();
        assert_eq!(mark_all(None, 7, many).keys.len(), MAX_SEEN);
    }
}
