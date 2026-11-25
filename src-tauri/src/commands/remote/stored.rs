//! Where the remote's settings live (#61): the switches in SQLite, the owner code and the
//! tunnel identity (relay identifier, secret, end-to-end key, #430) in the system's
//! credential store, like the TIDAL token. Together they reach the PC's handoff and history
//! routes from anywhere through the relay.
//!
//! Values still found in SQLite are the newest: settings from before 0.5.120, or written
//! there because the credential store refused them. They move to the credential store as
//! soon as it takes them. A credential store that cannot be **read** is an error, never
//! "no value": a new code would otherwise be drawn and every phone silently unpaired.

use serde::{Deserialize, Serialize};

use crate::i18n::trf;
use crate::jam::remote::{self, Features, TunnelSettings};
use crate::store::Store;

const KEY: &str = "remote";
const VAULT_SERVICE: &str = "seaglass";
const VAULT_USER: &str = "remote-link";

/// The remote's settings, as the commands use them. The SQLite record has the same shape,
/// with `code` and `tunnel` empty unless the credential store refused them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoteSettings {
    /// Remote turned on.
    pub enabled: bool,
    /// Playback handoffs accepted from the Android app (#414).
    #[serde(default)]
    pub handoff: bool,
    /// Owner code; drawn the first time it is turned on, kept afterwards.
    pub code: Option<String>,
    /// Identity on the relay and key shared with the phone (#430); drawn the first time
    /// it is turned on, the key changed with the code and at each "Link a phone" (#62).
    #[serde(default)]
    pub tunnel: Option<TunnelSettings>,
}

impl RemoteSettings {
    pub fn features(&self) -> Features {
        Features {
            remote: self.enabled,
            handoff: self.handoff,
        }
    }

    /// Damaged values (edited by hand) are dropped: new ones get drawn.
    fn checked(self) -> Self {
        Self {
            code: self.code.filter(|c| remote::is_code(c)),
            tunnel: self.tunnel.and_then(TunnelSettings::checked),
            ..self
        }
    }

    fn without_secrets(&self) -> Self {
        Self {
            code: None,
            tunnel: None,
            ..self.clone()
        }
    }
}

/// What the credential store holds, as JSON.
#[derive(Serialize, Deserialize)]
struct Link {
    code: String,
    tunnel: TunnelSettings,
}

/// The system's credential store, or a fake in the tests.
pub trait Vault {
    /// `Ok(None)` when there is no entry.
    fn read(&self) -> Result<Option<String>, String>;
    fn write(&self, value: &str) -> Result<(), String>;
}

/// Credential Manager on Windows, Secret Service on Linux.
pub struct Keyring;

impl Keyring {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(VAULT_SERVICE, VAULT_USER).map_err(|e| e.to_string())
    }
}

impl Vault for Keyring {
    fn read(&self) -> Result<Option<String>, String> {
        match Self::entry()?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn write(&self, value: &str) -> Result<(), String> {
        Self::entry()?
            .set_password(value)
            .map_err(|e| e.to_string())
    }
}

fn read_stored(store: Option<&Store>) -> RemoteSettings {
    store
        .and_then(|s| match s.get_setting::<RemoteSettings>(KEY) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("Remote: unreadable setting ({e})");
                None
            }
        })
        .unwrap_or_default()
}

/// The settings: switches from SQLite, code and tunnel from the credential store, or from
/// SQLite if they are still there (then moved). Error if the credential store cannot be
/// read.
pub fn load(store: Option<&Store>, vault: &dyn Vault) -> Result<RemoteSettings, String> {
    let stored = read_stored(store);
    if stored.code.is_some() || stored.tunnel.is_some() {
        let stored = stored.checked();
        save(store, vault, &stored)?;
        return Ok(stored);
    }
    let unreadable = |e: String| {
        trf!(
            "trousseau du système illisible, code de la télécommande inaccessible : {}",
            "the system's credential store cannot be read, no access to the remote's code: {}",
            e
        )
    };
    let link = match vault.read().map_err(unreadable)? {
        Some(json) => match serde_json::from_str::<Link>(&json) {
            Ok(link) => Some(link),
            Err(e) => {
                tracing::warn!("Remote: damaged entry in the credential store ({e})");
                None
            }
        },
        None => None,
    };
    Ok(match link {
        Some(link) => RemoteSettings {
            code: Some(link.code),
            tunnel: Some(link.tunnel),
            ..stored
        },
        None => stored,
    }
    .checked())
}

/// Keeps `settings`: code and tunnel in the credential store, the rest in SQLite; all in
/// SQLite if the credential store refuses them (logged), as before 0.5.120.
pub fn save(
    store: Option<&Store>,
    vault: &dyn Vault,
    settings: &RemoteSettings,
) -> Result<(), String> {
    let Some(store) = store else {
        return Ok(());
    };
    let record = match (&settings.code, &settings.tunnel) {
        (Some(code), Some(tunnel)) => {
            let link = Link {
                code: code.clone(),
                tunnel: tunnel.clone(),
            };
            let written = serde_json::to_string(&link)
                .map_err(|e| e.to_string())
                .and_then(|json| vault.write(&json));
            match written {
                Ok(()) => settings.without_secrets(),
                Err(e) => {
                    tracing::warn!(
                        "Remote: the credential store refused the code ({e}), kept in the settings"
                    );
                    settings.clone()
                }
            }
        }
        // Not drawn yet: nothing secret to move.
        _ => settings.clone(),
    };
    store.set_setting(KEY, &record).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeVault {
        value: Mutex<Option<String>>,
        refuse_writes: bool,
        unreadable: bool,
    }

    impl Vault for FakeVault {
        fn read(&self) -> Result<Option<String>, String> {
            if self.unreadable {
                return Err("locked".into());
            }
            Ok(self.value.lock().unwrap().clone())
        }

        fn write(&self, value: &str) -> Result<(), String> {
            if self.refuse_writes {
                return Err("no backend".into());
            }
            *self.value.lock().unwrap() = Some(value.to_owned());
            Ok(())
        }
    }

    fn full() -> RemoteSettings {
        RemoteSettings {
            enabled: true,
            handoff: true,
            code: Some(remote::new_code()),
            tunnel: Some(TunnelSettings::new()),
        }
    }

    fn record(store: &Store) -> RemoteSettings {
        store.get_setting(KEY).unwrap().unwrap()
    }

    #[test]
    fn secrets_go_to_the_credential_store() {
        let store = Store::in_memory().unwrap();
        let vault = FakeVault::default();
        let settings = full();
        save(Some(&store), &vault, &settings).unwrap();
        assert_eq!(record(&store), settings.without_secrets());
        assert!(vault.value.lock().unwrap().is_some());
        assert_eq!(load(Some(&store), &vault).unwrap(), settings);
    }

    #[test]
    fn settings_from_before_move_to_the_credential_store() {
        let store = Store::in_memory().unwrap();
        let settings = full();
        store.set_setting(KEY, &settings).unwrap();
        let vault = FakeVault::default();
        assert_eq!(load(Some(&store), &vault).unwrap(), settings);
        assert_eq!(record(&store), settings.without_secrets());
        // Read again from the credential store alone.
        assert_eq!(load(Some(&store), &vault).unwrap(), settings);
    }

    #[test]
    fn values_left_in_sqlite_win_over_an_older_entry() {
        let store = Store::in_memory().unwrap();
        let vault = FakeVault::default();
        save(Some(&store), &vault, &full()).unwrap();
        // Written later to SQLite while the credential store refused it.
        let newer = full();
        store.set_setting(KEY, &newer).unwrap();
        assert_eq!(load(Some(&store), &vault).unwrap(), newer);
        assert_eq!(record(&store), newer.without_secrets());
        assert_eq!(load(Some(&store), &vault).unwrap(), newer);
    }

    #[test]
    fn a_refusing_credential_store_keeps_them_in_sqlite() {
        let store = Store::in_memory().unwrap();
        let vault = FakeVault {
            refuse_writes: true,
            ..FakeVault::default()
        };
        let settings = full();
        save(Some(&store), &vault, &settings).unwrap();
        assert_eq!(record(&store), settings);
        assert_eq!(load(Some(&store), &vault).unwrap(), settings);
    }

    #[test]
    fn an_unreadable_credential_store_is_an_error() {
        let store = Store::in_memory().unwrap();
        store.set_setting(KEY, &full().without_secrets()).unwrap();
        let vault = FakeVault {
            unreadable: true,
            ..FakeVault::default()
        };
        assert!(load(Some(&store), &vault).is_err());
    }

    #[test]
    fn damaged_values_are_dropped() {
        let store = Store::in_memory().unwrap();
        let vault = FakeVault::default();
        *vault.value.lock().unwrap() = Some("{".into());
        store.set_setting(KEY, &full().without_secrets()).unwrap();
        let loaded = load(Some(&store), &vault).unwrap();
        assert_eq!((loaded.code, loaded.tunnel), (None, None));
        let edited = RemoteSettings {
            code: Some("AB23CD".into()),
            ..full()
        };
        store.set_setting(KEY, &edited).unwrap();
        assert_eq!(load(Some(&store), &vault).unwrap().code, None);
    }

    #[test]
    fn nothing_drawn_yet_stays_in_sqlite() {
        let store = Store::in_memory().unwrap();
        let vault = FakeVault::default();
        let off = RemoteSettings::default();
        save(Some(&store), &vault, &off).unwrap();
        assert_eq!(record(&store), off);
        assert!(vault.value.lock().unwrap().is_none());
    }
}
