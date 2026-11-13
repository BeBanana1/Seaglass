//! Historique d'écoute synchronisé avec l'ordinateur lié (#430).
//!
//! Même lien et même serveur que le transfert : le téléphone agit toujours. Chaque appel
//! (`POST api/plays/sync`) envoie un lot d'écoutes d'ici (depuis le dernier envoi) et
//! demande celles du PC depuis le dernier curseur reçu ; le PC fusionne et répond sans rien
//! retenir. Les règles de fusion et les curseurs sont dans `store/sync.rs`.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::Link;
use crate::commands::AppState;
use crate::commands::settings::pins::is_uuid;
use crate::store::{Outgoing, PlayBatch, SYNC_BATCH, Store, SyncState};

/// Événement émis quand des écoutes reçues ont été ajoutées (nombre en charge utile).
pub const EVENT: &str = "history-synced";
/// Appels au plus par synchro : 100 000 écoutes dans chaque sens.
const MAX_ROUNDS: usize = 100;
/// Synchros automatiques (lancement, statistiques, transfert) espacées d'au moins.
const MIN_INTERVAL_MS: i64 = 30_000;
/// Au lancement, laisser l'app démarrer d'abord.
const LAUNCH_DELAY: Duration = Duration::from_secs(5);

/// Appel du téléphone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    /// Identifiant du téléphone : le PC ne lui renvoie pas ses propres écoutes.
    pub device: String,
    /// Dernier id du PC déjà reçu.
    pub since: i64,
    pub plays: PlayBatch,
}

/// Réponse du PC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    /// Identifiant du PC : s'il change, le téléphone reprend tout de zéro.
    pub device: String,
    /// Prochain `since`.
    pub cursor: i64,
    /// Le PC a encore des écoutes après ce lot.
    pub more: bool,
    pub plays: PlayBatch,
}

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl SyncRequest {
    /// Appel reçu par le PC, vérifié : `None` s'il est incohérent.
    pub fn received(self) -> Option<Self> {
        if !is_uuid(&self.device) || self.since < 0 {
            return None;
        }
        Some(Self {
            plays: self.plays.received(now_ms(), is_uuid)?,
            ..self
        })
    }
}

/// Côté PC : fusionne le lot du téléphone et répond avec les écoutes suivantes. Renvoie
/// aussi le nombre d'écoutes ajoutées ici.
pub fn answer(store: &Store, request: &SyncRequest) -> crate::store::Result<(SyncResponse, usize)> {
    let own = store.device_id()?;
    let added = store.merge_plays(&request.plays, &own)?;
    let outbox =
        store.outgoing_plays(request.since, Outgoing::Except(&request.device), SYNC_BATCH)?;
    Ok((
        SyncResponse {
            device: own,
            cursor: outbox.cursor,
            more: outbox.more,
            plays: outbox.batch,
        },
        added,
    ))
}

/// `Store` sur un thread bloquant.
async fn blocking<T: Send + 'static>(
    store: &Arc<Store>,
    f: impl FnOnce(&Store) -> crate::store::Result<T> + Send + 'static,
) -> Result<T, String> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || f(&store))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Côté téléphone : échange jusqu'à ce que les deux côtés soient à jour. Renvoie le nombre
/// d'écoutes reçues et ajoutées ici.
pub async fn sync(link: &Link, store: &Arc<Store>) -> Result<usize, String> {
    let own = blocking(store, Store::device_id).await?;
    let mut state = blocking(store, Store::sync_state).await?;
    let mut received = 0;
    for _ in 0..MAX_ROUNDS {
        let pushed = state.pushed;
        let outbox = blocking(store, move |s| {
            s.outgoing_plays(pushed, Outgoing::Own, SYNC_BATCH)
        })
        .await?;
        let request = SyncRequest {
            device: own.clone(),
            since: state.pulled,
            plays: outbox.batch,
        };
        let response = link.sync_plays(&request).await?;
        if state.peer.as_deref() != Some(response.device.as_str()) {
            // Premier échange, ou PC réinstallé : tout reprendre (le lot envoyé est
            // dédoublonné par le PC).
            if state.peer.is_some() {
                tracing::info!("Historique : ordinateur lié changé, synchro reprise de zéro");
            }
            state = SyncState {
                peer: Some(response.device),
                ..SyncState::default()
            };
            continue;
        }
        let plays = response
            .plays
            .received(now_ms(), is_uuid)
            .ok_or_else(|| "réponse de l'ordinateur illisible".to_owned())?;
        let own_id = own.clone();
        received += blocking(store, move |s| s.merge_plays(&plays, &own_id)).await?;
        state.pushed = outbox.cursor;
        state.pulled = response.cursor;
        let saved = state.clone();
        blocking(store, move |s| s.set_sync_state(&saved)).await?;
        if !outbox.more && !response.more {
            state.synced_at = Some(now_ms());
            let saved = state.clone();
            blocking(store, move |s| s.set_sync_state(&saved)).await?;
            return Ok(received);
        }
    }
    tracing::warn!("Historique : synchro interrompue après {MAX_ROUNDS} échanges");
    Ok(received)
}

/// Une synchro à la fois.
static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Synchronise avec l'ordinateur lié s'il y en a un, et prévient l'UI si des écoutes sont
/// arrivées. `force` : même si la dernière synchro est toute récente (bouton de Réglages).
/// Renvoie la date de la dernière synchro réussie, `None` sans ordinateur lié.
pub async fn sync_now(app: &AppHandle, force: bool) -> Result<Option<i64>, String> {
    let Some(store) = app.state::<AppState>().store.clone() else {
        return Ok(None);
    };
    let Some(mut link) = super::load().await? else {
        return Ok(None);
    };
    // Lien d'avant #430 ou PC sans tunnel jusque-là : demander le chemin par le relais
    // (réseau local seulement ; sans réponse, on réessaiera à la prochaine synchro).
    if link.relay().is_none()
        && let Ok(Some(route)) = link.fetch_relay().await
    {
        super::save_relay(Some(route.clone())).await?;
        link = link.with_relay(Some(route));
    }
    let Ok(_guard) = RUNNING.try_lock() else {
        // Une synchro tourne déjà : elle suffit.
        return Ok(blocking(&store, Store::sync_state).await?.synced_at);
    };
    let last = blocking(&store, Store::sync_state).await?.synced_at;
    if !force && last.is_some_and(|at| now_ms() - at < MIN_INTERVAL_MS) {
        return Ok(last);
    }
    let received = sync(&link, &store).await?;
    if received > 0 {
        tracing::info!("Historique : {received} écoutes reçues de l'ordinateur");
        let _ = app.emit(EVENT, received);
    }
    Ok(blocking(&store, Store::sync_state).await?.synced_at)
}

/// Synchro silencieuse en tâche de fond (lancement, après un transfert) : un échec (PC
/// éteint, ailleurs) n'est que journalisé.
pub fn spawn(app: &AppHandle, delay: Option<Duration>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        if let Err(e) = sync_now(&app, false).await {
            tracing::info!("Historique : synchro impossible ({e})");
        }
    });
}

/// Au lancement, si un ordinateur est lié.
pub fn spawn_at_launch(app: &AppHandle) {
    spawn(app, Some(LAUNCH_DELAY));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_checked() {
        let request = SyncRequest {
            device: "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
            since: 0,
            plays: PlayBatch::default(),
        };
        assert!(request.clone().received().is_some());
        assert!(
            SyncRequest {
                device: "x".into(),
                ..request.clone()
            }
            .received()
            .is_none()
        );
        assert!(
            SyncRequest {
                since: -1,
                ..request
            }
            .received()
            .is_none()
        );
    }
}
