mod commands;
mod i18n;
mod integrations;
mod mpv;
mod player;
mod store;
mod tidal;
mod updater;

use std::sync::Arc;

use tauri::path::BaseDirectory;
use tauri::{Manager, RunEvent, WindowEvent};
use tracing_subscriber::EnvFilter;

use commands::AppState;
use integrations::tray;
use mpv::process::ProcessGuard;
use store::Store;
use tidal::TidalClient;
use updater::UpdateState;

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
        // Une seule instance (#83) : relancer Seaglass, par exemple pendant qu'il est réduit
        // dans la zone de notification, réaffiche la fenêtre existante au lieu d'ouvrir un
        // second lecteur. Enregistré en premier, comme le demande le plugin.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if tray::show_on_second_launch(args) {
                tray::show_main(app);
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Lancement au démarrage (clé `Run` de l'utilisateur, exe courant). Piloté
        // seulement par les commandes `settings` : aucune permission côté UI. L'app lancée
        // ainsi démarre cachée dans la zone de notification (#83).
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg(tray::MINIMIZED_ARG)
                .build(),
        )
        .manage(UpdateState::default())
        // Réglage « Fermer la fenêtre » sur « Réduire » : la fenêtre principale est cachée.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && tray::hide_on_close()
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let tidal = Arc::new(TidalClient::new()?);
            let mpv_conf = app
                .path()
                .resolve("mpv/seaglass.conf", BaseDirectory::Resource)?;
            let store = open_store(app);
            // Avant le lecteur : ses toasts et les requêtes TIDAL suivent la langue.
            let ui_settings = commands::settings::load_from(store.as_deref());
            i18n::set(ui_settings.language.resolve());
            ui_settings.apply_to_backend();
            commands::settings::refresh_autostart(app.handle());
            let mpv_process = Arc::new(ProcessGuard::default());
            let player = player::spawn(
                app.handle().clone(),
                Arc::clone(&tidal),
                store.clone(),
                mpv_conf,
                Arc::clone(&mpv_process),
            );
            attach_integrations(app, &player);
            // La fenêtre est créée cachée (`visible: false`) : affichée ici, sauf au
            // lancement au démarrage si l'icône de la zone de notification existe.
            if !tray::start_hidden(std::env::args()) {
                tray::show_main(app.handle());
            }
            app.manage(AppState {
                tidal,
                player,
                store,
            });
            updater::spawn(app.handle().clone(), Arc::clone(&mpv_process));
            app.manage(mpv_process);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::auth_status,
            commands::auth::login,
            commands::auth::logout,
            commands::catalog::search,
            commands::catalog::album,
            commands::catalog::artist,
            commands::catalog::playlist,
            commands::catalog::mix,
            commands::catalog::radio,
            commands::catalog::library_mixes,
            commands::catalog::library_tracks,
            commands::catalog::library_albums,
            commands::catalog::library_artists,
            commands::catalog::library_playlists,
            commands::catalog::home,
            commands::catalog::feed,
            commands::catalog::lyrics,
            commands::catalog::track_info,
            commands::catalog::hires_twin_album,
            commands::collection::likes,
            commands::collection::set_liked,
            commands::collection::own_playlists,
            commands::collection::add_to_playlist,
            commands::collection::create_playlist,
            commands::collection::remove_from_playlist,
            commands::player::play_tracks,
            commands::player::play_next,
            commands::player::add_to_queue,
            commands::player::enqueue_tracks,
            commands::player::remove_from_queue,
            commands::player::move_in_queue,
            commands::player::clear_queue,
            commands::player::answer_hires_offer,
            commands::player::set_sleep_timer,
            commands::player::next,
            commands::player::previous,
            commands::player::toggle_pause,
            commands::player::seek,
            commands::player::set_shuffle,
            commands::player::set_repeat,
            commands::player::set_volume,
            commands::player::set_muted,
            commands::player::player_state,
            commands::player::queue_state,
            commands::settings::ui_settings,
            commands::settings::set_ui_settings,
            commands::settings::language,
            commands::settings::set_language,
            commands::settings::set_home_layout,
            commands::settings::reset_home_layout,
            commands::settings::set_pins,
            commands::settings::set_list_sort,
            commands::settings::playback_settings,
            commands::settings::set_playback_settings,
            commands::settings::audio_devices,
            commands::settings::autostart_enabled,
            commands::settings::set_autostart,
            updater::update_status,
            updater::install_update,
            updater::check_for_updates,
        ])
        .build(tauri::generate_context!())
        .expect("échec du lancement de l'application")
        .run(|app, event| {
            // Tauri quitte sans exécuter les destructeurs : mpv doit être arrêté ici.
            // Une mise à jour reportée (« Plus tard ») s'installe à ce moment-là.
            if let RunEvent::Exit = event {
                app.state::<Arc<ProcessGuard>>().kill();
                app.state::<UpdateState>().install_on_exit();
            }
        });
}

/// SMTC (touches média, panneau Windows), branché sur la fenêtre principale, et icône de
/// la zone de notification. Un échec est journalisé sans empêcher l'app de démarrer.
fn attach_integrations(app: &tauri::App, player: &tokio::sync::mpsc::Sender<player::Cmd>) {
    let mut observers = integrations::Observers::default();
    match tray::create(app.handle(), player.clone()) {
        Ok(handle) => observers.tray = Some(handle),
        Err(e) => tracing::warn!("zone de notification indisponible : {e}"),
    }
    #[cfg(windows)]
    {
        let hwnd = app
            .get_webview_window("main")
            .ok_or_else(|| "fenêtre principale introuvable".to_owned())
            .and_then(|w| w.hwnd().map_err(|e| e.to_string()));
        match hwnd.and_then(|h| integrations::smtc::start(h.0 as isize, player.clone())) {
            Ok(smtc) => observers.smtc = Some(smtc),
            Err(e) => tracing::warn!("SMTC indisponible : {e}"),
        }
    }
    #[cfg(not(windows))]
    let _ = app;
    if player
        .try_send(player::Cmd::AttachObservers(Box::new(observers)))
        .is_err()
    {
        tracing::warn!("intégrations non branchées : lecteur indisponible");
    }
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
