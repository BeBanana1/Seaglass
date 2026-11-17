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
//!
//! Linux (#314) : seulement depuis une AppImage (`APPIMAGE`, validé par Tauri). Le plugin
//! réécrit ce fichier, puis l'app se relance elle-même. Sans AppImage, il prendrait
//! l'exécutable courant pour cible et remplacerait le binaire nu par une AppImage : les
//! mises à jour y sont désactivées (`supported`).
//!
//! Adresse (#452) : `latest.json` du dossier compilé (`servers::update_base`), absent de
//! `tauri.conf.json`. Sans adresse compilée, rien n'est vérifié. La vérification
//! périodique suit l'interrupteur « Mises à jour automatiques » ; le bouton, toujours.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::i18n::tr;
use crate::mpv::process::ProcessGuard;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// Premier contrôle un peu après le démarrage, pour ne pas ralentir l'ouverture.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
/// Surcharge de l'URL du manifeste (tests, autre canal). En debug, sans cette variable,
/// aucune vérification n'est faite.
const ENDPOINT_ENV: &str = "SEAGLASS_UPDATE_URL";

/// Manifeste à lire : `SEAGLASS_UPDATE_URL` au lancement, sinon `latest.json` du dossier
/// compilé (#452). `None` : app compilée sans mises à jour.
fn endpoint() -> Option<String> {
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

/// Vrai si l'app peut se mettre à jour elle-même : Windows (installeur NSIS), Linux
/// lancé depuis une AppImage.
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
        tracing::info!("mises à jour automatiques désactivées : app lancée hors d'une AppImage");
        return;
    }
    if cfg!(debug_assertions) && std::env::var(ENDPOINT_ENV).is_err() {
        return;
    }
    let Some(endpoint) = endpoint() else {
        tracing::info!("mises à jour absentes : aucune adresse compilée");
        return;
    };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            // Réglage relu à chaque tour : il peut changer pendant que l'app tourne.
            if crate::servers::auto_updates() {
                // Le téléchargement lancé par `check` émet lui-même `update-ready`.
                if let Err(e) = check(&app, &endpoint, &mpv).await {
                    tracing::warn!("vérification des mises à jour : {e}");
                }
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

    let update = match updater.check().await {
        Ok(update) => update,
        // Version publiée sans cette plateforme (release Windows seule, par exemple) :
        // rien à installer ici, ce n'est pas une erreur à montrer.
        Err(tauri_plugin_updater::Error::TargetNotFound(target)) => {
            tracing::info!(
                "manifeste sans entrée {target} : pas de mise à jour pour cette plateforme"
            );
            None
        }
        Err(e) => return Err(e.to_string()),
    };
    let Some(update) = update else {
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

/// Vrai si les Réglages peuvent proposer de vérifier les mises à jour (voir `supported`).
#[tauri::command]
pub fn updates_available(app: AppHandle) -> bool {
    supported(&app)
}

#[tauri::command]
pub fn update_status(state: State<'_, UpdateState>) -> Option<UpdateInfo> {
    state.info()
}

/// Installe tout de suite. Windows : l'installeur ferme l'app puis la relance. Linux :
/// l'AppImage est remplacée, puis l'app se relance (file sauvegardée et mpv arrêté par
/// `RunEvent::Exit`, comme à la fermeture).
#[tauri::command]
pub fn install_update(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    let pending = state
        .take()
        .ok_or_else(|| "aucune mise à jour prête".to_owned())?;
    if let Err(e) = pending.update.install(&pending.bytes) {
        let message = install_error(&app, e);
        // Gardée : après avoir déplacé l'AppImage, un nouvel essai ne retélécharge rien.
        if let Ok(mut slot) = state.pending.lock() {
            *slot = Some(pending);
        }
        return Err(message);
    }
    #[cfg(target_os = "linux")]
    app.request_restart();
    Ok(())
}

/// Message d'un échec d'installation. Linux : une AppImage rangée dans un dossier sans
/// droit d'écriture (`/opt`, clé montée en lecture seule…) ne peut pas être remplacée.
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
