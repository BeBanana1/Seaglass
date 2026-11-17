//! Jam (#25) : démarrer (en ligne, ou sur le réseau local depuis le bureau), arrêter, état,
//! contrôles de l'hôte (#288).

use tauri::{AppHandle, State};

use super::{AppState, CmdResult, err};
use crate::jam::relay_settings::{self, RelayChange, RelayView};
use crate::jam::{JamMode, JamRules, JamState, JamStatus};
use crate::store::Store;

/// Règles de l'hôte, retenues d'une Jam à l'autre (#288).
const KEY_RULES: &str = "jam.rules";

fn load_rules(store: Option<&Store>) -> JamRules {
    store
        .and_then(|s| match s.get_setting::<JamRules>(KEY_RULES) {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("Jam : règles illisibles, valeurs par défaut ({e})");
                None
            }
        })
        .unwrap_or_default()
        .sanitized()
}

/// Jam en cours (adresse, code, QR code, connexion, invités, règles), ou `null`.
#[tauri::command]
pub async fn jam_status(jam: State<'_, JamState>) -> CmdResult<Option<JamStatus>> {
    Ok(jam.status())
}

/// Vrai si l'app peut ouvrir une Jam en ligne (relais utilisable, #452).
#[tauri::command]
pub async fn jam_online_available() -> CmdResult<bool> {
    Ok(crate::jam::online_available())
}

/// Relais de la Jam en ligne et des transferts (#452) : choisi, compilé, utilisable.
#[tauri::command]
pub async fn relay_settings() -> CmdResult<RelayView> {
    Ok(relay_settings::view())
}

/// Change de relais (adresse vérifiée par `/healthz`), revient à celui compilé ou le coupe.
/// Vaut pour la prochaine Jam, et pour le tunnel des transferts à sa prochaine ouverture.
#[tauri::command]
pub async fn set_relay_settings(
    state: State<'_, AppState>,
    change: RelayChange,
) -> CmdResult<RelayView> {
    relay_settings::update(state.store.as_deref(), change).await
}

/// Démarre la Jam (ou renvoie celle qui tourne) : adresse et code à donner aux invités.
/// En ligne, l'adresse arrive ensuite par l'événement `jam-status`.
#[tauri::command]
pub async fn start_jam(
    app: AppHandle,
    state: State<'_, AppState>,
    jam: State<'_, JamState>,
    mode: JamMode,
) -> CmdResult<JamStatus> {
    let rules = load_rules(state.store.as_deref());
    jam.start(mode, rules, app, state.tidal.clone(), state.player.clone())
        .await
}

#[tauri::command]
pub async fn stop_jam(jam: State<'_, JamState>) -> CmdResult<()> {
    jam.stop();
    Ok(())
}

/// Règles de l'hôte (lecture / pause par les invités, limite par invité, albums) :
/// enregistrées, puis appliquées à la Jam en cours. État de la Jam, ou `null` sans Jam.
#[tauri::command]
pub async fn set_jam_rules(
    state: State<'_, AppState>,
    jam: State<'_, JamState>,
    rules: JamRules,
) -> CmdResult<Option<JamStatus>> {
    let rules = rules.sanitized();
    if let Some(store) = &state.store {
        store.set_setting(KEY_RULES, &rules).map_err(err)?;
    }
    Ok(jam.set_rules(rules))
}

/// Suspend (`false`) ou rouvre les ajouts des invités, le temps de la Jam (#288).
#[tauri::command]
pub async fn set_jam_adds_open(
    jam: State<'_, JamState>,
    open: bool,
) -> CmdResult<Option<JamStatus>> {
    Ok(jam.set_adds_open(open))
}

/// Exclut un invité (ses titres en attente sont retirés) ou le réadmet (#288).
#[tauri::command]
pub async fn set_jam_guest_banned(
    jam: State<'_, JamState>,
    id: u32,
    banned: bool,
) -> CmdResult<Option<JamStatus>> {
    Ok(jam.set_guest_banned(id, banned).await)
}
