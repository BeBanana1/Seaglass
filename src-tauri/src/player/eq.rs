//! Parametric equalizer (#428): named profiles (preamp + peak or shelf filters), one active
//! profile, off by default. Pure, tested logic; the mpv engine translates the profile into
//! `af` filters (`mpv::filters`), Media3 into biquads (`EqAudioProcessor.kt`, #467).
//!
//! The bounds are checked on deserialization: a profile coming from the UI, from a backup or
//! from the AutoEQ import (`player::autoeq`) is always valid.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::actor::Player;
use crate::engine::{Filters, Op};

/// Most filters per profile (AutoEQ gives 10; some corrections up to 15).
pub const MAX_FILTERS: usize = 20;
/// Most saved profiles.
pub const MAX_PROFILES: usize = 32;
/// Maximum length of a profile name, in characters.
pub const MAX_NAME: usize = 80;
/// Gain of a filter and preamp, in dB.
pub const GAIN_RANGE: (f64, f64) = (-20.0, 20.0);
pub const Q_RANGE: (f64, f64) = (0.1, 10.0);
pub const FREQ_RANGE: (f64, f64) = (20.0, 20_000.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterKind {
    /// Peak (bell) centered on `freq`.
    Peaking,
    /// Low shelf, below `freq`.
    LowShelf,
    /// High shelf, above `freq`.
    HighShelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub kind: FilterKind,
    /// Center frequency (peak) or cutoff (shelf), in Hz.
    pub freq: f64,
    /// Gain in dB.
    pub gain: f64,
    pub q: f64,
}

/// Reason a profile was refused, already in the app's language.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

fn within(v: f64, (min, max): (f64, f64)) -> bool {
    v.is_finite() && (min..=max).contains(&v)
}

impl Filter {
    /// Checks the bounds; `n` is the filter number (from 1) for the message.
    pub fn check(&self, n: usize) -> Result<(), Invalid> {
        use crate::i18n::trf;
        if !within(self.freq, FREQ_RANGE) {
            return Err(Invalid(trf!(
                "filtre {} : fréquence hors de 20 Hz à 20 kHz",
                "filter {}: frequency outside 20 Hz to 20 kHz",
                n
            )));
        }
        if !within(self.gain, GAIN_RANGE) {
            return Err(Invalid(trf!(
                "filtre {} : gain hors de ±20 dB",
                "filter {}: gain outside ±20 dB",
                n
            )));
        }
        if !within(self.q, Q_RANGE) {
            return Err(Invalid(trf!(
                "filtre {} : Q hors de 0,1 à 10",
                "filter {}: Q outside 0.1 to 10",
                n
            )));
        }
        Ok(())
    }
}

/// Equalizer profile: preamp then filters, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "RawProfile")]
pub struct EqProfile {
    pub id: u32,
    pub name: String,
    /// Preamp in dB; `None` = automatic (`auto_preamp`).
    pub preamp: Option<f64>,
    pub filters: Vec<Filter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProfile {
    id: u32,
    name: String,
    #[serde(default)]
    preamp: Option<f64>,
    filters: Vec<Filter>,
}

impl TryFrom<RawProfile> for EqProfile {
    type Error = Invalid;

    fn try_from(raw: RawProfile) -> Result<Self, Invalid> {
        let profile = Self {
            id: raw.id,
            name: raw.name.trim().to_owned(),
            preamp: raw.preamp,
            filters: raw.filters,
        };
        profile.check()?;
        Ok(profile)
    }
}

impl EqProfile {
    pub fn check(&self) -> Result<(), Invalid> {
        let len = self.name.chars().count();
        if len == 0 || len > MAX_NAME {
            return Err(Invalid(
                crate::i18n::tr(
                    "nom de profil vide ou trop long",
                    "profile name empty or too long",
                )
                .into(),
            ));
        }
        self.check_values()
    }

    /// Bounds of the preamp and the filters, without the name (AutoEQ import).
    pub fn check_values(&self) -> Result<(), Invalid> {
        use crate::i18n::{tr, trf};
        if self.preamp.is_some_and(|p| !within(p, GAIN_RANGE)) {
            return Err(Invalid(
                tr("préampli hors de ±20 dB", "preamp outside ±20 dB").into(),
            ));
        }
        if self.filters.len() > MAX_FILTERS {
            return Err(Invalid(trf!(
                "{} filtres au plus",
                "{} filters at most",
                MAX_FILTERS
            )));
        }
        for (i, f) in self.filters.iter().enumerate() {
            f.check(i + 1)?;
        }
        Ok(())
    }

    /// Automatic preamp: compensates the largest positive gain, to avoid clipping.
    pub fn auto_preamp(&self) -> f64 {
        let boost = self.filters.iter().map(|f| f.gain).fold(0.0_f64, f64::max);
        -boost
    }

    /// Preamp applied: the profile's, otherwise the automatic one.
    pub fn effective_preamp(&self) -> f64 {
        self.preamp.unwrap_or_else(|| self.auto_preamp())
    }
}

/// Setting Settings › Playback › Equalizer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "RawSettings")]
pub struct EqSettings {
    /// Off by default: the signal stays untouched.
    pub enabled: bool,
    /// Chosen profile, kept even when disabled.
    pub active: Option<u32>,
    pub profiles: Vec<EqProfile>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawSettings {
    enabled: bool,
    active: Option<u32>,
    profiles: Vec<EqProfile>,
}

impl TryFrom<RawSettings> for EqSettings {
    type Error = Invalid;

    fn try_from(raw: RawSettings) -> Result<Self, Invalid> {
        use crate::i18n::{tr, trf};
        if raw.profiles.len() > MAX_PROFILES {
            return Err(Invalid(trf!(
                "{} profils au plus",
                "{} profiles at most",
                MAX_PROFILES
            )));
        }
        let mut ids: Vec<u32> = raw.profiles.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != raw.profiles.len() {
            return Err(Invalid(
                tr("identifiants de profil en double", "duplicate profile ids").into(),
            ));
        }
        // Chosen profile gone: none, rather than refusing the whole settings.
        let active = raw
            .active
            .filter(|id| raw.profiles.iter().any(|p| p.id == *id));
        Ok(Self {
            enabled: raw.enabled,
            active,
            profiles: raw.profiles,
        })
    }
}

impl EqSettings {
    /// Profile actually applied: none if disabled or without a chosen profile.
    pub fn applied(&self) -> Option<&EqProfile> {
        if !self.enabled {
            return None;
        }
        let id = self.active?;
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Value kept per output device (#416): the applied profile, or none.
    pub fn device_choice(&self) -> Option<u32> {
        self.applied().map(|p| p.id)
    }

    /// Applies the choice kept for a device: a profile (if it still exists) or the equalizer
    /// off.
    pub fn apply_device_choice(&mut self, choice: Option<u32>) {
        match choice.filter(|id| self.profiles.iter().any(|p| p.id == *id)) {
            Some(id) => {
                self.enabled = true;
                self.active = Some(id);
            }
            None => self.enabled = false,
        }
    }
}

/// Equalizer held by the actor: setting and applied profile, shared with the engine.
#[derive(Debug, Clone, Default)]
pub(super) struct Equalizer {
    pub(super) settings: EqSettings,
    applied: Option<Arc<EqProfile>>,
}

/// Profile actually applied by the engine (mpv, or `EqAudioProcessor.kt` on Android).
fn engine_profile(settings: &EqSettings) -> Option<EqProfile> {
    settings.applied().cloned()
}

impl Equalizer {
    pub(super) fn new(settings: EqSettings) -> Self {
        let applied = engine_profile(&settings).map(Arc::new);
        Self { settings, applied }
    }

    /// Name of the applied profile, for the signal path.
    pub(super) fn applied_name(&self) -> Option<String> {
        self.applied.as_ref().map(|p| p.name.clone())
    }

    pub(super) fn applied(&self) -> Option<Arc<EqProfile>> {
        self.applied.clone()
    }

    /// New setting; returns `true` if the applied profile changed.
    pub(super) fn set(&mut self, settings: EqSettings) -> bool {
        let applied = engine_profile(&settings);
        let changed = applied.as_ref() != self.applied.as_deref();
        self.settings = settings;
        if changed {
            self.applied = applied.map(Arc::new);
        }
        changed
    }
}

impl Player {
    /// Filters of a file: given normalization gain, current equalizer.
    pub(super) fn filters(&self, gain: Option<f64>) -> Filters {
        Filters {
            gain,
            eq: self.equalizer.applied(),
        }
    }

    /// Filters of the current track, set live (normalization or equalizer changed), then the
    /// preloaded track is removed so it is reloaded with its own.
    pub(super) async fn refresh_filters(&mut self) {
        let gain = self.normalizer.current_gain();
        if self.loaded {
            self.engine_op(Op::CurrentFilters(self.filters(gain))).await;
            self.state.normalization = gain;
        }
        self.invalidate_next().await;
    }

    /// New equalizer setting.
    pub(super) async fn set_equalizer(&mut self, settings: EqSettings) {
        let changed = self.equalizer.set(settings);
        self.state.equalizer = self.equalizer.applied_name();
        self.note_device_profile();
        if changed {
            self.refresh_filters().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(freq: f64, gain: f64, q: f64) -> Filter {
        Filter {
            kind: FilterKind::Peaking,
            freq,
            gain,
            q,
        }
    }

    fn profile(id: u32, filters: Vec<Filter>) -> EqProfile {
        EqProfile {
            id,
            name: format!("P{id}"),
            preamp: None,
            filters,
        }
    }

    #[test]
    fn bounds_are_checked_on_deserialization() {
        let read = |json: &str| serde_json::from_str::<EqProfile>(json);
        let ok = read(
            r#"{"id":1,"name":" HD 600 ","filters":[{"kind":"peaking","freq":105,"gain":3.4,"q":0.7}]}"#,
        )
        .unwrap();
        assert_eq!(ok.name, "HD 600", "name cleaned up");
        assert_eq!(ok.preamp, None);
        for bad in [
            r#"{"id":1,"name":"x","filters":[{"kind":"peaking","freq":10,"gain":1,"q":1}]}"#,
            r#"{"id":1,"name":"x","filters":[{"kind":"peaking","freq":100,"gain":21,"q":1}]}"#,
            r#"{"id":1,"name":"x","filters":[{"kind":"lowShelf","freq":100,"gain":1,"q":0}]}"#,
            r#"{"id":1,"name":"x","preamp":-25,"filters":[]}"#,
            r#"{"id":1,"name":"  ","filters":[]}"#,
            r#"{"id":1,"name":"x","filters":[{"kind":"notch","freq":100,"gain":1,"q":1}]}"#,
        ] {
            assert!(read(bad).is_err(), "{bad}");
        }
        let many = profile(1, vec![pk(100.0, 1.0, 1.0); MAX_FILTERS + 1]);
        assert!(many.check().is_err());
    }

    #[test]
    fn auto_preamp_compensates_the_largest_boost() {
        let p = profile(1, vec![pk(100.0, 3.4, 1.0), pk(2000.0, -6.0, 1.0)]);
        assert_eq!(p.effective_preamp(), -3.4);
        assert_eq!(
            profile(1, vec![pk(100.0, -2.0, 1.0)]).effective_preamp(),
            0.0
        );
        let fixed = EqProfile {
            preamp: Some(-6.2),
            ..p
        };
        assert_eq!(fixed.effective_preamp(), -6.2);
    }

    #[test]
    fn settings_default_off_and_validate_profiles() {
        let s: EqSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, EqSettings::default());
        assert!(!s.enabled, "off by default");
        assert_eq!(s.applied(), None);

        let dup =
            r#"{"profiles":[{"id":1,"name":"a","filters":[]},{"id":1,"name":"b","filters":[]}]}"#;
        assert!(serde_json::from_str::<EqSettings>(dup).is_err());

        // Chosen profile gone: forgotten, without refusing the rest.
        let s: EqSettings = serde_json::from_str(
            r#"{"enabled":true,"active":9,"profiles":[{"id":1,"name":"a","filters":[]}]}"#,
        )
        .unwrap();
        assert_eq!(s.active, None);
        assert_eq!(s.applied(), None);
    }

    #[test]
    fn applied_profile_and_device_choice() {
        let mut s = EqSettings {
            enabled: true,
            active: Some(2),
            profiles: vec![profile(1, vec![]), profile(2, vec![pk(100.0, 2.0, 1.0)])],
        };
        assert_eq!(s.applied().map(|p| p.id), Some(2));
        assert_eq!(s.device_choice(), Some(2));
        s.enabled = false;
        assert_eq!(s.device_choice(), None);

        s.apply_device_choice(Some(1));
        assert!(s.enabled);
        assert_eq!(s.active, Some(1));
        s.apply_device_choice(None);
        assert!(!s.enabled);
        assert_eq!(s.active, Some(1), "the choice is kept for next time");
        s.apply_device_choice(Some(42));
        assert!(!s.enabled, "profile deleted since: disabled");
    }

    #[test]
    fn equalizer_reports_changes_of_the_applied_profile() {
        let a = profile(1, vec![pk(100.0, 2.0, 1.0)]);
        let off = EqSettings {
            enabled: false,
            active: Some(1),
            profiles: vec![a.clone()],
        };
        let mut eq = Equalizer::new(off.clone());
        assert!(eq.applied().is_none());
        let on = EqSettings {
            enabled: true,
            ..off.clone()
        };
        assert!(eq.set(on.clone()));
        assert_eq!(eq.applied().as_deref(), Some(&a));
        assert!(!eq.set(on.clone()), "rien de neuf");
        // Another, inactive profile added: the applied profile does not change.
        let mut more = on.clone();
        more.profiles.push(profile(2, vec![]));
        assert!(!eq.set(more.clone()));
        // The applied profile modified: change.
        more.profiles[0].filters[0].gain = 4.0;
        assert!(eq.set(more));
        assert!(eq.set(off));
        assert!(eq.applied().is_none());
    }

    #[test]
    fn serialized_form() {
        let s = EqSettings {
            enabled: true,
            active: Some(1),
            profiles: vec![EqProfile {
                id: 1,
                name: "HD 600".into(),
                preamp: Some(-6.2),
                filters: vec![Filter {
                    kind: FilterKind::LowShelf,
                    freq: 105.0,
                    gain: 5.5,
                    q: 0.7,
                }],
            }],
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"enabled":true,"active":1,"profiles":[{"id":1,"name":"HD 600","preamp":-6.2,"filters":[{"kind":"lowShelf","freq":105.0,"gain":5.5,"q":0.7}]}]}"#
        );
        assert_eq!(serde_json::from_str::<EqSettings>(&json).unwrap(), s);
    }
}
