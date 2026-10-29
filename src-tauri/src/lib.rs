mod commands;
mod mpv;
mod player;
mod store;
mod tidal;

use std::sync::Arc;

use tauri::path::BaseDirectory;
use tauri::{Manager, RunEvent};
use tracing_subscriber::EnvFilter;

use commands::AppState;
use mpv::process::ProcessGuard;
use store::Store;
use tidal::TidalClient;

const DB_FILE: &str = "seaglass.db";

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
            let store = open_store(app);
            let mpv_process = Arc::new(ProcessGuard::default());
            let player = player::spawn(
                app.handle().clone(),
                Arc::clone(&tidal),
                store,
                mpv_conf,
                Arc::clone(&mpv_process),
            );
            app.manage(AppState { tidal, player });
            app.manage(mpv_process);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::login,
            commands::logout,
            commands::search,
            commands::play_tracks,
            commands::play_next,
            commands::add_to_queue,
            commands::remove_from_queue,
            commands::next,
            commands::previous,
            commands::toggle_pause,
            commands::seek,
            commands::set_shuffle,
            commands::set_repeat,
            commands::player_state,
            commands::queue_state,
        ])
        .build(tauri::generate_context!())
        .expect("échec du lancement de l'application")
        .run(|app, event| {
            // Tauri quitte sans exécuter les destructeurs : mpv doit être arrêté ici.
            if let RunEvent::Exit = event {
                app.state::<Arc<ProcessGuard>>().kill();
            }
        });
}

/// Base SQLite dans le dossier de données de l'app ; l'app fonctionne sans, sans persistance.
fn open_store(app: &tauri::App) -> Option<Arc<Store>> {
    let open = || -> anyhow::Result<Store> {
        let dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&dir)?;
        Ok(Store::open(&dir.join(DB_FILE))?)
    };
    match open() {
        Ok(store) => Some(Arc::new(store)),
        Err(e) => {
            tracing::error!("base SQLite indisponible, pas de persistance : {e}");
            None
        }
    }
}
