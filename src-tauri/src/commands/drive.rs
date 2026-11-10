//! Mode conduite (#193) : demande d'ouverture (lien `seaglass://drive`, raccourci de l'app) et
//! écran gardé allumé. Une tuile lit sa source comme le bouton Lecture de sa page
//! (`src/lib/pins.ts`) : même ordre, tri mémorisé compris.

use tauri::AppHandle;

use super::CmdResult;

/// Vrai si le lien `seaglass://drive` ou le raccourci de l'app a demandé le mode conduite
/// depuis le dernier appel ; toujours faux sur le bureau. Lu au démarrage de l'UI et à
/// l'événement `open-drive`.
#[tauri::command]
pub async fn take_drive_request() -> CmdResult<bool> {
    Ok(crate::integrations::take_drive_request())
}

/// Garde l'écran allumé tant que le mode conduite est ouvert (Android ; rien sur le bureau).
#[tauri::command]
pub async fn keep_screen_on(app: AppHandle, on: bool) -> CmdResult<()> {
    #[cfg(target_os = "android")]
    {
        tauri_plugin_seaglass_mobile::native(&app)
            .call("keepScreenOn", serde_json::json!({ "on": on }))
            .await
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, on);
        Ok(())
    }
}
