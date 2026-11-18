//! Notification de Linux (#435) : `org.freedesktop.Notifications` sur le bus de session,
//! comme `notify-send`. L'action `default` (clic sur la notification) arrive par le signal
//! `ActionInvoked` ; on l'attend jusqu'à la fermeture de la notification, au plus 24 h.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use tauri::AppHandle;
use zbus::zvariant::Value;
use zbus::{Connection, Proxy};

use super::{Notice, escape};
use crate::i18n::tr;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const WAIT: Duration = Duration::from_secs(24 * 3600);

pub async fn show(app: &AppHandle, notice: Notice) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    let proxy = Proxy::new(&conn, NAME, PATH, NAME).await?;
    // Abonnement avant l'envoi : un clic immédiat ne se perd pas.
    let mut invoked = proxy.receive_signal("ActionInvoked").await?;
    let mut closed = proxy.receive_signal("NotificationClosed").await?;
    let hints: HashMap<&str, Value<'_>> = HashMap::new();
    let actions = vec!["default", tr("Ouvrir", "Open")];
    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Seaglass",
                0u32,
                "",
                // Le résumé est du texte brut selon la spécification ; seul le corps peut
                // être lu comme balisage (`body-markup`).
                notice.title.as_str(),
                escape(&notice.body),
                actions,
                hints,
                -1i32,
            ),
        )
        .await?;
    let app = app.clone();
    let target = notice.target;
    tauri::async_runtime::spawn(async move {
        let wait = async {
            loop {
                tokio::select! {
                    Some(msg) = invoked.next() => {
                        if let Ok((nid, key)) = msg.body().deserialize::<(u32, String)>()
                            && nid == id
                        {
                            if key == "default" {
                                super::clicked(&app, target.clone());
                            }
                            return;
                        }
                    }
                    Some(msg) = closed.next() => {
                        if let Ok((nid, _)) = msg.body().deserialize::<(u32, u32)>()
                            && nid == id
                        {
                            return;
                        }
                    }
                    else => return,
                }
            }
        };
        let _ = tokio::time::timeout(WAIT, wait).await;
        // La connexion (tenue par le proxy) se ferme ici.
        drop(proxy);
    });
    Ok(())
}
