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
mod handoff;
mod i18n;
mod integrations;
mod jam;
mod logs;
#[cfg(any(target_os = "android", test))]
mod media3;
mod mpv;
mod player;
mod servers;
mod store;
mod tidal;
mod tls;
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

    // The desktop single instance must be the first plugin: this one comes after.
    // Opening links in the system browser (#188), called only from Rust
    // (`commands::project`): no permission of the plugin on the UI side.
    platform_builder(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        // Jam (#25): stopped until the host starts it.
        .manage(jam::JamState::default())
        // Remote control (#336): turned back on below if it was on.
        .manage(jam::remote::RemoteState::default())
        // Feed (#415): keys of the last feed read.
        .manage(commands::feed::FeedState::default())
        .setup(|app| {
            #[cfg(target_os = "android")]
            if let Err(e) = android::init_tls() {
                tracing::error!("Android HTTPS verification unavailable: {e}");
            }
            #[cfg(target_os = "android")]
            android::install_keystore();
            // Before the rest: a crash further on already has its log on disk (#192).
            logs::setup(app);
            let tidal = Arc::new(TidalClient::new()?);
            let store = open_store(app);
            // Before the player: its toasts and the TIDAL requests follow the language.
            let ui_settings = commands::settings::load_from(store.as_deref());
            i18n::set(ui_settings.language.resolve());
            ui_settings.apply_to_backend();
            // Before the remote control, whose tunnel goes through the relay (#452).
            jam::relay_settings::load(store.as_deref());
            // Sending listens to ListenBrainz (#431), if an account is connected.
            integrations::listenbrainz::start(app.handle().clone(), store.clone());
            // TIDAL play log (#34): listens reported so that they are counted.
            player::start_play_log(Arc::clone(&tidal), store.clone());
            commands::shortcuts::sync(&ui_settings.pins);
            #[cfg(desktop)]
            commands::settings::zoom::apply(app.handle(), ui_settings.zoom);
            #[cfg(desktop)]
            commands::settings::system::refresh_autostart(app.handle());
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
            // Resuming playback at launch (#353): never at session startup (`--minimized`) or after
            // a crash; desktop only.
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
            // The window is created hidden (`visible: false`): shown here, except at launch at
            // startup if the notification area icon exists.
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
            // After `AppState`, which it reads: new releases badge (#415).
            commands::feed::spawn_watch(app.handle().clone());
            #[cfg(desktop)]
            {
                // After `AppState`: the global shortcuts handler (#413) takes the player from it.
                integrations::hotkeys::init(app.handle());
                integrations::hotkeys::apply(app.handle(), &ui_settings.global_shortcuts);
                commands::remote::restore(app.handle());
                updater::spawn(app.handle().clone(), Arc::clone(&mpv_process));
                app.manage(mpv_process);
            }
            #[cfg(target_os = "android")]
            apk_update::spawn(app.handle().clone());
            // Listening history with the linked computer (#430), if there is one.
            handoff::history::spawn_at_launch(app.handle());
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
            commands::recap::recap,
            commands::recap::recap_periods,
            commands::smart_lists::smart_list,
            commands::catalog::library_tracks,
            commands::catalog::library_albums,
            commands::catalog::library_artists,
            commands::catalog::library_playlists,
            commands::catalog::home,
            commands::catalog::home_rest,
            commands::feed::feed,
            commands::feed::mark_feed_seen,
            commands::catalog::lyrics,
            commands::catalog::track_info,
            commands::catalog::album_credits,
            commands::catalog::contributor,
            commands::link::open_link,
            commands::import::import_playlist,
            commands::import::cancel_import,
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
            commands::folders::playlist_folders,
            commands::folders::create_folder,
            commands::folders::rename_folder,
            commands::folders::move_to_folder,
            commands::folders::delete_folder,
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
            commands::blind_test::blind_test_start,
            commands::blind_test::blind_test_answer,
            commands::blind_test::blind_test_replay,
            commands::blind_test::blind_test_reveal,
            commands::blind_test::blind_test_next,
            commands::blind_test::blind_test_quit,
            commands::blind_test::blind_test_state,
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
            commands::settings::global_shortcuts_status,
            commands::settings::backup::export_settings,
            commands::settings::backup::import_settings,
            commands::settings::language,
            commands::settings::set_language,
            commands::settings::set_home_layout,
            commands::settings::set_home_featured,
            commands::settings::reset_home_layout,
            commands::settings::set_pins,
            commands::settings::set_list_sort,
            commands::settings::playback::playback_settings,
            commands::settings::playback::set_playback_settings,
            #[cfg(target_os = "android")]
            commands::settings::playback::quality_cap,
            commands::settings::equalizer::parse_autoeq,
            commands::settings::playback::audio_devices,
            commands::settings::system::platform,
            commands::settings::system::autostart_enabled,
            commands::settings::system::discord_available,
            commands::listenbrainz::listenbrainz_status,
            commands::listenbrainz::listenbrainz_connect,
            commands::listenbrainz::listenbrainz_disconnect,
            commands::settings::system::exclusive_available,
            #[cfg(desktop)]
            commands::settings::system::set_autostart,
            commands::jam::jam_status,
            commands::jam::jam_online_available,
            commands::jam::start_jam,
            commands::jam::set_jam_rules,
            commands::jam::set_jam_adds_open,
            commands::jam::set_jam_guest_banned,
            commands::jam::stop_jam,
            commands::jam::relay_settings,
            commands::jam::set_relay_settings,
            servers::update_server,
            commands::remote::remote_status,
            commands::remote::start_remote,
            commands::remote::stop_remote,
            commands::remote::set_handoff_accepted,
            commands::handoff::linked_computer,
            commands::handoff::link_computer,
            commands::handoff::unlink_computer,
            commands::handoff::handoff_pull,
            commands::handoff::handoff_push,
            commands::handoff::sync_history,
            commands::remote::renew_remote_code,
            commands::remote::open_handoff_pairing,
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
        .expect("the application failed to launch")
        .run(|_app, _event| {
            // Tauri quits without running destructors: last save of the queue (#260), then mpv is
            // stopped. A postponed update ("Later") is installed at this point.
            // Quitting with the mini player open (#384): its position is kept while the window
            // still exists.
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

/// Saves the queue and the position at once before the app quits (#260).
#[cfg(desktop)]
pub(crate) fn flush_player(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        player::flush_blocking(&state.player);
    }
}

/// Desktop plugins and window behavior.
#[cfg(desktop)]
fn platform_builder(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        // A single instance (#83): launching Seaglass again, for example while it is minimized to
        // the notification area, shows the existing window again instead of opening a second
        // player. Registered first, as the plugin asks.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if tray::show_on_second_launch(args) {
                tray::show_main(app);
            }
        }))
        // Built-in public key (#489): `tauri.conf.json` has none. Without it, `updater` checks
        // nothing.
        .plugin({
            let builder = tauri_plugin_updater::Builder::new();
            match servers::updater_pubkey() {
                Some(key) => builder.pubkey(key),
                None => builder,
            }
            .build()
        })
        // Launch at startup (the user's `Run` key, current exe). Driven only by the `settings`
        // commands: no permission on the UI side. The app launched this way starts hidden in the
        // notification area (#83).
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg(tray::MINIMIZED_ARG)
                .build(),
        )
        .manage(UpdateState::default())
        // "Close the window" setting on "Minimize": the main window is hidden.
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. }
                if window.label() == "main" && tray::hide_on_close() =>
            {
                api.prevent_close();
                let _ = window.hide();
            }
            // Mini player (#384): closing it brings the main window back.
            WindowEvent::CloseRequested { .. }
                if window.label() == commands::mini_player::LABEL =>
            {
                commands::mini_player::on_close_requested(window)
            }
            // Click on the notification area icon: hide if in the foreground (#226).
            WindowEvent::Focused(focused) if window.label() == "main" => tray::note_focus(*focused),
            // Fullscreen (#131) changes the size: the UI follows the window's actual state.
            WindowEvent::Resized(_) => commands::window::on_resized(window),
            _ => {}
        })
}

/// Android: no notification area, no autostart, no Tauri updater (updates through
/// `apk_update`); the native Kotlin bridge instead.
#[cfg(mobile)]
fn platform_builder(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(tauri_plugin_seaglass_mobile::init())
        .manage(apk_update::UpdateState::default());
    builder
}

/// SMTC (media keys, Windows panel) and thumbnail buttons, wired to the main window, MPRIS
/// on Linux, and the notification area icon; media session on Android. A failure is logged
/// without stopping the app from starting.
fn attach_integrations(app: &tauri::App, player: &tokio::sync::mpsc::Sender<player::Cmd>) {
    let mut observers = integrations::Observers::default();
    #[cfg(desktop)]
    match tray::create(app.handle(), player.clone()) {
        Ok(handle) => observers.tray = Some(handle),
        Err(e) => tracing::warn!("notification area unavailable: {e}"),
    }
    #[cfg(desktop)]
    if let Some(id) = integrations::discord::client_id() {
        observers.discord = Some(integrations::discord::start(id));
    }
    #[cfg(windows)]
    {
        let hwnd = app
            .get_webview_window("main")
            .ok_or_else(|| "main window not found".to_owned())
            .and_then(|w| w.hwnd().map_err(|e| e.to_string()));
        match hwnd
            .clone()
            .and_then(|h| integrations::smtc::start(h.0 as isize, player.clone()))
        {
            Ok(smtc) => observers.smtc = Some(smtc),
            Err(e) => tracing::warn!("SMTC unavailable: {e}"),
        }
        // Buttons under the thumbnail (#333): `setup` runs on the window's thread.
        match hwnd
            .and_then(|h| integrations::thumbbar::start(app.handle(), h.0 as isize, player.clone()))
        {
            Ok(bar) => observers.thumbbar = Some(bar),
            Err(e) => tracing::warn!("thumbnail buttons unavailable: {e}"),
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
        tracing::warn!("integrations not attached: player unavailable");
    }
}

/// SQLite database in the app's data folder; the app works without it, with no persistence.
fn open_store(app: &tauri::App) -> Option<Arc<Store>> {
    let open = || -> anyhow::Result<Store> {
        let dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&dir)?;
        Ok(Store::open(&dir.join(DB_FILE))?)
    };
    match open() {
        Ok(store) => Some(Arc::new(store)),
        Err(e) => {
            tracing::error!("SQLite database unavailable, no persistence: {e}");
            None
        }
    }
}

/// Event sent when TIDAL refuses the refresh along the way (#259): the UI goes back to the
/// sign-in screen.
const SESSION_LOST_EVENT: &str = "session-lost";

fn spawn_session_watch(app: tauri::AppHandle, tidal: Arc<TidalClient>) {
    use tauri::Emitter;
    tauri::async_runtime::spawn(async move {
        loop {
            tidal.session_lost().await;
            if let Err(e) = app.emit(SESSION_LOST_EVENT, ()) {
                tracing::warn!("cannot emit {SESSION_LOST_EVENT}: {e}");
            }
        }
    });
}
