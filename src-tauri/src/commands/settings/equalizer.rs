//! Égaliseur (#428) : les profils sont enregistrés avec les réglages de lecture
//! (`set_playback_settings`) ; ici seulement l'import d'un profil AutoEQ, analysé côté
//! Rust pour que l'UI n'ait aucune règle à elle.

use super::super::CmdResult;
use crate::player::autoeq;
use crate::player::eq::EqProfile;

/// Analyse le texte d'un « ParametricEQ.txt » d'AutoEQ (collé, ou lu par l'UI depuis un
/// fichier choisi). Le profil rendu a l'identifiant 0 et un nom vide, fixés par l'UI.
#[tauri::command]
pub async fn parse_autoeq(text: String) -> CmdResult<EqProfile> {
    autoeq::parse(&text).map_err(|e| e.to_string())
}
