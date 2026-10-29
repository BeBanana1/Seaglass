use std::path::Path;

/// Variables lues dans `../.env` (gitignoré) et injectées à la compilation.
/// Une variable d'environnement du même nom au lancement reste prioritaire (voir `tidal::auth`).
const EMBEDDED_ENV: [&str; 2] = ["TIDAL_CLIENT_ID", "TIDAL_CLIENT_SECRET"];

fn main() {
    check_version();
    let env_file = Path::new("../.env");
    println!("cargo:rerun-if-changed=../.env");
    if let Ok(content) = std::fs::read_to_string(env_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                if EMBEDDED_ENV.contains(&key) {
                    println!(
                        "cargo:rustc-env=SEAGLASS_{key}={}",
                        value.trim().trim_matches('"')
                    );
                }
            }
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
