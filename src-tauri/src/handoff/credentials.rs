//! Link kept in the credential store (Credential Manager, Keystore on Android).

use std::sync::atomic::Ordering;

use super::Link;
use super::transport::PREFER_RELAY;
use crate::jam::remote::RelayRoute;

const KEYRING_SERVICE: &str = "seaglass";
const KEYRING_USER: &str = "handoff-link";
/// Online address and tunnel key (#430), JSON of `RelayRoute`.
const KEYRING_RELAY: &str = "handoff-relay";

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

/// Linked computer, `None` if there is none (or if the stored link is damaged).
pub async fn load() -> Result<Option<Link>, String> {
    blocking(|| {
        let link = match keyring_entry()?.get_password() {
            Ok(stored) => Link::parse(&stored),
            Err(keyring::Error::NoEntry) => None,
            Err(e) => return Err(e.to_string()),
        };
        let Some(link) = link else { return Ok(None) };
        // Relay route damaged or missing: the link stays, without it.
        let relay = relay_entry()?
            .get_password()
            .ok()
            .and_then(|json| serde_json::from_str::<RelayRoute>(&json).ok());
        Ok(Some(link.with_relay(relay)))
    })
    .await
}

fn relay_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_RELAY).map_err(|e| e.to_string())
}

/// Keeps (or forgets) the relay route of the linked computer.
pub async fn save_relay(route: Option<RelayRoute>) -> Result<(), String> {
    blocking(move || {
        let entry = relay_entry()?;
        match route {
            Some(route) => {
                let json = serde_json::to_string(&route).map_err(|e| e.to_string())?;
                entry.set_password(&json).map_err(|e| e.to_string())
            }
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            },
        }
    })
    .await
}

pub async fn save(link: &Link) -> Result<(), String> {
    let stored = link.stored();
    blocking(move || {
        keyring_entry()?
            .set_password(&stored)
            .map_err(|e| e.to_string())
    })
    .await
}

pub async fn clear() -> Result<(), String> {
    save_relay(None).await?;
    PREFER_RELAY.store(false, Ordering::Relaxed);
    blocking(|| match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    })
    .await
}

/// `keyring` is synchronous (and blocking on Android, a call to the Keystore).
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}
