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
    // Copies des listes d'un compte précédent (#181) : jamais montrées à celui-ci.
    super::snapshots::clear(state).await;
    Ok(session)
}

/// Vrai si la connexion passe par le navigateur du système plutôt que par une fenêtre de
/// l'app. Linux (#17) : TIDAL sert une vérification anti-robot à WebKitGTK et finit par le
/// bloquer ; un navigateur courant (Firefox, Chrome…) passe sans difficulté.
#[tauri::command]
pub async fn login_in_browser() -> CmdResult<bool> {
    Ok(cfg!(target_os = "linux"))
}

/// Défi PKCE de la connexion par navigateur en cours, entre l'ouverture de la page et
/// l'adresse collée. Une seule connexion à la fois : en rouvrir une remplace le défi.
#[cfg(desktop)]
static BROWSER_LOGIN: Mutex<Option<auth::PkceChallenge>> = Mutex::new(None);

/// Longueur maximale de l'adresse collée : celle de TIDAL fait quelques centaines de
/// caractères.
#[cfg(desktop)]
const MAX_REDIRECT: usize = 4096;

/// Connexion par navigateur, étape 1 : ouvre la page de connexion TIDAL dans le navigateur
/// du système (adresse construite ici, jamais reçue de l'UI) et la renvoie, pour qu'elle
/// puisse être copiée à la main si aucun navigateur ne s'ouvre (WSL, système sans
/// navigateur par défaut). Elle ne contient que le défi PKCE, pas le vérificateur.
#[cfg(desktop)]
#[tauri::command]
pub async fn browser_login_start(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    use tauri_plugin_opener::OpenerExt;

    let (challenge, url) = state.tidal.begin_login().map_err(err)?;
    *BROWSER_LOGIN.lock().map_err(err)? = Some(challenge);
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        tracing::warn!("navigateur du système non ouvert : {e}");
    }
    Ok(url.into())
}

/// Connexion par navigateur, étape 2 : l'adresse de la page sur laquelle TIDAL a renvoyé
/// après la connexion. Seule une adresse de redirection attendue (`auth::redirect_code`)
/// est acceptée ; une adresse invalide laisse le défi en place pour un nouvel essai.
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
    let challenge = BROWSER_LOGIN.lock().map_err(err)?.take().ok_or_else(|| {
        crate::i18n::tr(
            "Ouvrez d'abord la page de connexion",
            "Open the sign-in page first",
        )
        .to_owned()
    })?;
    finish_login(&state, &challenge, &code).await
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
