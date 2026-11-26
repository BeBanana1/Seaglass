//! System notifications (#435): new releases from followed artists.
//!
//! Native code on each platform, no plugin: `tauri-plugin-notification` does not report the click
//! on the desktop, and the notification must open the album or the feed.
//! - Windows: WinRT toast (`toast.rs`), under the app's AUMID, which the NSIS installer sets on the
//!   Start menu shortcut. The click arrives as long as the app runs (`Activated`).
//! - Linux: `org.freedesktop.Notifications` over D-Bus (`dbus.rs`), `default` action.
//! - Android: "New releases" channel (`ReleaseNotifications.kt`); the click goes through
//!   `ShareActivity`, like a shared link.

#[cfg(target_os = "linux")]
mod dbus;
#[cfg(windows)]
mod toast;

use tauri::AppHandle;

use crate::commands::link::LinkTarget;

/// Notification ready to show, in the app's language.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    /// Page opened by a click.
    pub target: LinkTarget,
}

/// Shows `notice`; a failure only goes to the log.
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
        tracing::warn!("notification not shown: {e}");
    }
}

/// Click on a desktop notification: window in the foreground, page opened by the UI.
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

/// Asks for the permission to show notifications (Android 13+), when the setting is turned on;
/// elsewhere, nothing to ask.
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
                tracing::warn!("notification permission not requested: {e}");
            }
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

/// Text safe in the content of an XML element (Windows toast) or in the simple markup of Linux
/// notification servers (`body-markup`, which has no `&apos;`).
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
