//! Listening history synced with the linked computer (#430).
//!
//! Same link and same server as the handoff: the phone always acts. Each call (`POST
//! api/plays/sync`) sends a batch of listens from here (since the last send) and asks for the PC's
//! since the last cursor received; the PC merges and answers without keeping anything. The merge
//! rules and the cursors are in `store/sync.rs`.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::Link;
use crate::commands::AppState;
use crate::commands::settings::pins::is_uuid;
use crate::i18n::tr;
use crate::store::{Outgoing, PlayBatch, SYNC_BATCH, Store, SyncState};

/// Event emitted when received listens were added (their number as the payload).
pub const EVENT: &str = "history-synced";
/// Calls at most per sync: 100,000 listens each way.
const MAX_ROUNDS: usize = 100;
/// Automatic syncs (launch, statistics, handoff) are at least this far apart.
const MIN_INTERVAL_MS: i64 = 30_000;
/// At launch, let the app start first.
const LAUNCH_DELAY: Duration = Duration::from_secs(5);

/// Call from the phone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    /// Phone identifier: the PC does not send it back its own listens.
    pub device: String,
    /// Last PC id already received.
    pub since: i64,
    pub plays: PlayBatch,
}

/// Answer from the PC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    /// PC identifier: if it changes, the phone starts over from scratch.
    pub device: String,
    /// Next `since`.
    pub cursor: i64,
    /// The PC has more listens after this batch.
    pub more: bool,
    pub plays: PlayBatch,
}

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl SyncRequest {
    /// Call received by the PC, checked: `None` if it is inconsistent.
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

/// PC side: merges the phone's batch and answers with the next listens. Also returns the number of
/// listens added here.
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

/// `Store` on a blocking thread.
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

/// Phone side: exchanges until both sides are up to date. Returns the number of listens received
/// and added here.
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
            // First exchange, or PC reinstalled: start over (the PC removes duplicates from the
            // batch sent).
            if state.peer.is_some() {
                tracing::info!("History: linked computer changed, sync restarted from scratch");
            }
            state = SyncState {
                peer: Some(response.device),
                ..SyncState::default()
            };
            continue;
        }
        let plays = response.plays.received(now_ms(), is_uuid).ok_or_else(|| {
            tr(
                "réponse de l'ordinateur illisible",
                "unreadable answer from the computer",
            )
            .to_owned()
        })?;
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
    tracing::warn!("History: sync stopped after {MAX_ROUNDS} exchanges");
    Ok(received)
}

/// One sync at a time.
static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Syncs with the linked computer if there is one, and tells the UI if listens arrived. `force`:
/// even if the last sync is very recent (button in Settings). Returns the date of the last
/// successful sync, `None` without a linked computer.
pub async fn sync_now(app: &AppHandle, force: bool) -> Result<Option<i64>, String> {
    let Some(store) = app.state::<AppState>().store.clone() else {
        return Ok(None);
    };
    let Some(mut link) = super::load().await? else {
        return Ok(None);
    };
    // Link from before #430 or PC without a tunnel until now: ask for the relay route (local
    // network only; without an answer, the next sync will try again).
    if link.relay().is_none()
        && let Ok(Some(route)) = link.fetch_relay().await
    {
        super::save_relay(Some(route.clone())).await?;
        link = link.with_relay(Some(route));
    }
    let Ok(_guard) = RUNNING.try_lock() else {
        // A sync is already running: it is enough.
        return Ok(blocking(&store, Store::sync_state).await?.synced_at);
    };
    let last = blocking(&store, Store::sync_state).await?.synced_at;
    if !force && last.is_some_and(|at| now_ms() - at < MIN_INTERVAL_MS) {
        return Ok(last);
    }
    let received = sync(&link, &store).await?;
    if received > 0 {
        tracing::info!("History: {received} listens received from the computer");
        let _ = app.emit(EVENT, received);
    }
    Ok(blocking(&store, Store::sync_state).await?.synced_at)
}

/// Silent sync in the background (launch, after a handoff): a failure (PC off, elsewhere) is only
/// logged.
pub fn spawn(app: &AppHandle, delay: Option<Duration>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        if let Err(e) = sync_now(&app, false).await {
            tracing::info!("History: sync failed ({e})");
        }
    });
}

/// At launch, if a computer is linked.
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
