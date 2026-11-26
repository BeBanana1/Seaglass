//! App updates (`tauri-plugin-updater` plugin).
//!
//! Check at startup then every 6 h. A newer version is downloaded
//! in the background (signature checked by the plugin), then the UI receives `update-ready`.
//! The user chooses "Install and restart" (`install_update`) or "Later":
//! the installation then happens when the app closes. Nothing is ever installed
//! without an action from them, and playback is never cut off on its own.
//!
//! `check_for_updates` (Settings button) runs the same check on demand. It
//! answers as soon as the manifest is read (#187): the installer download (about forty
//! MB) goes on in the background and ends with `update-ready`.
//!
//! Linux (#314): only from an AppImage (`APPIMAGE`, validated by Tauri). The plugin
//! rewrites that file, then the app restarts itself. Without an AppImage, it would take
//! the current executable as its target and replace the bare binary with an AppImage: updates
//! are disabled there (`supported`).
//!
//! Address (#452): `latest.json` of the compiled folder (`servers::update_base`), absent from
//! `tauri.conf.json`. Public key (#489): compiled too (`servers::updater_pubkey`,
//! passed to the plugin in `lib.rs`). Without an address or a key, nothing is checked. The periodic
//! check follows the "Automatic updates" switch; the button, always.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::i18n::tr;
use crate::mpv::process::ProcessGuard;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// First check a little after startup, so as not to slow down the opening.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
/// Override of the manifest URL (tests, another channel). In debug, without this variable,
/// no check is made.
const ENDPOINT_ENV: &str = "SEAGLASS_UPDATE_URL";

/// Manifest to read: `SEAGLASS_UPDATE_URL` at launch, otherwise `latest.json` of the
/// compiled folder (#452). `None`: app compiled without updates (neither address nor public key).
fn endpoint() -> Option<String> {
    crate::servers::updater_pubkey()?;
    std::env::var(ENDPOINT_ENV)
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| crate::servers::update_base().map(|base| format!("{base}/latest.json")))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Release notes (free text from the manifest).
    pub notes: Option<String>,
    /// True while the installer is downloading: `update-ready` will follow (#187).
    pub downloading: bool,
}

impl UpdateInfo {
    fn of(update: &Update, downloading: bool) -> Self {
        Self {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
            downloading,
        }
    }
}

struct Pending {
    update: Update,
    bytes: Vec<u8>,
}

/// Update downloaded and verified, waiting for installation.
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Pending>>,
    /// Version being downloaded: never two downloads at once.
    downloading: Mutex<Option<UpdateInfo>>,
    /// Serializes the reading of the manifest between the background task and the button.
    checking: tokio::sync::Mutex<()>,
}

impl UpdateState {
    fn info(&self) -> Option<UpdateInfo> {
        let guard = self.pending.lock().ok()?;
        guard.as_ref().map(|p| UpdateInfo::of(&p.update, false))
    }

    fn downloading(&self) -> Option<UpdateInfo> {
        self.downloading.lock().ok()?.clone()
    }

    fn set_downloading(&self, info: Option<UpdateInfo>) {
        if let Ok(mut slot) = self.downloading.lock() {
            *slot = info;
        }
    }

    fn take(&self) -> Option<Pending> {
        self.pending.lock().ok()?.take()
    }

    /// When the app closes: installs the deferred update, without restarting the app.
    pub fn install_on_exit(&self) {
        if let Some(p) = self.take() {
            tracing::info!("installing version {} on close", p.update.version);
            if let Err(e) = p.update.restart_after_install(false).install(&p.bytes) {
                tracing::error!("cannot install the update: {e}");
            }
        }
    }
}

/// True if the app can update itself: Windows (NSIS installer), Linux
/// launched from an AppImage.
pub fn supported(app: &AppHandle) -> bool {
    #[cfg(target_os = "linux")]
    return app.env().appimage.is_some();
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        true
    }
}

pub fn spawn(app: AppHandle, mpv: Arc<ProcessGuard>) {
    if !supported(&app) {
        tracing::info!("automatic updates disabled: app launched outside an AppImage");
        return;
    }
    if cfg!(debug_assertions) && std::env::var(ENDPOINT_ENV).is_err() {
        return;
    }
    let Some(endpoint) = endpoint() else {
        tracing::info!("updates absent: address or public key not compiled");
        return;
    };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            // Setting reread on every round: it can change while the app runs.
            if crate::servers::auto_updates() {
                // The download started by `check` itself emits `update-ready`.
                if let Err(e) = check(&app, &endpoint, &mpv).await {
                    tracing::warn!("update check: {e}");
                }
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Reads the manifest. A newer version, neither ready nor already downloading, is
/// downloaded in the background (then `update-ready`). Returns the state of the update
/// (ready, or downloading), `None` if the app is up to date.
async fn check(
    app: &AppHandle,
    endpoint: &str,
    mpv: &Arc<ProcessGuard>,
) -> Result<Option<UpdateInfo>, String> {
    let state = app.state::<UpdateState>();
    let _checking = state.checking.lock().await;
    let url = endpoint
        .parse()
        .map_err(|e| format!("adresse du manifeste invalide : {e}"))?;
    let builder = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?;
    // The installer closes the app through `std::process::exit`: the queue must be saved
    // (#260) and mpv stopped before.
    let mpv = Arc::clone(mpv);
    let handle = app.clone();
    let updater = builder
        .on_before_exit(move || {
            crate::flush_player(&handle);
            mpv.kill();
        })
        .build()
        .map_err(|e| e.to_string())?;

    let update = match updater.check().await {
        Ok(update) => update,
        // Version published without this platform (Windows-only release, for example):
        // nothing to install here, not an error to show.
        Err(tauri_plugin_updater::Error::TargetNotFound(target)) => {
            tracing::info!("manifest has no {target} entry: no update for this platform");
            None
        }
        Err(e) => return Err(e.to_string()),
    };
    let Some(update) = update else {
        // A version that is ready but was removed from the manifest stays installable.
        return Ok(state.info());
    };
    if let Some(info) = state.info().filter(|i| i.version == update.version) {
        return Ok(Some(info));
    }
    if let Some(info) = state.downloading().filter(|i| i.version == update.version) {
        return Ok(Some(info));
    }
    let info = UpdateInfo::of(&update, true);
    state.set_downloading(Some(info.clone()));
    tracing::info!("version {} available, downloading", update.version);
    tauri::async_runtime::spawn(download(app.clone(), update));
    Ok(Some(info))
}

/// Downloads the installer, then keeps it and emits `update-ready` (`update-failed` with the
/// message on failure: the Settings page waits for the end of a requested download).
async fn download(app: AppHandle, update: Update) {
    let state = app.state::<UpdateState>();
    // `download` checks the signature: bytes not signed by our key are refused.
    let result = update.download(|_, _| {}, || {}).await;
    state.set_downloading(None);
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("download of version {}: {e}", update.version);
            let _ = app.emit("update-failed", e.to_string());
            return;
        }
    };
    tracing::info!(
        "version {} ready ({} bytes, signature verified)",
        update.version,
        bytes.len()
    );
    if let Ok(mut slot) = state.pending.lock() {
        *slot = Some(Pending { update, bytes });
    }
    if let Some(info) = state.info() {
        let _ = app.emit("update-ready", info);
    }
}

/// On-demand check, returned as soon as the manifest is read: the ready update (new
/// or deferred, re-emitted as `update-ready` so that the install offer
/// reappears), downloading (`downloading`, `update-ready` will follow), or `None` if
/// the app is up to date.
#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    mpv: State<'_, Arc<ProcessGuard>>,
) -> Result<Option<UpdateInfo>, String> {
    if !supported(&app) {
        return Err(tr(
            "mises à jour automatiques indisponibles hors de l'AppImage",
            "automatic updates are unavailable outside the AppImage",
        )
        .into());
    }
    let endpoint = endpoint().ok_or_else(|| {
        tr(
            "cette version de l'app n'a pas de serveur de mises à jour",
            "this build of the app has no update server",
        )
        .to_owned()
    })?;
    let info = check(&app, &endpoint, &mpv).await?;
    if let Some(info) = info.as_ref().filter(|i| !i.downloading) {
        let _ = app.emit("update-ready", info);
    }
    Ok(info)
}

/// True if Settings can offer to check for updates (see `supported`).
#[tauri::command]
pub fn updates_available(app: AppHandle) -> bool {
    supported(&app)
}

#[tauri::command]
pub fn update_status(state: State<'_, UpdateState>) -> Option<UpdateInfo> {
    state.info()
}

/// Installs right away. Windows: the installer closes the app then restarts it. Linux:
/// the AppImage is replaced, then the app restarts (queue saved and mpv stopped by
/// `RunEvent::Exit`, as on close).
#[tauri::command]
pub fn install_update(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    let pending = state
        .take()
        .ok_or_else(|| "aucune mise à jour prête".to_owned())?;
    if let Err(e) = pending.update.install(&pending.bytes) {
        let message = install_error(&app, e);
        // Kept: after moving the AppImage, a new attempt downloads nothing again.
        if let Ok(mut slot) = state.pending.lock() {
            *slot = Some(pending);
        }
        return Err(message);
    }
    #[cfg(target_os = "linux")]
    app.request_restart();
    Ok(())
}

/// Message of an installation failure. Linux: an AppImage stored in a folder without
/// write permission (`/opt`, read-only USB stick…) cannot be replaced.
fn install_error(app: &AppHandle, e: tauri_plugin_updater::Error) -> String {
    #[cfg(target_os = "linux")]
    if let tauri_plugin_updater::Error::Io(io) = &e
        && io.kind() == std::io::ErrorKind::PermissionDenied
    {
        let dir = app
            .env()
            .appimage
            .as_deref()
            .and_then(|p| std::path::Path::new(p).parent())
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        return crate::i18n::trf!(
            "impossible de remplacer l'AppImage : le dossier {} n'est pas modifiable. Télécharger la nouvelle version à la main, ou déplacer l'AppImage dans un dossier à soi",
            "cannot replace the AppImage: the folder {} is not writable. Download the new version manually, or move the AppImage to a folder you own",
            dir
        );
    }
    let _ = app;
    e.to_string()
}
