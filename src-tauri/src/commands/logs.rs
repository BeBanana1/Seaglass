//! Journaux et rapports de plantage (#192) : lus par l'UI pour être copiés, jamais envoyés.

use crate::logs;

/// Vrai une seule fois si le lancement précédent a planté : l'UI propose alors le rapport.
#[tauri::command]
pub fn take_crash_notice() -> bool {
    logs::take_crash_notice()
}

/// Rapport du dernier plantage (`null` s'il n'y en a pas).
#[tauri::command]
pub fn last_crash_report() -> Option<String> {
    logs::last_crash()
}

/// Fin du journal de ce lancement (`null` si le journal sur disque est indisponible).
#[tauri::command]
pub fn current_log() -> Option<String> {
    logs::current_log()
}

/// Ouvre le dossier des journaux dans l'explorateur de fichiers.
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
