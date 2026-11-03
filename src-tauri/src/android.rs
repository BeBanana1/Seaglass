//! Initialisation propre à Android (#24).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, RunEvent, WebviewWindowBuilder};

/// Donne à `rustls-platform-verifier` la JVM et le contexte de l'activité : sans cela, toute
/// requête HTTPS de `reqwest` panique. À appeler avant la première requête.
pub fn init_tls() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    // SAFETY : pointeurs posés par tao au lancement de l'activité, valides pendant toute la
    // vie du processus.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY : référence globale tenue par ndk-context, jamais libérée par ce wrapper.
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| e.to_string())
}

/// Moment où l'activité a été détruite (app balayée des récents) : la fenêtre est à
/// recréer.
static WINDOW_LOST: Mutex<Option<Instant>> = Mutex::new(None);
/// tao signale la destruction avant d'oublier l'activité : reconstruire tout de suite
/// attacherait la fenêtre à l'activité mourante, dont la WebView plante et tue l'app.
const REBUILD_DELAY: Duration = Duration::from_secs(1);

/// Garde le processus en vie sans activité, pour que la musique continue (#24).
///
/// Balayer l'app des récents détruit l'activité ; tao en fait la fermeture de la dernière
/// fenêtre et Tauri quitterait alors le processus, lecteur compris, malgré le service de
/// lecture. On annule cette sortie ; quand Android recrée l'activité (app rouverte), tao
/// la laisse sans fenêtre et on y reconstruit la fenêtre principale de la config.
pub fn keep_alive(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::ExitRequested {
            code: None, api, ..
        } => {
            tracing::info!("activité détruite : le lecteur reste actif");
            if let Ok(mut lost) = WINDOW_LOST.lock() {
                *lost = Some(Instant::now());
            }
            api.prevent_exit();
        }
        // Émis après chaque lot d'événements, dont ceux de la nouvelle activité (création,
        // reprise) : rien à faire tant que la fenêtre n'est pas perdue.
        RunEvent::MainEventsCleared => {
            let Ok(mut lost) = WINDOW_LOST.lock() else {
                return;
            };
            if !lost.is_some_and(|at| at.elapsed() >= REBUILD_DELAY) {
                return;
            }
            if app.get_webview_window("main").is_some() {
                *lost = None;
                return;
            }
            let Some(config) = app.config().app.windows.first() else {
                return;
            };
            // Échoue tant qu'aucune activité nouvelle n'attend de fenêtre : réessayé au
            // lot suivant, sans journal.
            if let Ok(builder) = WebviewWindowBuilder::from_config(app, config)
                && builder.build().is_ok()
            {
                tracing::info!("activité recréée : fenêtre principale reconstruite");
                *lost = None;
            }
        }
        _ => {}
    }
}
