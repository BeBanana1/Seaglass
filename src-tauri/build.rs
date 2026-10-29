use std::path::Path;

/// Variables lues dans `../.env` (gitignoré) et injectées à la compilation.
/// Une variable d'environnement du même nom au lancement reste prioritaire (voir `tidal::auth`).
const EMBEDDED_ENV: [&str; 2] = ["TIDAL_CLIENT_ID", "TIDAL_CLIENT_SECRET"];

fn main() {
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
