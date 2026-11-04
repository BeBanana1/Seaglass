//! Pont natif Android de Seaglass (#24). Le code Kotlin vit dans `android/` ; l'app
//! l'appelle depuis Rust, jamais depuis l'UI (aucune commande ni permission exposée).

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

/// Faux entre la destruction de l'activité (app balayée des récents) et la reconstruction de
/// la fenêtre : posé par l'app (`android::keep_alive`).
static ACTIVITY: AtomicBool = AtomicBool::new(true);

/// L'activité existe-t-elle ? Les commandes du plugin s'exécutent dans son
/// `PluginManager` ; sans elle, wry panique (#169).
pub fn set_activity(present: bool) {
    ACTIVITY.store(present, Ordering::Release);
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn require_activity() -> Result<(), String> {
    if ACTIVITY.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("aucune activité (app fermée) : commande native ignorée".into())
    }
}

/// Accès au code Kotlin, géré par l'app (`app.state::<Native<R>>()`).
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
    /// URL de redirection complète, ou absente si l'utilisateur a fermé la page.
    #[serde(default)]
    url: Option<String>,
}

#[cfg(target_os = "android")]
impl<R: Runtime> Native<R> {
    /// Ouvre `url` dans une WebView plein écran, sans accès à l'IPC de l'app, et rend
    /// l'URL de la première navigation qui commence par `redirect_prefix` (bloquée).
    /// `None` si l'utilisateur ferme la page (bouton Retour).
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

    /// Appelle une commande Kotlin du plugin qui ne renvoie rien : canaux des événements
    /// (`engineStart`, `sessionStart`, au lancement) et installation d'une mise à jour
    /// (`installApk`, depuis l'UI). Refusée sans activité ; tout ce qui peut partir en cours
    /// de lecture passe par [`direct`]. Les arguments suivent les classes `@InvokeArg` de
    /// `SeaglassPlugin.kt`.
    pub async fn call(&self, command: &str, args: impl Serialize) -> Result<(), String> {
        require_activity()?;
        self.0
            .run_mobile_plugin_async::<serde::de::IgnoredAny>(command, args)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Raccourci : le pont natif de l'app.
#[cfg(target_os = "android")]
pub fn native<R: Runtime, M: Manager<R>>(manager: &M) -> tauri::State<'_, Native<R>> {
    manager.state::<Native<R>>()
}
