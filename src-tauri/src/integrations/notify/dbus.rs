//! Linux notification (#435): `org.freedesktop.Notifications` on the session bus, like
//! `notify-send`. The `default` action (click on the notification) arrives through the
//! `ActionInvoked` signal; it is awaited until the notification closes, at most 24 h.

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
    // Subscribe before sending: an immediate click is not lost.
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
                // The summary is plain text according to the specification; only the body may be
                // read as markup (`body-markup`).
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
        // The connection (held by the proxy) closes here.
        drop(proxy);
    });
    Ok(())
}
