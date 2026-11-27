//! Android-specific initialization (#24).

use std::any::Any;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, RunEvent, WebviewWindowBuilder};
use tauri_plugin_seaglass_mobile::{direct, set_activity};

/// Gives `rustls-platform-verifier` the JVM and the activity context: without it, every
/// HTTPS request from `reqwest` panics. Call before the first request.
pub fn init_tls() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    // SAFETY: pointers set by tao when the activity launches, valid for the whole
    // life of the process.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY: global reference held by ndk-context, never released by this wrapper.
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| e.to_string())
}

/// System language (#186), ISO 639 code from `Locale.getDefault().getLanguage()` (`fr`, `en`…).
/// System class: found by `FindClass` even from a native thread.
pub fn system_language() -> Result<String, String> {
    use jni::objects::JString;
    use jni::{jni_sig, jni_str};

    let ctx = ndk_context::android_context();
    // SAFETY: JVM pointer set by tao at launch, valid for the whole life of the process.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> jni::errors::Result<String> {
        let locale = env
            .call_static_method(
                jni_str!("java/util/Locale"),
                jni_str!("getDefault"),
                jni_sig!("()Ljava/util/Locale;"),
                &[],
            )?
            .l()?;
        let language = env
            .call_method(
                &locale,
                jni_str!("getLanguage"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        env.cast_local::<JString>(language)?.try_to_string(env)
    })
    .map_err(|e| e.to_string())
}

/// Moment when the activity was destroyed (app swiped away from the recents): the window
/// has to be recreated.
static WINDOW_LOST: Mutex<Option<Instant>> = Mutex::new(None);
/// tao reports the destruction before forgetting the activity: rebuilding right away
/// would attach the window to the dying activity, whose WebView crashes and kills the app.
const REBUILD_DELAY: Duration = Duration::from_secs(1);

/// Keeps the process alive without an activity, so that the music goes on (#24).
///
/// Swiping the app away from the recents destroys the activity; tao turns that into the close of the last
/// window and Tauri would then quit the process, player included, despite the playback
/// service. We cancel that exit; when Android recreates the activity (app reopened), tao
/// leaves it without a window and we rebuild the main window from the config there.
pub fn keep_alive(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::ExitRequested {
            code: None, api, ..
        } => {
            tracing::info!("activity destroyed: the player stays active");
            // No more plugin commands until the new window: wry would panic.
            set_activity(false);
            if let Ok(mut lost) = WINDOW_LOST.lock() {
                *lost = Some(Instant::now());
            }
            api.prevent_exit();
        }
        // Emitted after each batch of events, including those of the new activity (creation,
        // resume): nothing to do as long as the window is not lost.
        RunEvent::MainEventsCleared => {
            let Ok(mut lost) = WINDOW_LOST.lock() else {
                return;
            };
            if !lost.is_some_and(|at| at.elapsed() >= REBUILD_DELAY) {
                return;
            }
            if app.get_webview_window("main").is_some() {
                set_activity(true);
                *lost = None;
                return;
            }
            let Some(config) = app.config().app.windows.first() else {
                return;
            };
            // Fails as long as no new activity is waiting for a window: retried on the next
            // batch, with no log.
            if let Ok(builder) = WebviewWindowBuilder::from_config(app, config)
                && builder.build().is_ok()
            {
                tracing::info!("activity recreated: main window rebuilt");
                set_activity(true);
                *lost = None;
            }
        }
        _ => {}
    }
}

/// Stores the `keyring` secrets (refresh token) in the Android Keystore, through the
/// Kotlin plugin (`SecretStore.kt`): without it, `keyring` only has an in-memory store on
/// Android and the sign-in is lost at every launch. Call before the first access.
/// Through direct JNI (#169): a token refresh can happen while the app is swiped away
/// from the recents, with no activity for the plugin commands.
pub fn install_keystore() {
    keyring::set_default_credential_builder(Box::new(KeystoreBuilder));
}

/// Crashes on the Kotlin side (#192, `CrashLog.kt`): installs the handler for uncaught
/// exceptions, which writes `crash.txt` in `dir`, and returns the last abnormal exit
/// reported by Android 11+ (native crash, ANR…) that has not been reported yet.
pub fn crash_setup(dir: &std::path::Path) -> Option<String> {
    #[derive(Serialize)]
    struct Args<'a> {
        dir: &'a str,
    }
    #[derive(Deserialize)]
    struct Setup {
        exit: Option<String>,
    }
    let dir = dir.to_string_lossy();
    match direct::call_blocking::<Setup>("crashSetup", Args { dir: &dir }) {
        Ok(setup) => setup.exit,
        Err(e) => {
            tracing::warn!("Kotlin crash log unavailable: {e}");
            None
        }
    }
}

#[derive(Debug)]
struct KeystoreBuilder;

impl CredentialBuilderApi for KeystoreBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(KeystoreCredential {
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

/// A Keystore secret. The calls block: `keyring` is synchronous, and its callers
/// (`tidal::auth`) run on tokio, never on Android's main thread.
struct KeystoreCredential {
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
        direct::call_blocking(command, args).map_err(|e| keyring::Error::PlatformFailure(e.into()))
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
