use std::path::Path;

/// Variables read from `../.env` (gitignored) and injected at compile time.
/// An environment variable of the same name at launch still takes priority (see `tidal::auth`).
/// `SEAGLASS_RELAY_KEY`: key of the online Jam relay (#25), becomes
/// `SEAGLASS_SEAGLASS_RELAY_KEY`; without it, only the local Jam is offered.
/// `DISCORD_CLIENT_ID`: Discord application for the presence (#215); without it, the setting is hidden.
const EMBEDDED_ENV: [&str; 4] = [
    "TIDAL_CLIENT_ID",
    "TIDAL_CLIENT_SECRET",
    "SEAGLASS_RELAY_KEY",
    "DISCORD_CLIENT_ID",
];

/// Service addresses (#452, see `src/servers.rs`) and the updater's public key
/// (#489), become `SEAGLASS_BUILD_<…>`. The build environment wins over `.env`: the
/// release scripts put there the address that is also used to write the manifests,
/// and the key read next to the private key. When absent, the service is absent.
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

/// `../VERSION` is the source of truth: the build fails if Cargo.toml differs
/// (tauri.conf.json has no version and takes Cargo.toml's).
fn check_version() {
    println!("cargo:rerun-if-changed=../VERSION");
    let version = std::fs::read_to_string("../VERSION").unwrap_or_default();
    let version = version.trim();
    let cargo = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    if version != cargo {
        panic!("VERSION ({version}) and Cargo.toml ({cargo}) differ: update both");
    }
}
