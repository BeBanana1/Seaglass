use std::path::Path;

/// Variables lues dans `../.env` (gitignoré) et injectées à la compilation.
/// Une variable d'environnement du même nom au lancement reste prioritaire (voir `tidal::auth`).
/// `SEAGLASS_RELAY_KEY` : clé du relais de la Jam en ligne (#25), devient
/// `SEAGLASS_SEAGLASS_RELAY_KEY` ; sans elle, seule la Jam locale est proposée.
/// `DISCORD_CLIENT_ID` : application Discord de la présence (#215) ; sans lui, réglage masqué.
const EMBEDDED_ENV: [&str; 4] = [
    "TIDAL_CLIENT_ID",
    "TIDAL_CLIENT_SECRET",
    "SEAGLASS_RELAY_KEY",
    "DISCORD_CLIENT_ID",
];

/// Adresses des services (#452, voir `src/servers.rs`) et clé publique de l'updater
/// (#489), deviennent `SEAGLASS_BUILD_<…>`. L'environnement de la compilation l'emporte
/// sur `.env` : les scripts de release y mettent l'adresse qui sert aussi à écrire les
/// manifestes, et la clé lue à côté de la clé privée. Absentes : service absent.
const SERVER_ENV: [(&str, &str); 3] = [
    ("SEAGLASS_UPDATE_BASE_URL", "SEAGLASS_BUILD_UPDATE_BASE_URL"),
    ("SEAGLASS_RELAY_BASE_URL", "SEAGLASS_BUILD_RELAY_BASE_URL"),
    ("SEAGLASS_UPDATER_PUBKEY", "SEAGLASS_BUILD_UPDATER_PUBKEY"),
];

fn main() {
    check_version();
    let env_file = Path::new("../.env");
    println!("cargo:rerun-if-changed=../.env");
    let mut from_file: Vec<(String, String)> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(env_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                if EMBEDDED_ENV.contains(&key) {
                    println!("cargo:rustc-env=SEAGLASS_{key}={value}");
                }
                from_file.push((key.to_owned(), value.to_owned()));
            }
        }
    }
    for (name, compiled) in SERVER_ENV {
        println!("cargo:rerun-if-env-changed={name}");
        let value = std::env::var(name).ok().or_else(|| {
            from_file
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        });
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            println!("cargo:rustc-env={compiled}={}", value.trim());
        }
    }
    tauri_build::build()
}

/// `../VERSION` est la source de vérité : la compilation échoue si Cargo.toml diverge
/// (tauri.conf.json n'a pas de version et reprend celle de Cargo.toml).
fn check_version() {
    println!("cargo:rerun-if-changed=../VERSION");
    let version = std::fs::read_to_string("../VERSION").unwrap_or_default();
    let version = version.trim();
    let cargo = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    if version != cargo {
        panic!("VERSION ({version}) et Cargo.toml ({cargo}) diffèrent : mettre à jour les deux");
    }
}
