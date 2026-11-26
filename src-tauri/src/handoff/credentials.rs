//! Lien gardé dans le gestionnaire de secrets (Credential Manager, Keystore sur Android).

use std::sync::atomic::Ordering;

use super::Link;
use super::transport::PREFER_RELAY;
use crate::jam::remote::RelayRoute;

const KEYRING_SERVICE: &str = "seaglass";
const KEYRING_USER: &str = "handoff-link";
/// Adresse en ligne et clé du tunnel (#430), JSON de `RelayRoute`.
const KEYRING_RELAY: &str = "handoff-relay";

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

/// Ordinateur lié, `None` s'il n'y en a pas (ou si le lien gardé est abîmé).
pub async fn load() -> Result<Option<Link>, String> {
    blocking(|| {
        let link = match keyring_entry()?.get_password() {
            Ok(stored) => Link::parse(&stored),
            Err(keyring::Error::NoEntry) => None,
            Err(e) => return Err(e.to_string()),
        };
        let Some(link) = link else { return Ok(None) };
        // Chemin par le relais abîmé ou absent : le lien reste, sans lui.
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

/// Garde (ou oublie) le chemin par le relais de l'ordinateur lié.
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

/// `keyring` est synchrone (et bloquant sur Android, appel au Keystore).
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}
