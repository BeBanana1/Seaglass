//! Sign-in, sign-out and session state.

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

/// Opens the TIDAL sign-in page and intercepts the final redirect to retrieve the
/// PKCE code, then exchanges it for a session.
#[tauri::command]
pub async fn login(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionInfo> {
    let (challenge, url) = state.tidal.begin_login().map_err(err)?;
    let code = login_page(&app, url).await?;
    let code = code.ok_or_else(|| err(TidalError::LoginCancelled))?;
    finish_login(&state, &challenge, &code).await
}

async fn finish_login(
    state: &AppState,
    challenge: &auth::PkceChallenge,
    code: &str,
) -> CmdResult<SessionInfo> {
    let session = state
        .tidal
        .complete_login(challenge, code)
        .await
        .map_err(err)?;
    // Copies of a previous account's lists (#181): never shown to this one.
    super::snapshots::clear(state).await;
    Ok(session)
}

/// True if sign-in goes through the system browser rather than through a window of the
/// app. Linux (#17): TIDAL serves an anti-bot check to WebKitGTK and ends up blocking
/// it; a mainstream browser (Firefox, Chrome…) gets through without difficulty.
#[tauri::command]
pub async fn login_in_browser() -> CmdResult<bool> {
    Ok(cfg!(target_os = "linux"))
}

/// PKCE challenge of the browser sign-in in progress, between opening the page and the
/// pasted address. One sign-in at a time: opening another one replaces the challenge.
#[cfg(desktop)]
static BROWSER_LOGIN: Mutex<Option<auth::PkceChallenge>> = Mutex::new(None);

/// Maximum length of the pasted address: TIDAL's is a few hundred
/// characters.
#[cfg(desktop)]
const MAX_REDIRECT: usize = 4096;

/// Browser sign-in, step 1: opens the TIDAL sign-in page in the system browser
/// (address built here, never received from the UI) and returns it, so that it can
/// be copied by hand if no browser opens (WSL, system without a default
/// browser). It only contains the PKCE challenge, not the verifier.
#[cfg(desktop)]
#[tauri::command]
pub async fn browser_login_start(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    use tauri_plugin_opener::OpenerExt;

    let (challenge, url) = state.tidal.begin_login().map_err(err)?;
    *BROWSER_LOGIN.lock().map_err(err)? = Some(challenge);
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        tracing::warn!("system browser not opened: {e}");
    }
    Ok(url.into())
}

/// Browser sign-in, step 2: the address of the page TIDAL redirected to after
/// sign-in. Only an expected redirect address (`auth::redirect_code`)
/// is accepted. The challenge is only removed once the session is open: after a failure
/// (rejected address, expired code or one from another sign-in page), a new attempt
/// shows the real error instead of "open the page first".
#[cfg(desktop)]
#[tauri::command]
pub async fn browser_login_finish(
    state: State<'_, AppState>,
    redirect: String,
) -> CmdResult<SessionInfo> {
    let code = browser_redirect_code(&redirect).ok_or_else(|| {
        crate::i18n::tr(
            "Adresse non reconnue : copiez l'adresse complète de la page sur laquelle TIDAL vous a renvoyé",
            "Address not recognized: copy the full address of the page TIDAL sent you to",
        )
        .to_owned()
    })?;
    let challenge = BROWSER_LOGIN.lock().map_err(err)?.clone().ok_or_else(|| {
        crate::i18n::tr(
            "Ouvrez d'abord la page de connexion",
            "Open the sign-in page first",
        )
        .to_owned()
    })?;
    let session = finish_login(&state, &challenge, &code).await?;
    if let Ok(mut pending) = BROWSER_LOGIN.lock() {
        *pending = None;
    }
    Ok(session)
}

#[cfg(desktop)]
fn browser_redirect_code(redirect: &str) -> Option<String> {
    let redirect = redirect.trim();
    if redirect.len() > MAX_REDIRECT {
        return None;
    }
    Url::parse(redirect)
        .ok()
        .and_then(|u| auth::redirect_code(&u))
}

/// Desktop: dedicated window, without any Tauri capability. `None` if it is closed.
/// Private browsing (#258): TIDAL's cookies do not outlive the window, so that a new
/// sign-in can pick another account and the web session does not stay
/// in the app's WebView2 profile.
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

/// Android: a single WebView for the app, the page opens in a separate native WebView
/// (Kotlin plugin, without Tauri IPC). `None` if the user goes back.
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

/// Desktop: browsing data of the app's WebView2 profile (#258), where TIDAL's cookies from
/// sign-ins made before private browsing remain. The interface keeps nothing there
/// (no cookies or web storage). Failure is only logged: the sign-out is done.
#[cfg(desktop)]
fn clear_login_data(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main")
        && let Err(e) = window.clear_all_browsing_data()
    {
        tracing::warn!("browsing data not cleared: {e}");
    }
}

/// Android: the sign-in WebView already clears its cookies before and after use
/// (`SeaglassPlugin.login`).
#[cfg(target_os = "android")]
fn clear_login_data(_app: &AppHandle) {}

#[cfg(all(test, desktop))]
mod tests {
    use super::*;

    #[test]
    fn pasted_redirect_is_checked() {
        let ok = "  https://tidal.com/android/login/auth?code=abc123&state=na 
";
        assert_eq!(browser_redirect_code(ok).as_deref(), Some("abc123"));
        for bad in [
            "",
            "abc123",
            "https://evil.example/android/login/auth?code=abc123",
            "https://tidal.com/android/login/auth",
        ] {
            assert_eq!(browser_redirect_code(bad), None, "{bad}");
        }
        let long = format!(
            "https://tidal.com/android/login/auth?code={}",
            "a".repeat(MAX_REDIRECT)
        );
        assert_eq!(browser_redirect_code(&long), None);
    }
}
