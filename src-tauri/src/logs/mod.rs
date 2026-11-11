//! Journaux et rapports de plantage (#192), gardés en local : rien ne part tout seul,
//! l'utilisateur copie le rapport depuis Réglages › Application.

mod file;
mod report;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer as _};

/// Lignes du journal courant renvoyées par « Copier le journal ».
const COPY_LINES: usize = 500;

static DIR: OnceLock<PathBuf> = OnceLock::new();
/// Plantage au lancement précédent, pas encore signalé à l'UI.
static CRASH_NOTICE: AtomicBool = AtomicBool::new(false);
/// Plantage au lancement précédent ; jamais remis à faux, contrairement à `CRASH_NOTICE`.
static CRASHED: AtomicBool = AtomicBool::new(false);

/// Premier appel de l'app : sortie standard, journal sur disque (ouvert par `setup`) et
/// hook des panics.
pub fn init() {
    let filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("seaglass_lib=info"))
    };
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter()))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file::FileWriter)
                .with_filter(filter()),
        )
        .init();
    report::install_hook();
}

/// Dans le `setup` de Tauri : ouvre le journal de ce lancement et recueille le plantage du
/// précédent. Un échec est journalisé sans empêcher l'app de démarrer.
pub fn setup(app: &tauri::App) {
    let dir = match app.path().app_log_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!("dossier des journaux introuvable : {e}");
            return;
        }
    };
    if let Err(e) = file::open(&dir) {
        tracing::warn!("journal sur disque indisponible : {e}");
        return;
    }
    tracing::info!(
        "Seaglass {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    report::set_dir(&dir);
    #[cfg(target_os = "android")]
    let system_exit = crate::android::crash_setup(&dir);
    #[cfg(not(target_os = "android"))]
    let system_exit: Option<String> = None;
    if report::collect(&dir, system_exit.as_deref()) {
        tracing::warn!(
            "le lancement précédent a planté : rapport dans {}",
            report::REPORT_FILE
        );
        CRASH_NOTICE.store(true, Ordering::SeqCst);
        CRASHED.store(true, Ordering::SeqCst);
    }
    let _ = DIR.set(dir);
}

fn dir() -> Option<&'static Path> {
    DIR.get().map(PathBuf::as_path)
}

/// Dossier des journaux, s'il a pu être ouvert.
#[cfg(desktop)]
pub fn log_dir() -> Option<&'static Path> {
    dir()
}

/// Vrai si le lancement précédent a planté : la lecture ne reprend pas seule (#353).
#[cfg_attr(mobile, allow(dead_code))]
pub fn previous_launch_crashed() -> bool {
    CRASHED.load(Ordering::SeqCst)
}

/// Vrai une seule fois si le lancement précédent a planté.
pub fn take_crash_notice() -> bool {
    CRASH_NOTICE.swap(false, Ordering::SeqCst)
}

/// Rapport du dernier plantage, s'il y en a un.
pub fn last_crash() -> Option<String> {
    report::last(dir()?)
}

/// Fin du journal de ce lancement, pour un bug sans plantage.
pub fn current_log() -> Option<String> {
    file::tail(&dir()?.join(file::LOG_FILE), COPY_LINES)
}
