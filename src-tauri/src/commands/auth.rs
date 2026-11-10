//! Connexion, déconnexion et état de la session.

#[cfg(desktop)]
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State, Url};
#[cfg(desktop)]
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
#[cfg(desktop)]
use tokio::sync::oneshot;

use super::{AppState, CmdResult, err};
use crate::tidal::models::SessionInfo;
use crate::tidal::{TidalError, auth};

#[cfg(desktop)]
const LOGIN_WINDOW: &str = "login";

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> CmdResult<Option<SessionInfo>> {
    state.tidal.current_session().await.map_err(err)
}

/// Ouvre la page de connexion TIDAL et intercepte la redirection finale pour récupérer le
/// code PKCE, puis l'échange contre une session.
#[tauri::command]
pub async fn login(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionInfo> {
    let (challenge, url) = state.tidal.begin_login().map_err(err)?;
    let code = login_page(&app, url).await?;
    let code = code.ok_or_else(|| err(TidalError::LoginCancelled))?;
    let session = state
        .tidal
        .complete_login(&challenge, &code)
        .await
        .map_err(err)?;
    // Copies des listes d'un compte précédent (#181) : jamais montrées à celui-ci.
    super::snapshots::clear(&state).await;
    Ok(session)
}

/// Bureau : fenêtre dédiée, sans aucune capability Tauri. `None` si elle est fermée.
/// Navigation privée (#258) : les cookies de TIDAL ne survivent pas à la fenêtre, si bien
/// qu'une nouvelle connexion peut prendre un autre compte et que la session web ne reste
/// pas dans le profil WebView2 de l'app.
#[cfg(desktop)]
async fn login_page(app: &AppHandle, url: Url) -> CmdResult<Option<String>> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.destroy();
    }
    let (tx, rx) = oneshot::channel::<Option<String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let send = |tx: &Arc<Mutex<Option<oneshot::Sender<Option<String>>>>>, code| {
        if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
            let _ = tx.send(code);
        }
    };

    let nav_tx = Arc::clone(&tx);
    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title(crate::i18n::tr("Connexion", "Sign in"))
        .inner_size(480.0, 760.0)
        .incognito(true)
        .on_navigation(move |url| match auth::redirect_code(url) {
            Some(code) => {
                send(&nav_tx, Some(code));
                false
            }
            None => true,
        })
        .build()
        .map_err(err)?;
    let close_tx = Arc::clone(&tx);
    window.on_window_event(move |ev| {
        if matches!(
            ev,
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
        ) {
            send(&close_tx, None);
        }
    });

    let code = rx.await.ok().flatten();
    let _ = window.destroy();
    Ok(code)
}

/// Android : une seule WebView pour l'app, la page s'ouvre dans une WebView native à part
/// (plugin Kotlin, sans IPC Tauri). `None` si l'utilisateur revient en arrière.
#[cfg(target_os = "android")]
async fn login_page(app: &AppHandle, url: Url) -> CmdResult<Option<String>> {
    let redirect = tauri_plugin_seaglass_mobile::native(app)
        .login(url.as_str(), auth::REDIRECT_URI)
        .await?;
    Ok(redirect
        .and_then(|u| Url::parse(&u).ok())
        .and_then(|u| auth::redirect_code(&u)))
}

#[tauri::command]
pub async fn logout(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.tidal.logout().await.map_err(err)?;
    clear_login_data(&app);
    super::snapshots::clear(&state).await;
    super::search_history::clear(&state);
    super::play_history::clear(&state);
    Ok(())
}

/// Bureau : données de navigation du profil WebView2 de l'app (#258), où restent les cookies
/// de TIDAL des connexions faites avant la navigation privée. L'interface n'y garde rien
/// (ni cookies ni stockage web). Échec seulement journalisé : la déconnexion est faite.
#[cfg(desktop)]
fn clear_login_data(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main")
        && let Err(e) = window.clear_all_browsing_data()
    {
        tracing::warn!("données de navigation non effacées : {e}");
    }
}

/// Android : la WebView de connexion efface déjà ses cookies avant et après usage
/// (`SeaglassPlugin.login`).
#[cfg(target_os = "android")]
fn clear_login_data(_app: &AppHandle) {}
