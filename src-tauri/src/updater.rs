//! Mises à jour de l'app (plugin `tauri-plugin-updater`).
//!
//! Vérification au démarrage puis toutes les 6 h. Une version plus récente est téléchargée
//! en arrière-plan (signature vérifiée par le plugin), puis l'UI reçoit `update-ready`.
//! L'utilisateur choisit « Installer et redémarrer » (`install_update`) ou « Plus tard » :
//! l'installation se fait alors à la fermeture de l'app. Rien n'est jamais installé
//! sans action de sa part, et la lecture n'est jamais coupée d'office.
//!
//! `check_for_updates` (bouton des Réglages) lance la même vérification à la demande. Elle
//! répond dès le manifeste lu (#187) : le téléchargement de l'installeur (une quarantaine de
//! Mo) continue en arrière-plan et se termine par `update-ready`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::mpv::process::ProcessGuard;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// Premier contrôle un peu après le démarrage, pour ne pas ralentir l'ouverture.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
/// Surcharge de l'URL du manifeste (tests, autre canal). En debug, sans cette variable,
/// aucune vérification n'est faite.
const ENDPOINT_ENV: &str = "SEAGLASS_UPDATE_URL";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Notes de version (texte libre du manifeste).
    pub notes: Option<String>,
    /// Vrai tant que l'installeur se télécharge : `update-ready` suivra (#187).
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

/// Mise à jour téléchargée et vérifiée, en attente d'installation.
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Pending>>,
    /// Version en cours de téléchargement : jamais deux téléchargements à la fois.
    downloading: Mutex<Option<UpdateInfo>>,
    /// Sérialise la lecture du manifeste entre la tâche de fond et le bouton.
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

    /// À la fermeture de l'app : installe la mise à jour reportée, sans relancer l'app.
    pub fn install_on_exit(&self) {
        if let Some(p) = self.take() {
            tracing::info!(
                "installation de la version {} à la fermeture",
                p.update.version
            );
            if let Err(e) = p.update.restart_after_install(false).install(&p.bytes) {
                tracing::error!("installation de la mise à jour impossible : {e}");
            }
        }
    }
}

pub fn spawn(app: AppHandle, mpv: Arc<ProcessGuard>) {
    let endpoint = std::env::var(ENDPOINT_ENV).ok();
    if cfg!(debug_assertions) && endpoint.is_none() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            // Le téléchargement lancé par `check` émet lui-même `update-ready`.
            if let Err(e) = check(&app, endpoint.as_deref(), &mpv).await {
                tracing::warn!("vérification des mises à jour : {e}");
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Lit le manifeste. Une version plus récente, ni prête ni déjà en téléchargement, est
/// téléchargée en arrière-plan (puis `update-ready`). Renvoie l'état de la mise à jour
/// (prête, ou en téléchargement), `None` si l'app est à jour.
async fn check(
    app: &AppHandle,
    endpoint: Option<&str>,
    mpv: &Arc<ProcessGuard>,
) -> Result<Option<UpdateInfo>, String> {
    let state = app.state::<UpdateState>();
    let _checking = state.checking.lock().await;
    let mut builder = app.updater_builder();
    if let Some(url) = endpoint {
        let url = url
            .parse()
            .map_err(|e| format!("{ENDPOINT_ENV} invalide : {e}"))?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    // L'installeur ferme l'app par `std::process::exit` : la file doit être sauvegardée
    // (#260) et mpv arrêté avant.
    let mpv = Arc::clone(mpv);
    let handle = app.clone();
    let updater = builder
        .on_before_exit(move || {
            crate::flush_player(&handle);
            mpv.kill();
        })
        .build()
        .map_err(|e| e.to_string())?;

    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        // Une version prête mais retirée du manifeste reste installable.
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
    tracing::info!("version {} disponible, téléchargement", update.version);
    tauri::async_runtime::spawn(download(app.clone(), update));
    Ok(Some(info))
}

/// Télécharge l'installeur, puis le garde et émet `update-ready` (`update-failed` avec le
/// message en cas d'échec : la page Réglages attend la fin d'un téléchargement demandé).
async fn download(app: AppHandle, update: Update) {
    let state = app.state::<UpdateState>();
    // `download` vérifie la signature : des octets non signés par notre clé sont refusés.
    let result = update.download(|_, _| {}, || {}).await;
    state.set_downloading(None);
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("téléchargement de la version {} : {e}", update.version);
            let _ = app.emit("update-failed", e.to_string());
            return;
        }
    };
    tracing::info!(
        "version {} prête ({} octets, signature vérifiée)",
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

/// Vérification à la demande, rendue dès le manifeste lu : la mise à jour prête (nouvelle
/// ou reportée, réémise en `update-ready` pour que la proposition d'installation
/// réapparaisse), en téléchargement (`downloading`, `update-ready` suivra), ou `None` si
/// l'app est à jour.
#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    mpv: State<'_, Arc<ProcessGuard>>,
) -> Result<Option<UpdateInfo>, String> {
    let endpoint = std::env::var(ENDPOINT_ENV).ok();
    let info = check(&app, endpoint.as_deref(), &mpv).await?;
    if let Some(info) = info.as_ref().filter(|i| !i.downloading) {
        let _ = app.emit("update-ready", info);
    }
    Ok(info)
}

#[tauri::command]
pub fn update_status(state: State<'_, UpdateState>) -> Option<UpdateInfo> {
    state.info()
}

/// Installe tout de suite : l'installeur ferme l'app puis la relance.
#[tauri::command]
pub fn install_update(state: State<'_, UpdateState>) -> Result<(), String> {
    let pending = state
        .take()
        .ok_or_else(|| "aucune mise à jour prête".to_owned())?;
    pending
        .update
        .install(&pending.bytes)
        .map_err(|e| e.to_string())
}
