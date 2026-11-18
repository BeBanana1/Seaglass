//! Notifications du système (#435) : nouvelles sorties des artistes suivis.
//!
//! Code natif sur chaque plateforme, sans plugin : `tauri-plugin-notification` ne remonte pas
//! le clic sur le bureau, or la notification doit ouvrir l'album ou le fil.
//! - Windows : toast WinRT (`toast.rs`), sous l'AUMID de l'app, que l'installeur NSIS pose
//!   sur le raccourci du menu Démarrer. Le clic arrive tant que l'app tourne (`Activated`).
//! - Linux : `org.freedesktop.Notifications` par D-Bus (`dbus.rs`), action `default`.
//! - Android : canal « Nouvelles sorties » (`ReleaseNotifications.kt`) ; le clic passe par
//!   `ShareActivity`, comme un lien partagé.

#[cfg(target_os = "linux")]
mod dbus;
#[cfg(windows)]
mod toast;

use tauri::AppHandle;

use crate::commands::link::LinkTarget;

/// Notification prête à montrer, dans la langue de l'app.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    /// Page ouverte par un clic.
    pub target: LinkTarget,
}

/// Montre `notice` ; un échec ne fait que passer au journal.
pub async fn show(app: &AppHandle, notice: Notice) {
    #[cfg(windows)]
    let result = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || toast::show(&app, notice))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()))
    };
    #[cfg(target_os = "linux")]
    let result = dbus::show(app, notice).await.map_err(|e| e.to_string());
    #[cfg(target_os = "android")]
    let result = {
        let _ = app;
        android(notice).await
    };
    #[cfg(not(any(windows, target_os = "linux", target_os = "android")))]
    let result: Result<(), String> = {
        let _ = (app, notice);
        Ok(())
    };
    if let Err(e) = result {
        tracing::warn!("notification non affichée : {e}");
    }
}

/// Clic sur une notification du bureau : fenêtre au premier plan, page ouverte par l'UI.
#[cfg(any(windows, target_os = "linux"))]
fn clicked(app: &AppHandle, target: LinkTarget) {
    super::tray::show_main(app);
    crate::commands::link::open(app, target);
}

#[cfg(target_os = "android")]
async fn android(notice: Notice) -> Result<(), String> {
    let link = match &notice.target {
        LinkTarget::Album { id, .. } => format!("https://tidal.com/album/{id}"),
        _ => crate::commands::link::FEED_LINK.to_owned(),
    };
    let args = serde_json::json!({
        "title": notice.title,
        "body": notice.body,
        "link": link,
        "channel": crate::i18n::tr("Nouvelles sorties", "New releases"),
    });
    tauri_plugin_seaglass_mobile::direct::call::<serde::de::IgnoredAny>("notifyRelease", args)
        .await
        .map(|_| ())
}

/// Demande la permission d'afficher des notifications (Android 13+), à l'activation du
/// réglage ; ailleurs, rien à demander.
pub fn request_permission(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let native = tauri_plugin_seaglass_mobile::native(&app);
            if let Err(e) = native
                .call("requestNotifications", serde_json::json!({}))
                .await
            {
                tracing::warn!("permission des notifications non demandée : {e}");
            }
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

/// Texte sûr dans le contenu d'un élément XML (toast de Windows) ou dans le balisage simple
/// des serveurs de notifications de Linux (`body-markup`, qui n'a pas `&apos;`).
#[cfg_attr(not(any(windows, target_os = "linux", test)), allow(dead_code))]
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escapes_markup() {
        assert_eq!(
            escape(r#"Simon & Garfunkel <live> "73" l'album"#),
            r#"Simon &amp; Garfunkel &lt;live&gt; "73" l'album"#
        );
        assert_eq!(escape("Café"), "Café");
    }
}
