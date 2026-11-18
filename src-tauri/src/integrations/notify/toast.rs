//! Toast WinRT (#435). L'AUMID est l'identifiant de l'app (`dev.seaglass.app`), posé par
//! l'installeur NSIS de Tauri sur le raccourci du menu Démarrer : sans ce raccourci (app
//! jamais installée), Windows refuse le toast sans erreur.
//!
//! Le clic arrive par `Activated` tant que l'app tourne (dans la zone de notification
//! compris). Touché plus tard depuis le centre de notifications, app fermée, il ne fait que
//! relancer l'app : il n'y a pas d'activateur COM enregistré.

use tauri::AppHandle;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{HSTRING, IInspectable, Ref};

use super::{Notice, escape};

/// Bloquant (WinRT) : depuis `spawn_blocking`.
pub fn show(app: &AppHandle, notice: Notice) -> windows::core::Result<()> {
    // Déjà initialisé sur ce thread : erreur sans conséquence.
    // SAFETY : initialisation de WinRT pour le thread courant, sans pointeur.
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

    /// Vrai toast sous l'AUMID de l'app (installée une fois, pour le raccourci) :
    /// `cargo test live_toast -- --ignored`. `Setting` échoue pour un AUMID inconnu de
    /// Windows, là où `Show` réussirait sans rien afficher.
    #[test]
    #[ignore]
    fn live_toast() {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let notice = Notice {
            title: "Nouvelle sortie".into(),
            body: "Seaglass · essai du toast (#435)".into(),
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
