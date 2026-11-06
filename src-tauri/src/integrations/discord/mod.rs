//! Présence Discord (#215, bureau seulement) : « Écoute Seaglass » avec le titre en cours.
//!
//! Désactivée par défaut (réglage `UiSettings::discord`). Une tâche se connecte à
//! l'application Discord du poste quand le réglage est actif, réessaie toutes les 15 s si
//! Discord est fermé, et n'envoie l'activité que si elle change (titre, lecture, saut).

mod activity;
mod ipc;

use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;
use tokio::time::Instant;

use super::NowPlaying;
use activity::Activity;

/// Délai avant de réessayer quand Discord est fermé ou a coupé la connexion.
const RETRY: Duration = Duration::from_secs(15);
/// Discord accepte 5 mises à jour par 20 s ; au-delà, la dernière attend son tour.
const MIN_INTERVAL: Duration = Duration::from_secs(4);

static ENABLED: LazyLock<watch::Sender<bool>> = LazyLock::new(|| watch::Sender::new(false));

/// Réglage de l'utilisateur, appliqué au démarrage et à chaque `set_ui_settings`.
pub fn set_enabled(on: bool) {
    ENABLED.send_if_modified(|v| {
        let changed = *v != on;
        *v = on;
        changed
    });
}

/// Identifiant de l'application Discord : variable `DISCORD_CLIENT_ID` au lancement, sinon
/// celle du `.env` compilée par `build.rs`. Absent : le réglage n'est pas proposé.
pub fn client_id() -> Option<String> {
    std::env::var("DISCORD_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("SEAGLASS_DISCORD_CLIENT_ID").map(str::to_owned))
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
}

pub struct DiscordHandle {
    tx: watch::Sender<Option<NowPlaying>>,
}

impl DiscordHandle {
    pub fn update(&self, state: NowPlaying) {
        self.tx.send_replace(Some(state));
    }
}

pub fn start(client_id: String) -> DiscordHandle {
    let (tx, rx) = watch::channel(None);
    tauri::async_runtime::spawn(run(client_id, rx, ENABLED.subscribe()));
    DiscordHandle { tx }
}

async fn run(
    client_id: String,
    mut playing: watch::Receiver<Option<NowPlaying>>,
    mut enabled: watch::Receiver<bool>,
) {
    loop {
        while !*enabled.borrow_and_update() {
            if enabled.changed().await.is_err() {
                return;
            }
        }
        match ipc::open(&client_id).await {
            Ok(mut conn) => {
                tracing::info!("Discord connecté");
                match session(&mut conn, &mut playing, &mut enabled).await {
                    End::Disabled => continue,
                    End::Stopped => return,
                    End::Lost(e) => tracing::info!("Discord : {e}"),
                }
            }
            Err(e) => tracing::debug!("{e}"),
        }
        tokio::select! {
            _ = tokio::time::sleep(RETRY) => {}
            r = enabled.changed() => if r.is_err() { return; },
        }
    }
}

enum End {
    Disabled,
    Stopped,
    Lost(ipc::IpcError),
}

async fn session<S>(
    conn: &mut ipc::Connection<S>,
    playing: &mut watch::Receiver<Option<NowPlaying>>,
    enabled: &mut watch::Receiver<bool>,
) -> End
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + 'static,
{
    let mut sent: Option<Activity> = None;
    let mut last_send: Option<Instant> = None;
    let mut wake: Option<Instant> = None;
    let mut dirty = true;
    loop {
        if dirty {
            dirty = false;
            let next = activity::desired(
                playing.borrow_and_update().as_ref(),
                sent.as_ref(),
                now_ms(),
            );
            if activity::needs_update(sent.as_ref(), next.as_ref()) {
                let ready_at = last_send.map(|t| t + MIN_INTERVAL);
                if ready_at.is_none_or(|t| t <= Instant::now()) {
                    if let Err(e) = conn
                        .set_activity(next.as_ref().map(Activity::to_json))
                        .await
                    {
                        return End::Lost(e);
                    }
                    sent = next;
                    last_send = Some(Instant::now());
                    wake = None;
                } else {
                    wake = ready_at;
                }
            }
        }
        tokio::select! {
            r = playing.changed() => {
                if r.is_err() {
                    return End::Stopped;
                }
                dirty = true;
            }
            r = enabled.changed() => {
                if r.is_err() {
                    return End::Stopped;
                }
                if !*enabled.borrow_and_update() {
                    return End::Disabled;
                }
            }
            frame = conn.recv() => {
                let Some(frame) = frame else {
                    return End::Lost(ipc::IpcError::Closed);
                };
                if let Err(e) = conn.handle(frame).await {
                    return End::Lost(e);
                }
            }
            _ = tokio::time::sleep_until(wake.unwrap_or_else(Instant::now)), if wake.is_some() => {
                dirty = true;
            }
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
