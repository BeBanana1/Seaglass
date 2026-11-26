//! WinRT toast (#435). The AUMID is the app's identifier (`dev.seaglass.app`), set by Tauri's NSIS
//! installer on the Start menu shortcut: without that shortcut (app never installed), Windows
//! refuses the toast without an error.
//!
//! The click arrives through `Activated` as long as the app runs (in the notification area
//! included). Clicked later from the notification center with the app closed, it only starts the
//! app again: no COM activator is registered.

use tauri::AppHandle;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{HSTRING, IInspectable, Ref};

use super::{Notice, escape};

/// Blocking (WinRT): from `spawn_blocking`.
pub fn show(app: &AppHandle, notice: Notice) -> windows::core::Result<()> {
    // Already initialized on this thread: harmless error.
    // SAFETY: WinRT initialization for the current thread, no pointer.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(toast_xml(&notice)))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let handle = app.clone();
    let target = notice.target;
    toast.Activated(&TypedEventHandler::new(
        move |_: Ref<ToastNotification>, _: Ref<IInspectable>| {
            super::clicked(&handle, target.clone());
            Ok(())
        },
    ))?;
    let id = HSTRING::from(app.config().identifier.as_str());
    ToastNotificationManager::CreateToastNotifierWithId(&id)?.Show(&toast)
}

fn toast_xml(notice: &Notice) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text>\
         </binding></visual></toast>",
        escape(&notice.title),
        escape(&notice.body)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::link::LinkTarget;
    use windows::UI::Notifications::NotificationSetting;

    #[test]
    fn builds_generic_toast() {
        let notice = Notice {
            title: "Nouvelle sortie".into(),
            body: "A&B · <Live>".into(),
            target: LinkTarget::Feed,
        };
        assert_eq!(
            toast_xml(&notice),
            "<toast><visual><binding template=\"ToastGeneric\"><text>Nouvelle sortie</text>\
             <text>A&amp;B · &lt;Live&gt;</text></binding></visual></toast>"
        );
    }

    /// Real toast under the app's AUMID (installed once, for the shortcut): `cargo test live_toast
    /// -- --ignored`. `Setting` fails for an AUMID Windows does not know, where `Show` would
    /// succeed without showing anything.
    #[test]
    #[ignore]
    fn live_toast() {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let notice = Notice {
            title: "Nouvelle sortie".into(),
            body: "Seaglass · toast test (#435)".into(),
            target: LinkTarget::Feed,
        };
        let xml = XmlDocument::new().unwrap();
        xml.LoadXml(&HSTRING::from(toast_xml(&notice))).unwrap();
        let toast = ToastNotification::CreateToastNotification(&xml).unwrap();
        let notifier =
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("dev.seaglass.app"))
                .unwrap();
        assert_eq!(notifier.Setting().unwrap(), NotificationSetting::Enabled);
        notifier.Show(&toast).unwrap();
    }
}
