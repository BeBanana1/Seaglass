//! Mises à jour de l'app (plugin `tauri-plugin-updater`).
//!
//! Vérification au démarrage puis toutes les 6 h. Une version plus récente est téléchargée
//! en arrière-plan (signature vérifiée par le plugin), puis l'UI reçoit `update-ready`.
//! L'utilisateur choisit « Installer et redémarrer » (`install_update`) ou « Plus tard » :
//! l'installation se fait alors à la fermeture de l'app. Rien n'est jamais installé
//! sans action de sa part, et la lecture n'est jamais coupée d'office.
//!
//! `check_for_updates` (bouton des Réglages) lance la même vérification à la demande.

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
}

struct Pending {
    update: Update,
    bytes: Vec<u8>,
}

/// Mise à jour téléchargée et vérifiée, en attente d'installation.
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Pending>>,
    /// Sérialise la tâche de fond et le bouton : jamais deux téléchargements à la fois.
    checking: tokio::sync::Mutex<()>,
}

impl UpdateState {
    fn info(&self) -> Option<UpdateInfo> {
        let guard = self.pending.lock().ok()?;
        guard.as_ref().map(|p| UpdateInfo {
            version: p.update.version.clone(),
            current_version: p.update.current_version.clone(),
            notes: p.update.body.clone(),
        })
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
            match check(&app, endpoint.as_deref(), &mpv).await {
                Ok(Some(info)) => {
                    let _ = app.emit("update-ready", info);
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("vérification des mises à jour : {e}"),
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Vérifie et télécharge une version plus récente. Renvoie la mise à jour nouvellement
/// prête, ou `None` si rien n'a changé (app à jour, ou version déjà téléchargée).
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
    // L'installeur ferme l'app par `std::process::exit` : mpv doit être arrêté avant.
    let mpv = Arc::clone(mpv);
    let updater = builder
        .on_before_exit(move || mpv.kill())
        .build()
        .map_err(|e| e.to_string())?;

    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    if state.info().is_some_and(|i| i.version == update.version) {
        return Ok(None);
    }
    tracing::info!("version {} disponible, téléchargement", update.version);
    // `download` vérifie la signature : des octets non signés par notre clé sont refusés.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(
        "version {} prête ({} octets, signature vérifiée)",
        update.version,
        bytes.len()
    );
    if let Ok(mut slot) = state.pending.lock() {
        *slot = Some(Pending { update, bytes });
    }
    Ok(state.info())
}

/// Vérification à la demande. Renvoie la mise à jour prête (nouvelle ou déjà téléchargée
/// et reportée), `None` si l'app est à jour. Une mise à jour prête est réémise en
/// `update-ready` pour que la proposition d'installation réapparaisse.
#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    mpv: State<'_, Arc<ProcessGuard>>,
) -> Result<Option<UpdateInfo>, String> {
    let endpoint = std::env::var(ENDPOINT_ENV).ok();
    check(&app, endpoint.as_deref(), &mpv).await?;
    let info = app.state::<UpdateState>().info();
    if let Some(info) = &info {
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
