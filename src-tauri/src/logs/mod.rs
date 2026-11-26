//! Logs and crash reports (#192), kept local: nothing leaves on its own,
//! the user copies the report from Settings › Application.

mod file;
mod report;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Manager;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer as _};

/// Lines of the current log returned by "Copy the log".
const COPY_LINES: usize = 500;

static DIR: OnceLock<PathBuf> = OnceLock::new();
/// Crash at the previous launch, not yet reported to the UI.
static CRASH_NOTICE: AtomicBool = AtomicBool::new(false);
/// Crash at the previous launch; never reset to false, unlike `CRASH_NOTICE`.
static CRASHED: AtomicBool = AtomicBool::new(false);

/// First call of the app: standard output, log on disk (opened by `setup`) and
/// panic hook.
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

/// In Tauri's `setup`: opens this launch's log and picks up the previous launch's
/// crash. A failure is logged without stopping the app from starting.
pub fn setup(app: &tauri::App) {
    let dir = match app.path().app_log_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!("log folder not found: {e}");
            return;
        }
    };
    if let Err(e) = file::open(&dir) {
        tracing::warn!("log on disk unavailable: {e}");
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
            "the previous launch crashed: report in {}",
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

/// Log folder, if it could be opened.
#[cfg(desktop)]
pub fn log_dir() -> Option<&'static Path> {
    dir()
}

/// True if the previous launch crashed: playback does not resume on its own (#353).
#[cfg_attr(mobile, allow(dead_code))]
pub fn previous_launch_crashed() -> bool {
    CRASHED.load(Ordering::SeqCst)
}

/// True only once if the previous launch crashed.
pub fn take_crash_notice() -> bool {
    CRASH_NOTICE.swap(false, Ordering::SeqCst)
}

/// Report of the last crash, if there is one.
pub fn last_crash() -> Option<String> {
    report::last(dir()?)
}

/// End of this launch's log, for a bug without a crash.
pub fn current_log() -> Option<String> {
    file::tail(&dir()?.join(file::LOG_FILE), COPY_LINES)
}
