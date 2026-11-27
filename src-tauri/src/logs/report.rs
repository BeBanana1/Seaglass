//! Crash reports (#192).
//!
//! At the time of the crash, `crash.txt` receives the cause: Rust panic (hook below) or
//! uncaught Kotlin exception (`CrashLog.kt`). At the next launch, `collect` turns it into
//! `last-crash.txt` with the end of the crashed launch's log and, on Android, the exit reason
//! given by the system (native crash, ANR).

use std::fmt::Write as _;
use std::fs;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use super::file::{OLD_LOG_FILE, redact, tail};

/// Written at the time of the crash; this name is reused by `CrashLog.kt`.
pub const CRASH_FILE: &str = "crash.txt";
/// Full report of the last crash, assembled at the next launch.
pub const REPORT_FILE: &str = "last-crash.txt";
/// Lines of the previous log attached to the report.
const TAIL_LINES: usize = 300;

static CRASH_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Installed from launch; `crash.txt` is only written once `set_dir` has been called.
pub fn install_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = panic_text(info);
        if let Some(path) = CRASH_PATH.get() {
            let _ = fs::write(path, redact(&text));
        }
        tracing::error!("{text}");
        previous(info);
    }));
}

pub fn set_dir(dir: &Path) {
    let _ = CRASH_PATH.set(dir.join(CRASH_FILE));
}

fn panic_text(info: &PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    let mut text = format!(
        "{}\npanic dans le thread « {} » : {}\n",
        header(SystemTime::now()),
        thread.name().unwrap_or("?"),
        info.payload_as_str().unwrap_or("(message non textuel)"),
    );
    if let Some(at) = info.location() {
        let _ = writeln!(text, "à {}:{}:{}", at.file(), at.line(), at.column());
    }
    let _ = write!(text, "\n{}", std::backtrace::Backtrace::force_capture());
    text
}

/// Version, platform and date (UTC): the report can be read after an update.
fn header(now: SystemTime) -> String {
    format!(
        "Seaglass {} ({} {}), {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        utc(now)
    )
}

/// `YYYY-MM-DD hh:mm:ss UTC`.
fn utc(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let utc = crate::utc::Utc::from_secs(i64::try_from(secs).unwrap_or(0));
    format!("{} {} UTC", utc.date(), utc.time())
}

/// At startup, after the log rotation: true if the previous launch crashed,
/// its report then being in `last-crash.txt`.
pub fn collect(dir: &Path, system_exit: Option<&str>) -> bool {
    let crash_path = dir.join(CRASH_FILE);
    let crash = fs::read_to_string(&crash_path).ok();
    if crash.is_none() && system_exit.is_none() {
        return false;
    }
    let log = tail(&dir.join(OLD_LOG_FILE), TAIL_LINES);
    let report = build(crash.as_deref(), system_exit, log.as_deref());
    if let Err(e) = fs::write(dir.join(REPORT_FILE), report) {
        tracing::warn!("crash report not saved: {e}");
        return false;
    }
    if crash.is_some()
        && let Err(e) = fs::remove_file(&crash_path)
    {
        tracing::warn!("{CRASH_FILE} not deleted: {e}");
    }
    true
}

fn build(crash: Option<&str>, system_exit: Option<&str>, log: Option<&str>) -> String {
    let mut report = String::from("# Rapport de plantage Seaglass\n");
    if let Some(crash) = crash {
        let _ = write!(report, "\n{}\n", crash.trim_end());
    }
    if let Some(exit) = system_exit {
        let _ = write!(
            report,
            "\n## Sortie signalée par Android\n\n{}\n",
            exit.trim_end()
        );
    }
    if let Some(log) = log {
        let _ = write!(
            report,
            "\n## Fin du journal ({TAIL_LINES} dernières lignes au plus)\n\n{}\n",
            redact(log)
        );
    }
    report
}

/// Content of `last-crash.txt`, if it exists.
pub fn last(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join(REPORT_FILE)).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn utc_formats_dates() {
        assert_eq!(utc(UNIX_EPOCH), "1970-01-01 00:00:00 UTC");
        let t = UNIX_EPOCH + Duration::from_secs(1_793_276_645);
        assert_eq!(utc(t), "2026-10-29 12:24:05 UTC");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(utc(leap), "2000-02-29 00:00:00 UTC");
    }

    #[test]
    fn report_joins_available_parts() {
        let report = build(Some("panic ici\n"), None, Some("ligne https://a/b?sig=1"));
        assert!(report.starts_with("# Rapport de plantage Seaglass\n\npanic ici\n"));
        assert!(!report.contains("Android"));
        assert!(report.ends_with("ligne https://a/b?…\n"));
        let report = build(None, Some("ANR"), None);
        assert!(report.contains("## Sortie signalée par Android\n\nANR\n"));
    }

    #[test]
    fn collect_moves_crash_into_report() {
        let dir = std::env::temp_dir().join(format!("seaglass-crash-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(!collect(&dir, None));
        fs::write(dir.join(CRASH_FILE), "boom").unwrap();
        fs::write(dir.join(OLD_LOG_FILE), "avant\n").unwrap();
        assert!(collect(&dir, None));
        assert!(!dir.join(CRASH_FILE).exists());
        let report = last(&dir).unwrap();
        assert!(
            report.contains("boom") && report.contains("avant"),
            "{report}"
        );
        assert!(!collect(&dir, None));
        fs::remove_dir_all(&dir).unwrap();
    }
}
