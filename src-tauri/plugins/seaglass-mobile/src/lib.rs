//! Seaglass native Android bridge (#24). The Kotlin code lives in `android/`; the app
//! calls it from Rust, never from the UI (no command or permission exposed).

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "android")]
pub mod direct;

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "dev.seaglass.mobile";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("seaglass-mobile")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            app.manage(Native(
                api.register_android_plugin(PLUGIN_IDENTIFIER, "SeaglassPlugin")?,
            ));
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}

/// False between the activity's destruction (app swiped away from the recents) and the
/// window's reconstruction: set by the app (`android::keep_alive`).
static ACTIVITY: AtomicBool = AtomicBool::new(true);

/// Does the activity exist? The plugin's commands run in its `PluginManager`; without it,
/// wry panics (#169).
pub fn set_activity(present: bool) {
    ACTIVITY.store(present, Ordering::Release);
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn require_activity() -> Result<(), String> {
    if ACTIVITY.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("no activity (app closed): native command ignored".into())
    }
}

/// Access to the Kotlin code, managed by the app (`app.state::<Native<R>>()`).
#[cfg(target_os = "android")]
pub struct Native<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginArgs<'a> {
    url: &'a str,
    redirect_prefix: &'a str,
}

#[derive(Deserialize)]
struct LoginResult {
    /// Full redirect URL, or absent if the user closed the page.
    #[serde(default)]
    url: Option<String>,
}

#[cfg(target_os = "android")]
impl<R: Runtime> Native<R> {
    /// Opens `url` in a full-screen WebView, without access to the app's IPC, and returns
    /// the URL of the first navigation that starts with `redirect_prefix` (blocked).
    /// `None` if the user closes the page (Back button).
    pub async fn login(&self, url: &str, redirect_prefix: &str) -> Result<Option<String>, String> {
        require_activity()?;
        self.0
            .run_mobile_plugin_async::<LoginResult>(
                "login",
                LoginArgs {
                    url,
                    redirect_prefix,
                },
            )
            .await
            .map(|r| r.url)
            .map_err(|e| e.to_string())
    }

    /// Calls a Kotlin plugin command that returns nothing: event channels (`engineStart`,
    /// `sessionStart`, at launch) and installing an update (`installApk`, from the UI).
    /// Refused without an activity; anything that can fire during playback goes through
    /// [`direct`]. The arguments follow the `@InvokeArg` classes of `SeaglassPlugin.kt`.
    pub async fn call(&self, command: &str, args: impl Serialize) -> Result<(), String> {
        require_activity()?;
        self.0
            .run_mobile_plugin_async::<serde::de::IgnoredAny>(command, args)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Shortcut: the app's native bridge.
#[cfg(target_os = "android")]
pub fn native<R: Runtime, M: Manager<R>>(manager: &M) -> tauri::State<'_, Native<R>> {
    manager.state::<Native<R>>()
}
