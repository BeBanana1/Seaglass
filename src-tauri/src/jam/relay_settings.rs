//! Relais réglable (#452) : la Jam en ligne et le tunnel des transferts (#430) passent par
//! un relais (`relay/`) que chacun peut héberger. Par défaut, celui compilé
//! (`servers::relay_base`) ; l'utilisateur peut en donner un autre, ou couper le relais.
//!
//! Le relais n'accepte que les hôtes qui présentent sa clé (`SEAGLASS_RELAY_KEY`, pas un
//! secret : elle écarte les robots). Un autre relais a la sienne, saisie avec l'adresse ;
//! sans clé saisie, la clé compilée est envoyée.
//!
//! Gardé à part des réglages d'interface (`relay.settings`) : `set_ui_settings` reçoit
//! l'objet entier de l'UI et contournerait la validation de l'adresse. Pas exporté par la
//! sauvegarde des réglages (#352). Un changement vaut pour la prochaine Jam, et pour le
//! tunnel à sa prochaine ouverture.

use std::sync::RwLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::relay::RelayConfig;
use crate::i18n::{tr, trf};
use crate::servers;
use crate::store::Store;

const KEY: &str = "relay.settings";
const MAX_URL_LEN: usize = 200;
const MAX_KEY_LEN: usize = 200;
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RelaySettings {
    /// Faux : ni Jam en ligne ni tunnel, même avec un relais compilé.
    pub enabled: bool,
    /// `https://…` sans `/` final ; `None` : relais compilé.
    pub url: Option<String>,
    /// Clé de ce relais ; `None` : clé compilée.
    pub key: Option<String>,
}

impl Default for RelaySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            url: None,
            key: None,
        }
    }
}

static SETTINGS: RwLock<Option<RelaySettings>> = RwLock::new(None);

/// Lit les réglages enregistrés (au lancement).
pub fn load(store: Option<&Store>) {
    let settings = store
        .and_then(|s| match s.get_setting::<RelaySettings>(KEY) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("relais : réglage illisible, valeurs par défaut ({e})");
                None
            }
        })
        .unwrap_or_default();
    set(settings);
}

fn set(settings: RelaySettings) {
    if let Ok(mut slot) = SETTINGS.write() {
        *slot = Some(settings);
    }
}

pub fn current() -> RelaySettings {
    SETTINGS
        .read()
        .ok()
        .and_then(|s| s.clone())
        .unwrap_or_default()
}

/// Relais à joindre maintenant, `None` sans relais (coupé, aucune adresse, aucune clé).
pub fn config() -> Option<RelayConfig> {
    let runtime = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    resolve(
        &current(),
        Overrides {
            url: runtime("SEAGLASS_RELAY_URL"),
            key: runtime("SEAGLASS_RELAY_KEY"),
        },
        servers::relay_base(),
        option_env!("SEAGLASS_SEAGLASS_RELAY_KEY"),
    )
}

/// Variables d'environnement au lancement (tests) : `SEAGLASS_RELAY_URL` (adresse
/// `ws(s)://…/host` complète) et `SEAGLASS_RELAY_KEY`.
#[derive(Default)]
struct Overrides {
    url: Option<String>,
    key: Option<String>,
}

fn resolve(
    settings: &RelaySettings,
    overrides: Overrides,
    compiled_url: Option<&str>,
    compiled_key: Option<&str>,
) -> Option<RelayConfig> {
    if !settings.enabled {
        return None;
    }
    let key = overrides
        .key
        .or_else(|| settings.key.clone())
        .or_else(|| compiled_key.map(str::to_owned))
        .filter(|k| !k.is_empty())?;
    let url = match overrides.url {
        Some(url) => url,
        None => host_url(settings.url.as_deref().or(compiled_url)?)?,
    };
    Some(RelayConfig { url, key })
}

/// `https://hôte[/chemin]` → `wss://hôte[/chemin]/host`.
fn host_url(base: &str) -> Option<String> {
    let rest = base.strip_prefix("https://")?;
    Some(format!("wss://{}/host", rest.trim_end_matches('/')))
}

/// Adresse saisie, ramenée à `https://hôte[:port][/chemin]` ; refusée sinon.
pub fn normalize(input: &str) -> Result<String, String> {
    let invalid = || {
        tr(
            "adresse du relais invalide : https://… attendu",
            "invalid relay address: https://… expected",
        )
        .to_owned()
    };
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.len() > MAX_URL_LEN {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(trimmed).map_err(|_| invalid())?;
    let clean = url.scheme() == "https"
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    if !clean {
        return Err(invalid());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// La clé part dans un en-tête HTTP : caractères visibles seulement.
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= MAX_KEY_LEN && key.bytes().all(|b| b.is_ascii_graphic())
}

/// Le relais répond-il ? `GET /healthz` doit renvoyer `ok`.
async fn check_health(base: &str) -> Result<(), String> {
    let unreachable = |detail: String| {
        trf!(
            "relais injoignable ({}) : vérifiez l'adresse",
            "relay unreachable ({}): check the address",
            detail
        )
    };
    let client = reqwest::Client::builder()
        .https_only(true)
        .timeout(HEALTH_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let body = client
        .get(format!("{base}/healthz"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| unreachable(e.to_string()))?
        .text()
        .await
        .map_err(|e| unreachable(e.to_string()))?;
    if body.trim() != "ok" {
        return Err(tr(
            "cette adresse ne répond pas comme un relais Seaglass",
            "this address does not answer like a Seaglass relay",
        )
        .to_owned());
    }
    Ok(())
}

/// Réglage du relais pour l'UI. La clé n'est jamais renvoyée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayView {
    pub enabled: bool,
    /// Adresse choisie ; `None` : celle compilée (`default_url`).
    pub url: Option<String>,
    pub default_url: Option<String>,
    /// Une clé propre à ce relais est enregistrée.
    pub custom_key: bool,
    /// Le relais est utilisable (adresse et clé connues).
    pub usable: bool,
}

pub fn view() -> RelayView {
    let settings = current();
    RelayView {
        enabled: settings.enabled,
        url: settings.url.clone(),
        default_url: servers::relay_base().map(str::to_owned),
        custom_key: settings.key.is_some(),
        usable: config().is_some(),
    }
}

/// Changement demandé par l'UI. `url` vide ou égale au relais compilé : relais compilé.
/// `key` : `None` garde la clé, vide revient à la clé compilée.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayChange {
    pub enabled: bool,
    pub url: Option<String>,
    pub key: Option<String>,
}

/// Valide, vérifie qu'un relais choisi répond, enregistre et applique.
pub async fn update(store: Option<&Store>, change: RelayChange) -> Result<RelayView, String> {
    let mut settings = current();
    settings.enabled = change.enabled;
    settings.url = match change.url.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(input) => {
            let url = normalize(input)?;
            (Some(url.as_str()) != servers::relay_base()).then_some(url)
        }
    };
    if let Some(key) = change.key.map(|k| k.trim().to_owned()) {
        if key.is_empty() {
            settings.key = None;
        } else if valid_key(&key) {
            settings.key = Some(key);
        } else {
            return Err(tr("clé du relais invalide", "invalid relay key").to_owned());
        }
    }
    if settings.enabled
        && let Some(url) = &settings.url
    {
        check_health(url).await?;
    }
    if let Some(store) = store {
        store
            .set_setting(KEY, &settings)
            .map_err(|e| e.to_string())?;
    }
    set(settings);
    Ok(view())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPILED: Option<&str> = Some("https://jam.example");
    const KEY: Option<&str> = Some("build-key");

    #[test]
    fn compiled_relay_is_the_default() {
        let c = resolve(
            &RelaySettings::default(),
            Overrides::default(),
            COMPILED,
            KEY,
        )
        .unwrap();
        assert_eq!(c.url, "wss://jam.example/host");
        assert_eq!(c.key, "build-key");
        assert_eq!(c.device_url(), "wss://jam.example/device");
    }

    #[test]
    fn chosen_relay_and_key_win() {
        let settings = RelaySettings {
            enabled: true,
            url: Some("https://relay.friend.org/seaglass".into()),
            key: Some("their-key".into()),
        };
        let c = resolve(&settings, Overrides::default(), COMPILED, KEY).unwrap();
        assert_eq!(c.url, "wss://relay.friend.org/seaglass/host");
        assert_eq!(c.key, "their-key");
        // Sans clé saisie : la clé compilée.
        let settings = RelaySettings {
            key: None,
            ..settings
        };
        let c = resolve(&settings, Overrides::default(), COMPILED, KEY).unwrap();
        assert_eq!(c.key, "build-key");
    }

    #[test]
    fn no_relay_without_address_key_or_when_disabled() {
        let off = RelaySettings {
            enabled: false,
            ..RelaySettings::default()
        };
        assert!(resolve(&off, Overrides::default(), COMPILED, KEY).is_none());
        let default = RelaySettings::default();
        assert!(resolve(&default, Overrides::default(), None, KEY).is_none());
        assert!(resolve(&default, Overrides::default(), COMPILED, None).is_none());
        assert!(resolve(&default, Overrides::default(), COMPILED, Some("")).is_none());
    }

    #[test]
    fn launch_overrides_keep_working() {
        let overrides = Overrides {
            url: Some("ws://127.0.0.1:47880/host".into()),
            key: Some("test".into()),
        };
        let c = resolve(&RelaySettings::default(), overrides, None, None).unwrap();
        assert_eq!(c.url, "ws://127.0.0.1:47880/host");
        assert_eq!(c.key, "test");
    }

    #[test]
    fn addresses_are_normalized_or_refused() {
        assert_eq!(
            normalize(" https://Jam.Example.org/ ").unwrap(),
            "https://jam.example.org"
        );
        assert_eq!(
            normalize("https://example.org:8443/relay/").unwrap(),
            "https://example.org:8443/relay"
        );
        for bad in [
            "",
            "jam.example.org",
            "http://jam.example.org",
            "wss://jam.example.org",
            "https://user:pw@jam.example.org",
            "https://jam.example.org/?x=1",
            "https://jam.example.org/#x",
            "ftp://jam.example.org",
        ] {
            assert!(normalize(bad).is_err(), "{bad}");
        }
        assert!(normalize(&format!("https://{}.org", "a".repeat(MAX_URL_LEN))).is_err());
    }

    #[test]
    fn keys_must_fit_in_a_header() {
        assert!(valid_key("abc-DEF_123"));
        assert!(!valid_key(""));
        assert!(!valid_key("avec espace"));
        assert!(!valid_key("é"));
        assert!(!valid_key(&"k".repeat(MAX_KEY_LEN + 1)));
    }

    #[test]
    fn stored_settings_default_missing_fields() {
        let s: RelaySettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, RelaySettings::default());
        let s: RelaySettings = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert!(!s.enabled);
    }
}
