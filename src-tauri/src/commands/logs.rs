//! Logs and crash reports (#192): read by the UI to be copied, never sent.

use crate::logs;

/// True only once if the previous launch crashed: the UI then offers the report.
#[tauri::command]
pub fn take_crash_notice() -> bool {
    logs::take_crash_notice()
}

/// Report of the last crash (`null` if there is none).
#[tauri::command]
pub fn last_crash_report() -> Option<String> {
    logs::last_crash()
}

/// End of this launch's log (`null` if the on-disk log is unavailable).
#[tauri::command]
pub fn current_log() -> Option<String> {
    logs::current_log()
}

/// Opens the logs folder in the file explorer.
#[cfg(desktop)]
#[tauri::command]
pub fn open_log_dir(app: tauri::AppHandle) -> super::CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;

    let dir = logs::log_dir().ok_or_else(|| {
        crate::i18n::tr("Journal sur disque indisponible", "Log file unavailable").to_owned()
    })?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(super::err)
}
