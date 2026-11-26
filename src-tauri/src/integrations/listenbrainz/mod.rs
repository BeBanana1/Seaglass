//! Listen submission to ListenBrainz (#431), optional, desktop and Android.
//!
//! The user's token, checked by `validate-token`, is kept in the keyring with its account name: its
//! presence is the only switch. The player announces the start of a track (`now_playing`, not
//! queued: it only matters at the time) and every counted listen (`listened`, rule of
//! `player::listen`), queued in SQLite (`store::scrobbles`). A task empties the queue: a single
//! listen goes as `single`, several as `import`. Without a network or with the service unavailable,
//! it retries with a growing wait; a refused token suspends the submission until the next token,
//! without losing the queue.

mod api;
mod payload;

use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Notify, watch};

use crate::i18n::tr;
use crate::store::Store;
use crate::tidal::models::Track;
use api::{SendError, Token};
use payload::ListenType;

const KEYRING_SERVICE: &str = "seaglass";
const KEYRING_USER: &str = "listenbrainz";
/// Listens per `import` request (ListenBrainz accepts 1,000).
const BATCH: usize = 100;
const MIN_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);

/// Connected account. No `Debug`: the token appears in no log.
#[derive(Clone)]
struct Account {
    token: Token,
    user: String,
    /// ListenBrainz refused the token on a submission (revoked since).
    refused: bool,
}

/// Form kept in the keyring.
#[derive(Serialize, Deserialize)]
struct Saved {
    token: String,
    user: String,
}

static ACCOUNT: LazyLock<watch::Sender<Option<Account>>> =
    LazyLock::new(|| watch::Sender::new(None));
/// A listen just entered the queue.
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
static STORE: OnceLock<Arc<Store>> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();

/// State shown in Settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Connected account, `None` if submission is off.
    pub user: Option<String>,
    pub refused: bool,
    /// Listens not sent yet.
    pub pending: u32,
}

/// Loads the stored account and starts the submission task. After the Android keystore is
/// installed.
pub fn start(app: AppHandle, store: Option<Arc<Store>>) {
    let _ = APP.set(app);
    let Some(store) = store else { return };
    let _ = STORE.set(Arc::clone(&store));
    tauri::async_runtime::spawn(async move {
        match blocking(load).await {
            Ok(account) => {
                ACCOUNT.send_replace(account);
            }
            Err(e) => tracing::warn!("ListenBrainz: unreadable account in the keyring: {e}"),
        }
        run(store).await;
    });
}

/// The player starts a track: announced without being kept, without waiting for the reply.
pub fn now_playing(track: &Track) {
    let Some(token) = active_token() else { return };
    let listen = payload::listen(track, None);
    tauri::async_runtime::spawn(async move {
        let Ok(client) = api::client() else { return };
        let sent = api::submit(&client, &token, ListenType::PlayingNow, vec![listen]).await;
        if let Err(SendError::Unauthorized) = sent {
            refuse(&token);
        }
    });
}

/// Counted listen, started at `started_at` (Unix seconds). Blocking (SQLite): called from
/// `spawn_blocking`. Nothing is kept without an account.
pub fn listened(store: &Store, track: &Track, started_at: i64) {
    if ACCOUNT.borrow().is_none() {
        return;
    }
    let listen = payload::listen(track, Some(started_at)).to_string();
    match store.queue_scrobble(&listen) {
        Ok(()) => WAKE.notify_one(),
        Err(e) => tracing::warn!("ListenBrainz: listen not queued: {e}"),
    }
}

pub async fn status() -> Status {
    let (user, refused) = match &*ACCOUNT.borrow() {
        Some(a) => (Some(a.user.clone()), a.refused),
        None => (None, false),
    };
    let pending = match STORE.get().cloned() {
        Some(store) if user.is_some() => blocking(move || store.scrobble_count().map_err(err))
            .await
            .unwrap_or(0),
        _ => 0,
    };
    Status {
        user,
        refused,
        pending,
    }
}

/// Checks the pasted token, keeps it and (re)starts the submission.
pub async fn connect(raw: String) -> Result<Status, String> {
    let token = Token::parse(&raw).ok_or_else(|| {
        tr(
            "jeton ListenBrainz invalide : collez-le tel quel",
            "invalid ListenBrainz token: paste it as is",
        )
        .to_owned()
    })?;
    if STORE.get().is_none() {
        return Err(tr(
            "envoi impossible : la base de l'app est indisponible",
            "cannot send listens: the app database is unavailable",
        )
        .to_owned());
    }
    let user = api::validate(&api::client()?, &token).await?;
    let saved = Saved {
        token: token.as_str().to_owned(),
        user: user.clone(),
    };
    blocking(move || {
        let json = serde_json::to_string(&saved).map_err(err)?;
        entry()?.set_password(&json).map_err(err)
    })
    .await?;
    tracing::info!("ListenBrainz connected");
    ACCOUNT.send_replace(Some(Account {
        token,
        user,
        refused: false,
    }));
    WAKE.notify_one();
    Ok(status().await)
}

/// Forgets the token and empties the queue: pending listens will not go to another account
/// connected afterwards.
pub async fn disconnect() -> Result<Status, String> {
    blocking(|| match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(err(e)),
    })
    .await?;
    ACCOUNT.send_replace(None);
    if let Some(store) = STORE.get().cloned() {
        blocking(move || store.clear_scrobbles().map_err(err)).await?;
    }
    tracing::info!("ListenBrainz disconnected");
    Ok(status().await)
}

fn active_token() -> Option<Token> {
    ACCOUNT
        .borrow()
        .as_ref()
        .filter(|a| !a.refused)
        .map(|a| a.token.clone())
}

/// The token `token` was refused; no effect if the user pasted another one since.
fn refuse(token: &Token) {
    let changed = ACCOUNT.send_if_modified(|account| match account {
        Some(a) if a.token == *token && !a.refused => {
            a.refused = true;
            true
        }
        _ => false,
    });
    if !changed {
        return;
    }
    tracing::warn!("ListenBrainz: token refused, submission suspended");
    if let Some(app) = APP.get() {
        #[derive(Clone, Serialize)]
        struct Toast {
            level: &'static str,
            message: &'static str,
        }
        let _ = app.emit(
            "toast",
            Toast {
                level: "error",
                message: tr(
                    "ListenBrainz refuse le jeton : reconnectez-vous dans les réglages",
                    "ListenBrainz refuses the token: reconnect in the settings",
                ),
            },
        );
    }
}

async fn run(store: Arc<Store>) {
    let client = match api::client() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("ListenBrainz: HTTP client unavailable: {e}");
            return;
        }
    };
    let mut account = ACCOUNT.subscribe();
    let mut batch = BATCH;
    let mut backoff = MIN_BACKOFF;
    loop {
        let token = account
            .borrow_and_update()
            .as_ref()
            .filter(|a| !a.refused)
            .map(|a| a.token.clone());
        let Some(token) = token else {
            if account.changed().await.is_err() {
                return;
            }
            continue;
        };
        let read = Arc::clone(&store);
        let pending = match blocking(move || read.pending_scrobbles(batch).map_err(err)).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("ListenBrainz: unreadable queue: {e}");
                tokio::time::sleep(MAX_BACKOFF).await;
                continue;
            }
        };
        if pending.is_empty() {
            tokio::select! {
                () = WAKE.notified() => {}
                changed = account.changed() => if changed.is_err() { return },
            }
            continue;
        }
        let ids: Vec<i64> = pending.iter().map(|s| s.id).collect();
        let listens: Vec<serde_json::Value> = pending
            .iter()
            .filter_map(|s| serde_json::from_str(&s.listen).ok())
            .collect();
        let kind = if listens.len() == 1 {
            ListenType::Single
        } else {
            ListenType::Import
        };
        let outcome = if listens.is_empty() {
            Ok(())
        } else {
            api::submit(&client, &token, kind, listens).await
        };
        match outcome {
            Ok(()) => {
                drop_sent(&store, ids).await;
                batch = BATCH;
                backoff = MIN_BACKOFF;
            }
            Err(SendError::Retry(wait)) => {
                let delay = wait.unwrap_or(backoff).max(Duration::from_secs(1));
                backoff = (backoff * 2).min(MAX_BACKOFF);
                tokio::select! {
                    () = tokio::time::sleep(delay) => {}
                    changed = account.changed() => if changed.is_err() { return },
                }
            }
            Err(SendError::Unauthorized) => refuse(&token),
            // Batch refused: send one listen at a time to isolate the faulty one.
            Err(SendError::Rejected) if ids.len() > 1 => batch = 1,
            Err(SendError::Rejected) => {
                tracing::warn!("ListenBrainz: listen refused, removed from the queue");
                drop_sent(&store, ids).await;
            }
        }
    }
}

async fn drop_sent(store: &Arc<Store>, ids: Vec<i64>) {
    let store = Arc::clone(store);
    if let Err(e) = blocking(move || store.drop_scrobbles(&ids).map_err(err)).await {
        tracing::warn!("ListenBrainz: sent listens not removed: {e}");
    }
}

fn load() -> Result<Option<Account>, String> {
    let json = match entry()?.get_password() {
        Ok(json) => json,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    let Ok(saved) = serde_json::from_str::<Saved>(&json) else {
        return Ok(None);
    };
    Ok(Token::parse(&saved.token).map(|token| Account {
        token,
        user: saved.user,
        refused: false,
    }))
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(err)
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(err)?
}
