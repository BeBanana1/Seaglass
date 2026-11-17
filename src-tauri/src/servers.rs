//! Adresses des services en ligne (#452) : rien n'est écrit en dur dans le code.
//!
//! Valeurs de compilation (lues par `build.rs` dans l'environnement, sinon dans `.env`) :
//! - `SEAGLASS_UPDATE_BASE_URL` : dossier des mises à jour (`latest.json`, `android.json`,
//!   APK). Absent : pas de mises à jour, l'app ne propose rien. Les scripts de release le
//!   fixent (le même sert à écrire les manifestes), avec l'adresse de l'auteur par défaut.
//! - `SEAGLASS_RELAY_BASE_URL` : relais de la Jam en ligne et des transferts, par défaut ;
//!   l'utilisateur peut en choisir un autre dans les Réglages (`jam::relay`).
//!
//! L'adresse des mises à jour n'est pas réglable dans l'app : l'updater n'accepte que des
//! builds signés par la clé compilée (`tauri.conf.json` › `pubkey`, certificat de l'APK
//! sur Android). Un autre serveur ne pourrait servir qu'un miroir des mêmes builds ; qui
//! recompile avec sa clé fixe aussi son adresse.

use std::sync::atomic::{AtomicBool, Ordering};

/// Interrupteur « Mises à jour automatiques » des Réglages, reporté par `apply_to_backend`.
static AUTO_UPDATES: AtomicBool = AtomicBool::new(true);

/// Valeur de compilation non vide, sans `/` final.
fn compiled(value: Option<&'static str>) -> Option<&'static str> {
    value
        .map(|v| v.trim().trim_end_matches('/'))
        .filter(|v| !v.is_empty())
}

/// Dossier des mises à jour compilé, sans `/` final ; `None` : mises à jour absentes.
pub fn update_base() -> Option<&'static str> {
    compiled(option_env!("SEAGLASS_BUILD_UPDATE_BASE_URL"))
}

/// Relais compilé (`https://…`, sans `/` final), proposé par défaut.
pub fn relay_base() -> Option<&'static str> {
    compiled(option_env!("SEAGLASS_BUILD_RELAY_BASE_URL"))
}

pub fn set_auto_updates(enabled: bool) {
    AUTO_UPDATES.store(enabled, Ordering::Relaxed);
}

/// Vrai si la vérification périodique doit tourner (le bouton « Vérifier » marche toujours).
pub fn auto_updates() -> bool {
    AUTO_UPDATES.load(Ordering::Relaxed)
}

/// Adresse du dossier des mises à jour, pour les Réglages (lecture seule) ; `None` : l'app
/// a été compilée sans, la section des mises à jour est masquée.
#[tauri::command]
pub fn update_server() -> Option<String> {
    update_base().map(str::to_owned)
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
