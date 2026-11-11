//! Taille de l'interface (#337) : zoom de la WebView, bureau seulement (Android a déjà le
//! réglage de taille d'affichage du système). Le zoom agrandit tout d'un coup, `px` des
//! thèmes compris, sans toucher au CSS.

use serde::{Deserialize, Deserializer, Serialize};

/// Niveaux proposés, en pourcentage ; les raccourcis passent de l'un à l'autre.
pub const LEVELS: [u16; 6] = [80, 90, 100, 110, 125, 150];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Zoom(u16);

impl Default for Zoom {
    fn default() -> Self {
        Self(100)
    }
}

impl Zoom {
    pub fn new(percent: u16) -> Option<Self> {
        LEVELS.contains(&percent).then_some(Self(percent))
    }

    pub fn factor(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

/// Une valeur hors de `LEVELS` est refusée, comme un thème inconnu.
impl<'de> Deserialize<'de> for Zoom {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let percent = u16::deserialize(d)?;
        Self::new(percent).ok_or_else(|| serde::de::Error::custom("zoom inconnu"))
    }
}

/// Applique le zoom à la fenêtre principale ; sans fenêtre (démarrage caché), rien à faire.
#[cfg(desktop)]
pub(crate) fn apply(app: &tauri::AppHandle, zoom: Zoom) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main")
        && let Err(e) = window.set_zoom(zoom.factor())
    {
        tracing::warn!("zoom de l'interface non appliqué : {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::UiSettings;
    use super::*;

    #[test]
    fn zoom_is_validated_and_defaults_to_100() {
        let s: UiSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.zoom, Zoom(100));
        let s: UiSettings = serde_json::from_str(r#"{"zoom":125}"#).unwrap();
        assert_eq!(s.zoom.factor(), 1.25);
        assert!(serde_json::from_str::<UiSettings>(r#"{"zoom":130}"#).is_err());
    }
}
