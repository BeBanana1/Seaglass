//! Envoi des écoutes à ListenBrainz (#431), facultatif, bureau et Android.
//!
//! Le jeton de l'utilisateur, vérifié par `validate-token`, est gardé dans le keyring avec
//! le nom de son compte : sa présence est le seul interrupteur. Le lecteur annonce le début
//! d'un titre (`now_playing`, sans file : il ne vaut que sur le moment) et chaque écoute
//! comptée (`listened`, règle de `player::listen`), mise en file dans SQLite
//! (`store::scrobbles`). Une tâche vide la file : une écoute seule part en `single`,
//! plusieurs en `import`. Sans réseau ou service indisponible, elle réessaie avec une
//! attente croissante ; un jeton refusé suspend l'envoi jusqu'au prochain jeton, sans perdre
//! la file.

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
/// Écoutes par requête `import` (ListenBrainz en accepte 1 000).
const BATCH: usize = 100;
const MIN_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);

/// Compte connecté. Pas de `Debug` : le jeton n'apparaît dans aucun journal.
#[derive(Clone)]
struct Account {
    token: Token,
    user: String,
    /// ListenBrainz a refusé le jeton à un envoi (révoqué depuis).
    refused: bool,
}

/// Forme gardée dans le keyring.
#[derive(Serialize, Deserialize)]
struct Saved {
    token: String,
    user: String,
}

static ACCOUNT: LazyLock<watch::Sender<Option<Account>>> =
    LazyLock::new(|| watch::Sender::new(None));
/// Une écoute vient d'entrer dans la file.
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
static STORE: OnceLock<Arc<Store>> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();

/// État affiché dans les réglages.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Compte connecté, `None` si l'envoi est désactivé.
    pub user: Option<String>,
    pub refused: bool,
    /// Écoutes pas encore envoyées.
    pub pending: u32,
}

/// Charge le compte gardé et lance la tâche d'envoi. Après l'installation du keystore
/// d'Android.
pub fn start(app: AppHandle, store: Option<Arc<Store>>) {
    let _ = APP.set(app);
    let Some(store) = store else { return };
    let _ = STORE.set(Arc::clone(&store));
    tauri::async_runtime::spawn(async move {
        match blocking(load).await {
            Ok(account) => {
                ACCOUNT.send_replace(account);
            }
            Err(e) => tracing::warn!("ListenBrainz : compte illisible dans le keyring : {e}"),
        }
        run(store).await;
    });
}

/// Le lecteur commence un titre : annoncé sans être gardé, sans attendre la réponse.
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

/// Écoute comptée, commencée à `started_at` (secondes Unix). Bloquant (SQLite) : appelé
/// depuis `spawn_blocking`. Rien n'est gardé sans compte.
pub fn listened(store: &Store, track: &Track, started_at: i64) {
    if ACCOUNT.borrow().is_none() {
        return;
    }
    let listen = payload::listen(track, Some(started_at)).to_string();
    match store.queue_scrobble(&listen) {
        Ok(()) => WAKE.notify_one(),
        Err(e) => tracing::warn!("ListenBrainz : écoute non mise en file : {e}"),
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

/// Vérifie le jeton collé, le garde et (re)lance l'envoi.
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
    tracing::info!("ListenBrainz connecté");
    ACCOUNT.send_replace(Some(Account {
        token,
        user,
        refused: false,
    }));
    WAKE.notify_one();
    Ok(status().await)
}

/// Oublie le jeton et vide la file : les écoutes en attente ne partiront pas sur un autre
/// compte connecté ensuite.
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
    tracing::info!("ListenBrainz déconnecté");
    Ok(status().await)
}

fn active_token() -> Option<Token> {
    ACCOUNT
        .borrow()
        .as_ref()
        .filter(|a| !a.refused)
        .map(|a| a.token.clone())
}

/// Le jeton `token` a été refusé ; sans effet si l'utilisateur en a collé un autre depuis.
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
    tracing::warn!("ListenBrainz : jeton refusé, envoi suspendu");
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
            tracing::warn!("ListenBrainz : client HTTP indisponible : {e}");
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
                tracing::warn!("ListenBrainz : file illisible : {e}");
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
            // Lot refusé : on reprend une écoute à la fois pour isoler la fautive.
            Err(SendError::Rejected) if ids.len() > 1 => batch = 1,
            Err(SendError::Rejected) => {
                tracing::warn!("ListenBrainz : écoute refusée, retirée de la file");
                drop_sent(&store, ids).await;
            }
        }
    }
}

async fn drop_sent(store: &Arc<Store>, ids: Vec<i64>) {
    let store = Arc::clone(store);
    if let Err(e) = blocking(move || store.drop_scrobbles(&ids).map_err(err)).await {
        tracing::warn!("ListenBrainz : écoutes envoyées non retirées : {e}");
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
