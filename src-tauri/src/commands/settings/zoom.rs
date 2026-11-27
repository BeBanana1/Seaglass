//! Interface size (#337): WebView zoom, desktop only (Android already has the
//! system's display size setting). The zoom enlarges everything at once, themes'
//! `px` included, without touching the CSS.

use serde::{Deserialize, Deserializer, Serialize};

/// Levels offered, as a percentage; the shortcuts go from one to the next.
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

    /// Desktop only, like `apply`: without it, the Android build warns about an
    /// unused method (#357).
    #[cfg(any(desktop, test))]
    pub fn factor(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

/// A value outside `LEVELS` is refused, like an unknown theme.
impl<'de> Deserialize<'de> for Zoom {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let percent = u16::deserialize(d)?;
        Self::new(percent).ok_or_else(|| serde::de::Error::custom("unknown zoom"))
    }
}

/// Applies the zoom to the main window; without a window (hidden startup), nothing to do.
#[cfg(desktop)]
pub(crate) fn apply(app: &tauri::AppHandle, zoom: Zoom) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main")
        && let Err(e) = window.set_zoom(zoom.factor())
    {
        tracing::warn!("interface zoom not applied: {e}");
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
