#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "android", test))]
mod apk_update;
mod commands;
mod engine;
mod i18n;
mod integrations;
#[cfg(any(target_os = "android", test))]
mod media3;
mod mpv;
mod player;
mod store;
mod tidal;
#[cfg(desktop)]
mod updater;

use std::sync::Arc;

use tauri::Manager;
#[cfg(desktop)]
use tauri::path::BaseDirectory;
#[cfg(desktop)]
use tauri::{RunEvent, WindowEvent};
use tracing_subscriber::EnvFilter;

use commands::AppState;
#[cfg(desktop)]
use integrations::tray;
#[cfg(desktop)]
use mpv::process::ProcessGuard;
use store::Store;
use tidal::TidalClient;
#[cfg(desktop)]
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

    // L'instance unique du bureau doit être le premier plugin : celui-ci vient après.
    // Ouverture de liens dans le navigateur du système (#188), appelée seulement depuis
    // Rust (`commands::project`) : aucune permission du plugin côté UI.
    platform_builder(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(target_os = "android")]
            if let Err(e) = android::init_tls() {
                tracing::error!("vérification HTTPS d'Android indisponible : {e}");
            }
            #[cfg(target_os = "android")]
            android::install_keystore();
            let tidal = Arc::new(TidalClient::new()?);
            let store = open_store(app);
            // Avant le lecteur : ses toasts et les requêtes TIDAL suivent la langue.
            let ui_settings = commands::settings::load_from(store.as_deref());
            i18n::set(ui_settings.language.resolve());
            ui_settings.apply_to_backend();
            #[cfg(desktop)]
            commands::settings::refresh_autostart(app.handle());
            #[cfg(desktop)]
            let mpv_process = Arc::new(ProcessGuard::default());
            #[cfg(desktop)]
            let launch = engine::Launch {
                conf: app
                    .path()
                    .resolve("mpv/seaglass.conf", BaseDirectory::Resource)?,
                process: Arc::clone(&mpv_process),
            };
            #[cfg(mobile)]
            let launch = engine::Launch {
                app: app.handle().clone(),
            };
            let player = player::spawn(
                app.handle().clone(),
                Arc::clone(&tidal),
                store.clone(),
                launch,
            );
            attach_integrations(app, &player);
            // La fenêtre est créée cachée (`visible: false`) : affichée ici, sauf au
            // lancement au démarrage si l'icône de la zone de notification existe.
            #[cfg(desktop)]
            if !tray::start_hidden(std::env::args()) {
                tray::show_main(app.handle());
            }
            app.manage(AppState {
                tidal,
                player,
                store,
            });
            #[cfg(desktop)]
            {
                updater::spawn(app.handle().clone(), Arc::clone(&mpv_process));
                app.manage(mpv_process);
            }
            #[cfg(target_os = "android")]
            apk_update::spawn(app.handle().clone());
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
            commands::snapshots::cached_playlist,
            commands::snapshots::cached_library,
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
            commands::catalog::album_credits,
            commands::link::open_link,
            commands::catalog::hires_twin_album,
            commands::collection::likes,
            commands::collection::set_liked,
            commands::collection::own_playlists,
            commands::collection::add_to_playlist,
            commands::collection::create_playlist,
            commands::collection::remove_from_playlist,
            commands::project::open_project_page,
            commands::player::play_tracks,
            commands::player::play_next,
            commands::player::take_now_playing_request,
            commands::player::add_to_queue,
            commands::player::enqueue_tracks,
            commands::player::remove_from_queue,
            commands::player::move_in_queue,
            commands::player::clear_queue,
            commands::player::play_from_queue,
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
            commands::settings::set_home_featured,
            commands::settings::reset_home_layout,
            commands::settings::set_pins,
            commands::settings::set_list_sort,
            commands::settings::playback_settings,
            commands::settings::set_playback_settings,
            commands::settings::audio_devices,
            commands::settings::platform,
            commands::settings::autostart_enabled,
            #[cfg(desktop)]
            commands::settings::set_autostart,
            #[cfg(desktop)]
            commands::window::fullscreen,
            #[cfg(desktop)]
            commands::window::set_fullscreen,
            #[cfg(desktop)]
            updater::update_status,
            #[cfg(desktop)]
            updater::install_update,
            #[cfg(desktop)]
            updater::check_for_updates,
            #[cfg(target_os = "android")]
            apk_update::update_status,
            #[cfg(target_os = "android")]
            apk_update::install_update,
            #[cfg(target_os = "android")]
            apk_update::check_for_updates,
        ])
        .build(tauri::generate_context!())
        .expect("échec du lancement de l'application")
        .run(|_app, _event| {
            // Tauri quitte sans exécuter les destructeurs : mpv doit être arrêté ici.
            // Une mise à jour reportée (« Plus tard ») s'installe à ce moment-là.
            #[cfg(desktop)]
            if let RunEvent::Exit = _event {
                _app.state::<Arc<ProcessGuard>>().kill();
                _app.state::<UpdateState>().install_on_exit();
            }
            #[cfg(target_os = "android")]
            android::keep_alive(_app, &_event);
        });
}

/// Plugins et comportement de fenêtre du bureau.
#[cfg(desktop)]
fn platform_builder(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
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
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. }
                if window.label() == "main" && tray::hide_on_close() =>
            {
                api.prevent_close();
                let _ = window.hide();
            }
            // Le plein écran (#131) change la taille : l'UI suit l'état réel de la fenêtre.
            WindowEvent::Resized(_) => commands::window::on_resized(window),
            _ => {}
        })
}

/// Android : ni zone de notification, ni démarrage auto, ni updater de Tauri (mises à
/// jour par `apk_update`) ; le pont natif Kotlin à la place.
#[cfg(mobile)]
fn platform_builder(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(tauri_plugin_seaglass_mobile::init())
        .manage(apk_update::UpdateState::default());
    builder
}

/// SMTC (touches média, panneau Windows), branché sur la fenêtre principale, et icône de
/// la zone de notification ; session média sur Android. Un échec est journalisé sans empêcher l'app de démarrer.
fn attach_integrations(app: &tauri::App, player: &tokio::sync::mpsc::Sender<player::Cmd>) {
    let mut observers = integrations::Observers::default();
    #[cfg(desktop)]
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
    #[cfg(target_os = "android")]
    {
        observers.media_session = Some(integrations::media_session::start(
            app.handle(),
            player.clone(),
        ));
    }
    #[cfg(not(any(windows, target_os = "android")))]
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
