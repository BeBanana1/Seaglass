//! Mises à jour de l'app Android (#24) : l'updater de Tauri ne gère pas Android.
//!
//! Le manifeste `android.json` (écrit par `scripts/release-android.ps1`) est lu 20 s après
//! le démarrage puis toutes les 6 h, jamais en debug (un APK de debug, signé par une autre
//! clé, ne peut pas être remplacé par une release). Rien n'est téléchargé d'office
//! (données mobiles) : l'UI reçoit `update-ready` et propose « Télécharger et installer ».
//! `install_update` télécharge l'APK dans le cache de l'app, vérifie son SHA-256, puis le
//! plugin Kotlin le confie à `PackageInstaller` : Android demande confirmation et vérifie
//! lui-même que l'APK porte la même clé de signature que l'app installée.
//!
//! Logique pure (manifeste, versions) testée aussi sur le bureau.

// Hors Android, seuls les tests utilisent ce module.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use serde::{Deserialize, Serialize};

/// Manifeste publié à côté de l'installeur Windows.
pub const MANIFEST_URL: &str = "https://updates.bebanana.ch/seaglass/android.json";
/// Seuls les APK servis sous ce préfixe sont acceptés, quel que soit le manifeste.
const DOWNLOAD_PREFIX: &str = "https://updates.bebanana.ch/seaglass/";
/// Au-delà, le manifeste est refusé (l'APK release fait quelques dizaines de Mo).
const MAX_SIZE: u64 = 300 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Version disponible et ce qu'il faut pour l'installer.
#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    pub version: String,
    pub url: String,
    /// En minuscules, 64 caractères hexadécimaux.
    pub sha256: String,
    pub size: u64,
    pub notes: Option<String>,
}

/// Même forme que l'`UpdateInfo` du bureau, avec la taille : sur Android la mise à jour
/// est proposée avant d'être téléchargée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    /// Taille de l'APK en octets.
    pub size: u64,
}

impl Available {
    pub fn info(&self, current: &str) -> UpdateInfo {
        UpdateInfo {
            version: self.version.clone(),
            current_version: current.to_owned(),
            notes: self.notes.clone(),
            size: self.size,
        }
    }
}

/// `x.y.z` en triplet comparable ; `None` pour toute autre forme.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.trim().split('.').map(|p| {
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<u64>().ok())
            .flatten()
    });
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Valide le manifeste et renvoie la version qu'il propose si elle est plus récente que
/// `current`.
pub fn evaluate(manifest: Manifest, current: &str) -> Result<Option<Available>, String> {
    let offered = parse_version(&manifest.version)
        .ok_or_else(|| format!("version invalide dans le manifeste : {}", manifest.version))?;
    let installed =
        parse_version(current).ok_or_else(|| format!("version de l'app invalide : {current}"))?;
    if offered <= installed {
        return Ok(None);
    }
    if !manifest.url.starts_with(DOWNLOAD_PREFIX) || manifest.url.contains("..") {
        return Err(format!("adresse de l'APK refusée : {}", manifest.url));
    }
    let sha256 = manifest.sha256.to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("empreinte SHA-256 invalide dans le manifeste".to_owned());
    }
    if manifest.size == 0 || manifest.size > MAX_SIZE {
        return Err(format!("taille de l'APK invalide : {}", manifest.size));
    }
    Ok(Some(Available {
        version: manifest.version.trim().to_owned(),
        url: manifest.url,
        sha256,
        size: manifest.size,
        notes: manifest.notes.filter(|n| !n.trim().is_empty()),
    }))
}

/// Empreinte en hexadécimal minuscule.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(target_os = "android")]
pub use android::*;

#[cfg(target_os = "android")]
mod android {
    use std::sync::Mutex;
    use std::time::Duration;

    use sha2::{Digest, Sha256};
    use tauri::{AppHandle, Emitter, Manager, State};
    use tokio::io::AsyncWriteExt;

    use super::{Available, MANIFEST_URL, Manifest, UpdateInfo, evaluate, hex};
    use crate::i18n::tr;

    const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
    const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
    const APK_FILE: &str = "update.apk";
    /// Erreur de `ApkInstaller.kt` : droit « Installer des applis inconnues » absent.
    const NEEDS_PERMISSION: &str = "install-permission";

    #[derive(Default)]
    pub struct UpdateState {
        available: Mutex<Option<Available>>,
        /// Sérialise la tâche de fond, le bouton « Vérifier » et l'installation.
        busy: tokio::sync::Mutex<()>,
    }

    impl UpdateState {
        fn get(&self) -> Option<Available> {
            self.available.lock().ok()?.clone()
        }

        fn set(&self, value: Option<Available>) {
            if let Ok(mut slot) = self.available.lock() {
                *slot = value;
            }
        }
    }

    fn current_version(app: &AppHandle) -> String {
        app.package_info().version.to_string()
    }

    fn http() -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())
    }

    pub fn spawn(app: AppHandle) {
        if cfg!(debug_assertions) {
            return;
        }
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FIRST_CHECK_DELAY).await;
            loop {
                match check(&app).await {
                    Ok(Some(info)) => {
                        let _ = app.emit("update-ready", info);
                    }
                    Ok(None) => {}
                    Err(e) => tracing::warn!("vérification des mises à jour : {e}"),
                }
                tokio::time::sleep(CHECK_INTERVAL).await;
            }
        });
    }

    /// Lit le manifeste. Renvoie la version nouvellement trouvée (`None` si l'app est à
    /// jour ou si cette version était déjà proposée).
    async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
        let state = app.state::<UpdateState>();
        let _busy = state.busy.lock().await;
        let manifest: Manifest = http()?
            .get(MANIFEST_URL)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| format!("manifeste illisible : {e}"))?;
        let current = current_version(app);
        let found = evaluate(manifest, &current)?;
        let known = state.get();
        state.set(found.clone());
        Ok(found
            .filter(|f| known.as_ref() != Some(f))
            .map(|f| f.info(&current)))
    }

    #[tauri::command]
    pub async fn check_for_updates(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
        check(&app).await?;
        let info = app
            .state::<UpdateState>()
            .get()
            .map(|a| a.info(&current_version(&app)));
        if let Some(info) = &info {
            let _ = app.emit("update-ready", info);
        }
        Ok(info)
    }

    #[tauri::command]
    pub fn update_status(app: AppHandle, state: State<'_, UpdateState>) -> Option<UpdateInfo> {
        state.get().map(|a| a.info(&current_version(&app)))
    }

    /// Télécharge l'APK, vérifie taille et SHA-256, puis ouvre la boîte d'installation
    /// d'Android. Se termine quand la boîte est affichée ; l'installation remplace le
    /// processus.
    #[tauri::command]
    pub async fn install_update(app: AppHandle) -> Result<(), String> {
        let state = app.state::<UpdateState>();
        let _busy = state.busy.lock().await;
        let update = state
            .get()
            .ok_or_else(|| tr("aucune mise à jour disponible", "no update available").to_owned())?;
        let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
        let path = dir.join(APK_FILE);
        let result = download(&update, &path).await;
        if let Err(e) = result {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(e);
        }
        tracing::info!(
            "APK {} téléchargé ({} octets, SHA-256 vérifié)",
            update.version,
            update.size
        );
        let apk = path.to_string_lossy().into_owned();
        let installed = tauri_plugin_seaglass_mobile::native(&app)
            .call("installApk", serde_json::json!({ "path": apk }))
            .await;
        installed.map_err(|e| {
            // Le Kotlin efface l'APK après l'avoir copié dans la session ; sinon, ici.
            let _ = std::fs::remove_file(&path);
            if e.contains(NEEDS_PERMISSION) {
                tr(
                    "Autorisez Seaglass à installer des applications dans le réglage ouvert, puis réessayez.",
                    "Allow Seaglass to install apps in the settings page that opened, then try again.",
                )
                .to_owned()
            } else {
                e
            }
        })
    }

    async fn download(update: &Available, path: &std::path::Path) -> Result<(), String> {
        let mut response = http()?
            .get(&update.url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?;
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            written += chunk.len() as u64;
            if written > update.size {
                return Err(tr("APK plus gros qu'annoncé", "APK larger than announced").to_owned());
            }
            hasher.update(&chunk);
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        if written != update.size || hex(&hasher.finalize()) != update.sha256 {
            return Err(tr(
                "APK altéré : empreinte différente du manifeste",
                "corrupted APK: checksum does not match the manifest",
            )
            .to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> Manifest {
        Manifest {
            version: version.to_owned(),
            url: format!("{DOWNLOAD_PREFIX}{version}/Seaglass_{version}_arm64.apk"),
            sha256: "AB".repeat(32),
            size: 40_000_000,
            notes: Some("- Correctif".to_owned()),
        }
    }

    #[test]
    fn versions_parse_strictly() {
        assert_eq!(parse_version("0.4.47"), Some((0, 4, 47)));
        assert_eq!(parse_version(" 1.10.0 "), Some((1, 10, 0)));
        for bad in ["0.4", "0.4.47.1", "0.4.x", "v0.4.47", "0..1", "", "0.4.-1"] {
            assert_eq!(parse_version(bad), None, "{bad}");
        }
    }

    #[test]
    fn newer_version_is_offered() {
        let found = evaluate(manifest("0.4.47"), "0.4.46").unwrap().unwrap();
        assert_eq!(found.version, "0.4.47");
        assert_eq!(found.sha256, "ab".repeat(32));
        // Comparaison numérique, pas lexicale.
        assert!(evaluate(manifest("0.4.100"), "0.4.99").unwrap().is_some());
        assert!(evaluate(manifest("0.5.0"), "0.4.99").unwrap().is_some());
    }

    #[test]
    fn same_or_older_version_is_ignored() {
        assert_eq!(evaluate(manifest("0.4.46"), "0.4.46").unwrap(), None);
        assert_eq!(evaluate(manifest("0.4.45"), "0.4.46").unwrap(), None);
    }

    #[test]
    fn foreign_or_malformed_download_is_refused() {
        let mut m = manifest("0.4.47");
        m.url = "https://example.com/seaglass.apk".to_owned();
        assert!(evaluate(m, "0.4.46").is_err());
        let mut m = manifest("0.4.47");
        m.url = "http://updates.bebanana.ch/seaglass/x.apk".to_owned();
        assert!(evaluate(m, "0.4.46").is_err());
        let mut m = manifest("0.4.47");
        m.url = format!("{DOWNLOAD_PREFIX}../autre/x.apk");
        assert!(evaluate(m, "0.4.46").is_err());
        let mut m = manifest("0.4.47");
        m.sha256 = "zz".repeat(32);
        assert!(evaluate(m, "0.4.46").is_err());
        let mut m = manifest("0.4.47");
        m.size = 0;
        assert!(evaluate(m, "0.4.46").is_err());
        assert!(evaluate(manifest("bientôt"), "0.4.46").is_err());
    }

    #[test]
    fn manifest_from_release_script_parses() {
        let json = r#"{"version":"0.4.47","url":"https://updates.bebanana.ch/seaglass/0.4.47/Seaglass_0.4.47_arm64.apk","sha256":"00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff","size":41234567,"notes":"- A\n- B","pub_date":"2026-11-03T18:00:00Z"}"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let found = evaluate(m, "0.4.46").unwrap().unwrap();
        assert_eq!(found.size, 41_234_567);
        assert_eq!(found.info("0.4.46").current_version, "0.4.46");
    }

    #[test]
    fn hex_is_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0x0f]), "00ab0f");
    }
}
