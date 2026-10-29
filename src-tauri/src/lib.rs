mod commands;
mod mpv;
mod player;
mod tidal;

use std::sync::Arc;

use tauri::Manager;
use tauri::path::BaseDirectory;
use tracing_subscriber::EnvFilter;

use commands::AppState;
use tidal::TidalClient;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("seaglass_lib=info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let tidal = Arc::new(TidalClient::new()?);
            let mpv_conf = app
                .path()
                .resolve("mpv/seaglass.conf", BaseDirectory::Resource)?;
            let player = player::spawn(app.handle().clone(), Arc::clone(&tidal), mpv_conf);
            app.manage(AppState { tidal, player });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::login,
            commands::logout,
            commands::search,
            commands::play,
            commands::toggle_pause,
            commands::seek,
            commands::player_state,
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de l'application");
}
