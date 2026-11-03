//! Initialisation propre à Android (#24).

use std::any::Any;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi};
use serde::{Deserialize, Serialize};
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

/// Range les secrets de `keyring` (refresh token) dans l'Android Keystore, par le plugin
/// Kotlin (`SecretStore.kt`) : sans cela, `keyring` n'a sur Android qu'un magasin en
/// mémoire et la connexion est perdue à chaque lancement. À appeler avant le premier accès.
pub fn install_keystore(app: &AppHandle) {
    keyring::set_default_credential_builder(Box::new(KeystoreBuilder(app.clone())));
}

#[derive(Debug)]
struct KeystoreBuilder(AppHandle);

impl CredentialBuilderApi for KeystoreBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(KeystoreCredential {
            app: self.0.clone(),
            name: format!("{service}/{user}"),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl std::fmt::Debug for KeystoreCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeystoreCredential")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Un secret du Keystore. Les appels bloquent : `keyring` est synchrone, et ses appelants
/// (`tidal::auth`) tournent sur tokio, jamais sur le thread principal d'Android.
struct KeystoreCredential {
    app: AppHandle,
    name: String,
}

#[derive(Serialize)]
struct SecretArgs<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<&'a str>,
}

#[derive(Deserialize)]
struct SecretValue {
    value: Option<String>,
}

#[derive(Deserialize)]
struct SecretDeleted {
    existed: bool,
}

impl KeystoreCredential {
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        command: &str,
        value: Option<&str>,
    ) -> keyring::Result<T> {
        let args = SecretArgs {
            name: &self.name,
            value,
        };
        tauri_plugin_seaglass_mobile::native(&self.app)
            .call_blocking(command, args)
            .map_err(|e| keyring::Error::PlatformFailure(e.into()))
    }
}

impl CredentialApi for KeystoreCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        let value = std::str::from_utf8(secret)
            .map_err(|_| keyring::Error::BadEncoding(secret.to_vec()))?;
        self.call::<serde::de::IgnoredAny>("secretSet", Some(value))
            .map(drop)
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.call::<SecretValue>("secretGet", None)?
            .value
            .map(String::into_bytes)
            .ok_or(keyring::Error::NoEntry)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        if self.call::<SecretDeleted>("secretDelete", None)?.existed {
            Ok(())
        } else {
            Err(keyring::Error::NoEntry)
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
