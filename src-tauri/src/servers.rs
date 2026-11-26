//! Addresses of the online services (#452): nothing is hardcoded in the code.
//!
//! Compile-time values (read by `build.rs` from the environment, otherwise from `.env`):
//! - `SEAGLASS_UPDATE_BASE_URL`: updates folder (`latest.json`, `android.json`,
//!   APK). Absent: no updates, the app offers nothing. The release scripts set it
//!   (the same one is used to write the manifests), with the author's address by default.
//! - `SEAGLASS_RELAY_BASE_URL`: relay for online Jam and transfers, by default;
//!   the user can pick another one in Settings (`jam::relay`).
//!
//! - `SEAGLASS_UPDATER_PUBKEY` (#489): public key of the desktop updater (content of the
//!   `.pub` file from `tauri signer generate`). Absent: no updates on the
//!   desktop, even with an address. The release scripts read it next to the private
//!   key; `tauri.conf.json` contains none.
//!
//! The updates address is not settable in the app: the updater only accepts
//! builds signed by the compiled key (public key above, APK certificate on
//! Android). Another server could only serve a mirror of the same builds; whoever
//! recompiles with their own key sets their own address too.

use std::sync::atomic::{AtomicBool, Ordering};

/// "Automatic updates" switch in Settings, carried over by `apply_to_backend`.
static AUTO_UPDATES: AtomicBool = AtomicBool::new(true);

/// Non-empty compile-time value, without a trailing `/`.
fn compiled(value: Option<&'static str>) -> Option<&'static str> {
    value
        .map(|v| v.trim().trim_end_matches('/'))
        .filter(|v| !v.is_empty())
}

/// Compiled updates folder, without a trailing `/`; `None`: updates absent.
pub fn update_base() -> Option<&'static str> {
    compiled(option_env!("SEAGLASS_BUILD_UPDATE_BASE_URL"))
}

/// Public key of the desktop updater (#489); `None`: no desktop updates.
pub fn updater_pubkey() -> Option<&'static str> {
    compiled(option_env!("SEAGLASS_BUILD_UPDATER_PUBKEY"))
}

/// Updates possible for this build: an address, and on the desktop a public key
/// (Android checks the APK certificate).
fn updates_configured() -> bool {
    update_base().is_some() && (cfg!(target_os = "android") || updater_pubkey().is_some())
}

/// Compiled relay (`https://…`, without a trailing `/`), offered by default.
pub fn relay_base() -> Option<&'static str> {
    compiled(option_env!("SEAGLASS_BUILD_RELAY_BASE_URL"))
}

pub fn set_auto_updates(enabled: bool) {
    AUTO_UPDATES.store(enabled, Ordering::Relaxed);
}

/// True if the periodic check must run (the "Check" button always works).
pub fn auto_updates() -> bool {
    AUTO_UPDATES.load(Ordering::Relaxed)
}

/// Address of the updates folder, for Settings (read-only); `None`: the app
/// was compiled without one (or, on the desktop, without a public key), the updates
/// section is hidden.
#[tauri::command]
pub fn update_server() -> Option<String> {
    update_base()
        .filter(|_| updates_configured())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::compiled;

    #[test]
    fn compiled_values_are_trimmed() {
        assert_eq!(compiled(None), None);
        assert_eq!(compiled(Some("")), None);
        assert_eq!(compiled(Some("  ")), None);
        assert_eq!(
            compiled(Some("https://u.example/seaglass/")),
            Some("https://u.example/seaglass")
        );
    }
}
