#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "android", test))]
mod apk_update;
mod commands;
mod content;
mod engine;
#[cfg(desktop)]
mod export;
mod foreign;
mod i18n;
mod integrations;
mod jam;
mod logs;
#[cfg(any(target_os = "android", test))]
mod media3;
mod mpv;
mod player;
mod store;
mod tidal;
#[cfg(desktop)]
mod updater;
mod utc;

use std::sync::Arc;

use tauri::Manager;
#[cfg(desktop)]
use tauri::path::BaseDirectory;
#[cfg(desktop)]
use tauri::{RunEvent, WindowEvent};

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
    logs::init();

    // L'instance unique du bureau doit être le premier plugin : celui-ci vient après.
    // Ouverture de liens dans le navigateur du système (#188), appelée seulement depuis
    // Rust (`commands::project`) : aucune permission du plugin côté UI.
    platform_builder(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        // Jam (#25) : arrêtée tant que l'hôte ne la démarre pas.
        .manage(jam::JamState::default())
        // Télécommande (#336) : rallumée plus bas si elle l'était.
        .manage(jam::remote::RemoteState::default())
        .setup(|app| {
            #[cfg(target_os = "android")]
            if let Err(e) = android::init_tls() {
                tracing::error!("vérification HTTPS d'Android indisponible : {e}");
            }
            #[cfg(target_os = "android")]
            android::install_keystore();
            // Avant le reste : un plantage plus loin a déjà son journal sur disque (#192).
            logs::setup(app);
            let tidal = Arc::new(TidalClient::new()?);
            let store = open_store(app);
            // Avant le lecteur : ses toasts et les requêtes TIDAL suivent la langue.
            let ui_settings = commands::settings::load_from(store.as_deref());
            i18n::set(ui_settings.language.resolve());
            ui_settings.apply_to_backend();
            commands::shortcuts::sync(&ui_settings.pins);
            #[cfg(desktop)]
            commands::settings::zoom::apply(app.handle(), ui_settings.zoom);
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
            // Reprise de la lecture au lancement (#353) : jamais au démarrage de session
            // (`--minimized`) ni après un plantage ; bureau seulement.
            #[cfg(desktop)]
            let may_resume = !std::env::args().any(|a| a == tray::MINIMIZED_ARG)
                && !logs::previous_launch_crashed();
            #[cfg(mobile)]
            let may_resume = false;
            let player = player::spawn(
                app.handle().clone(),
                Arc::clone(&tidal),
                store.clone(),
                launch,
                may_resume,
            );
            attach_integrations(app, &player);
            // La fenêtre est créée cachée (`visible: false`) : affichée ici, sauf au
            // lancement au démarrage si l'icône de la zone de notification existe.
            #[cfg(desktop)]
            if !tray::start_hidden(std::env::args()) {
                tray::show_main(app.handle());
            }
            spawn_session_watch(app.handle().clone(), Arc::clone(&tidal));
            app.manage(AppState {
                tidal,
                player,
                store,
            });
            #[cfg(desktop)]
            {
                commands::remote::restore(app.handle());
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
            commands::auth::login_in_browser,
            #[cfg(desktop)]
            commands::auth::browser_login_start,
            #[cfg(desktop)]
            commands::auth::browser_login_finish,
            commands::auth::logout,
            commands::catalog::search,
            commands::search_history::search_history,
            commands::search_history::note_search_history,
            commands::search_history::remove_search_history,
            commands::search_history::clear_search_history,
            commands::catalog::album,
            commands::catalog::artist,
            commands::catalog::playlist,
            commands::snapshots::cached_playlist,
            commands::snapshots::cached_library,
            commands::catalog::mix,
            commands::catalog::radio,
            commands::catalog::library_mixes,
            commands::catalog::artist_extra,
            commands::catalog::album_related,
            commands::play_history::play_history,
            commands::link::take_shared_link,
            commands::link::link_handling,
            commands::link::open_link_settings,
            commands::play_history::clear_play_history,
            commands::play_stats::play_stats,
            commands::smart_lists::smart_list,
            commands::catalog::library_tracks,
            commands::catalog::library_albums,
            commands::catalog::library_artists,
            commands::catalog::library_playlists,
            commands::catalog::home,
            commands::catalog::home_rest,
            commands::catalog::feed,
            commands::catalog::lyrics,
            commands::catalog::track_info,
            commands::catalog::album_credits,
            commands::link::open_link,
            commands::changelog::changelog,
            commands::logs::take_crash_notice,
            commands::logs::last_crash_report,
            commands::logs::current_log,
            #[cfg(desktop)]
            commands::logs::open_log_dir,
            #[cfg(desktop)]
            commands::export::export_library,
            #[cfg(desktop)]
            commands::export::cancel_library_export,
            commands::catalog::hires_twin_album,
            commands::blocks::blocked,
            commands::blocks::set_blocked,
            commands::collection::likes,
            commands::collection::set_liked,
            commands::collection::own_playlists,
            commands::collection::add_to_playlist,
            commands::collection::create_playlist,
            commands::collection::remove_from_playlist,
            commands::collection::remove_many_from_playlist,
            commands::collection::restore_many_to_playlist,
            commands::collection::set_tracks_liked,
            commands::collection::restore_to_playlist,
            commands::collection::move_in_playlist,
            commands::collection::edit_playlist,
            commands::collection::delete_playlist,
            commands::cleanup::playlist_cleanup,
            commands::cleanup::clean_playlist,
            commands::cleanup::undo_playlist_cleanup,
            commands::cleanup::liked_cleanup,
            commands::cleanup::clean_liked,
            commands::later::later_ids,
            commands::later::save_for_later,
            commands::later::set_saved_for_later,
            commands::project::open_project_page,
            commands::player::play_tracks,
            commands::player::play_next,
            commands::player::take_now_playing_request,
            commands::drive::take_drive_request,
            commands::shortcuts::take_pin_request,
            commands::drive::keep_screen_on,
            commands::player::add_to_queue,
            commands::player::enqueue_tracks,
            commands::player::remove_from_queue,
            commands::player::move_in_queue,
            commands::player::clear_queue,
            commands::player::restore_queue,
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
            commands::player::queue_track_ids,
            commands::settings::ui_settings,
            commands::settings::set_ui_settings,
            commands::settings::backup::export_settings,
            commands::settings::backup::import_settings,
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
            commands::settings::discord_available,
            commands::settings::exclusive_available,
            #[cfg(desktop)]
            commands::settings::set_autostart,
            commands::jam::jam_status,
            commands::jam::jam_online_available,
            commands::jam::start_jam,
            commands::jam::set_jam_rules,
            commands::jam::set_jam_adds_open,
            commands::jam::set_jam_guest_banned,
            commands::jam::stop_jam,
            commands::remote::remote_status,
            commands::remote::start_remote,
            commands::remote::stop_remote,
            commands::remote::renew_remote_code,
            #[cfg(desktop)]
            commands::window::fullscreen,
            #[cfg(desktop)]
            commands::window::set_fullscreen,
            #[cfg(desktop)]
            commands::mini_player::open_mini_player,
            #[cfg(desktop)]
            commands::mini_player::close_mini_player,
            #[cfg(desktop)]
            commands::mini_player::mini_player_on_top,
            #[cfg(desktop)]
            commands::mini_player::set_mini_player_on_top,
            #[cfg(desktop)]
            updater::update_status,
            #[cfg(desktop)]
            updater::updates_available,
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
            // Tauri quitte sans exécuter les destructeurs : dernière sauvegarde de la file
            // (#260), puis arrêt de mpv. Une mise à jour reportée (« Plus tard »)
            // s'installe à ce moment-là.
            // Quitter avec le mini-lecteur ouvert (#384) : sa position est gardée tant que la
            // fenêtre existe encore.
            #[cfg(desktop)]
            if let RunEvent::ExitRequested { .. } = _event {
                commands::mini_player::on_exit(_app);
            }
            #[cfg(desktop)]
            if let RunEvent::Exit = _event {
                flush_player(_app);
                _app.state::<Arc<ProcessGuard>>().kill();
                _app.state::<UpdateState>().install_on_exit();
            }
            #[cfg(target_os = "android")]
            android::keep_alive(_app, &_event);
        });
}

/// Sauvegarde immédiate de la file et de la position avant que l'app quitte (#260).
#[cfg(desktop)]
pub(crate) fn flush_player(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        player::flush_blocking(&state.player);
    }
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
            // Mini-lecteur (#384) : sa fermeture ramène la fenêtre principale.
            WindowEvent::CloseRequested { .. }
                if window.label() == commands::mini_player::LABEL =>
            {
                commands::mini_player::on_close_requested(window)
            }
            // Clic sur l'icône de la zone de notification : cacher si au premier plan (#226).
            WindowEvent::Focused(focused) if window.label() == "main" => tray::note_focus(*focused),
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

/// SMTC (touches média, panneau Windows) et boutons de la miniature, branchés sur la fenêtre principale, MPRIS sous Linux, et icône de
/// la zone de notification ; session média sur Android. Un échec est journalisé sans empêcher l'app de démarrer.
fn attach_integrations(app: &tauri::App, player: &tokio::sync::mpsc::Sender<player::Cmd>) {
    let mut observers = integrations::Observers::default();
    #[cfg(desktop)]
    match tray::create(app.handle(), player.clone()) {
        Ok(handle) => observers.tray = Some(handle),
        Err(e) => tracing::warn!("zone de notification indisponible : {e}"),
    }
    #[cfg(desktop)]
    if let Some(id) = integrations::discord::client_id() {
        observers.discord = Some(integrations::discord::start(id));
    }
    #[cfg(windows)]
    {
        let hwnd = app
            .get_webview_window("main")
            .ok_or_else(|| "fenêtre principale introuvable".to_owned())
            .and_then(|w| w.hwnd().map_err(|e| e.to_string()));
        match hwnd
            .clone()
            .and_then(|h| integrations::smtc::start(h.0 as isize, player.clone()))
        {
            Ok(smtc) => observers.smtc = Some(smtc),
            Err(e) => tracing::warn!("SMTC indisponible : {e}"),
        }
        // Boutons sous la miniature (#333) : `setup` tourne sur le thread de la fenêtre.
        match hwnd
            .and_then(|h| integrations::thumbbar::start(app.handle(), h.0 as isize, player.clone()))
        {
            Ok(bar) => observers.thumbbar = Some(bar),
            Err(e) => tracing::warn!("boutons de la miniature indisponibles : {e}"),
        }
    }
    #[cfg(target_os = "linux")]
    {
        observers.mpris = Some(integrations::mpris::start(
            app.handle().clone(),
            player.clone(),
        ));
    }
    #[cfg(target_os = "android")]
    {
        observers.media_session = Some(integrations::media_session::start(
            app.handle(),
            player.clone(),
        ));
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "android")))]
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

/// Événement émis quand TIDAL refuse le rafraîchissement en cours de route (#259) : l'UI
/// revient à l'écran de connexion.
const SESSION_LOST_EVENT: &str = "session-lost";

fn spawn_session_watch(app: tauri::AppHandle, tidal: Arc<TidalClient>) {
    use tauri::Emitter;
    tauri::async_runtime::spawn(async move {
        loop {
            tidal.session_lost().await;
            if let Err(e) = app.emit(SESSION_LOST_EVENT, ()) {
                tracing::warn!("émission de {SESSION_LOST_EVENT} impossible : {e}");
            }
        }
    });
}
