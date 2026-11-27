//! Drive mode (#193): open request (`seaglass://drive` link, app shortcut) and
//! screen kept on. A tile reads its source like the Play button of its page
//! (`src/lib/pins.ts`): same order, remembered sort included.

use tauri::AppHandle;

use super::CmdResult;

/// True if the `seaglass://drive` link or the app shortcut asked for drive mode
/// since the last call; always false on desktop. Read at UI startup and on the
/// `open-drive` event.
#[tauri::command]
pub async fn take_drive_request() -> CmdResult<bool> {
    Ok(crate::integrations::take_drive_request())
}

/// Keeps the screen on while drive mode is open (Android; nothing on desktop).
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
