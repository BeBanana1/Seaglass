//! Égaliseur paramétrique (#428) : profils nommés (préampli + filtres en pic ou en
//! plateau), un profil actif, désactivé par défaut. Logique pure testée ; le moteur mpv
//! traduit le profil en filtres `af` (`mpv::filters`), Media3 en biquads
//! (`EqAudioProcessor.kt`, #467).
//!
//! Les bornes sont vérifiées à la désérialisation : un profil qui arrive de l'UI, d'une
//! sauvegarde ou de l'import AutoEQ (`player::autoeq`) est toujours valide.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::actor::Player;
use crate::engine::{Filters, Op};

/// Filtres par profil au plus (AutoEQ en donne 10 ; certaines corrections jusqu'à 15).
pub const MAX_FILTERS: usize = 20;
/// Profils enregistrés au plus.
pub const MAX_PROFILES: usize = 32;
/// Longueur maximale d'un nom de profil, en caractères.
pub const MAX_NAME: usize = 80;
/// Gain d'un filtre et préampli, en dB.
pub const GAIN_RANGE: (f64, f64) = (-20.0, 20.0);
pub const Q_RANGE: (f64, f64) = (0.1, 10.0);
pub const FREQ_RANGE: (f64, f64) = (20.0, 20_000.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterKind {
    /// Pic (cloche) centré sur `freq`.
    Peaking,
    /// Plateau des graves, sous `freq`.
    LowShelf,
    /// Plateau des aigus, au-dessus de `freq`.
    HighShelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub kind: FilterKind,
    /// Fréquence centrale (pic) ou de coupure (plateau), en Hz.
    pub freq: f64,
    /// Gain en dB.
    pub gain: f64,
    pub q: f64,
}

/// Raison du refus d'un profil, déjà dans la langue de l'app.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

fn within(v: f64, (min, max): (f64, f64)) -> bool {
    v.is_finite() && (min..=max).contains(&v)
}

impl Filter {
    /// Vérifie les bornes ; `n` est le numéro du filtre (à partir de 1) pour le message.
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

/// Profil d'égaliseur : préampli puis filtres, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "RawProfile")]
pub struct EqProfile {
    pub id: u32,
    pub name: String,
    /// Préampli en dB ; `None` = automatique (`auto_preamp`).
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

    /// Bornes du préampli et des filtres, sans le nom (import AutoEQ, `player::autoeq`).
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

    /// Préampli automatique : compense le plus fort gain positif, pour ne pas écrêter.
    pub fn auto_preamp(&self) -> f64 {
        let boost = self.filters.iter().map(|f| f.gain).fold(0.0_f64, f64::max);
        -boost
    }

    /// Préampli appliqué : celui du profil, sinon l'automatique.
    pub fn effective_preamp(&self) -> f64 {
        self.preamp.unwrap_or_else(|| self.auto_preamp())
    }
}

/// Réglage Réglages › Lecture › Égaliseur.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "RawSettings")]
pub struct EqSettings {
    /// Désactivé par défaut : le signal reste intact.
    pub enabled: bool,
    /// Profil choisi, gardé même désactivé.
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
        // Profil choisi disparu : aucun, plutôt que de refuser tous les réglages.
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
    /// Profil réellement appliqué : aucun si désactivé ou sans profil choisi.
    pub fn applied(&self) -> Option<&EqProfile> {
        if !self.enabled {
            return None;
        }
        let id = self.active?;
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Valeur retenue par périphérique de sortie (#416) : le profil appliqué, ou aucun.
    pub fn device_choice(&self) -> Option<u32> {
        self.applied().map(|p| p.id)
    }

    /// Applique le choix retenu pour un périphérique : un profil (s'il existe encore) ou
    /// l'égaliseur désactivé.
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

/// Égaliseur tenu par l'acteur : réglage et profil appliqué, partagé avec le moteur.
#[derive(Debug, Clone, Default)]
pub(super) struct Equalizer {
    pub(super) settings: EqSettings,
    applied: Option<Arc<EqProfile>>,
}

/// Profil réellement appliqué par le moteur (mpv, ou `EqAudioProcessor.kt` sous Android).
fn engine_profile(settings: &EqSettings) -> Option<EqProfile> {
    settings.applied().cloned()
}

impl Equalizer {
    pub(super) fn new(settings: EqSettings) -> Self {
        let applied = engine_profile(&settings).map(Arc::new);
        Self { settings, applied }
    }

    /// Nom du profil appliqué, pour la chaîne audio.
    pub(super) fn applied_name(&self) -> Option<String> {
        self.applied.as_ref().map(|p| p.name.clone())
    }

    pub(super) fn applied(&self) -> Option<Arc<EqProfile>> {
        self.applied.clone()
    }

    /// Nouveau réglage ; renvoie `true` si le profil appliqué a changé.
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
    /// Filtres d'un fichier : gain de normalisation donné, égaliseur en vigueur.
    pub(super) fn filters(&self, gain: Option<f64>) -> Filters {
        Filters {
            gain,
            eq: self.equalizer.applied(),
        }
    }

    /// Filtres du titre en cours, posés à chaud (normalisation ou égaliseur changé), puis
    /// le préchargé est retiré pour être rechargé avec les siens.
    pub(super) async fn refresh_filters(&mut self) {
        let gain = self.normalizer.current_gain();
        if self.loaded {
            self.engine_op(Op::CurrentFilters(self.filters(gain))).await;
            self.state.normalization = gain;
        }
        self.invalidate_next().await;
    }

    /// Nouveau réglage de l'égaliseur.
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
        assert_eq!(ok.name, "HD 600", "nom nettoyé");
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
        assert!(!s.enabled, "désactivé par défaut");
        assert_eq!(s.applied(), None);

        let dup =
            r#"{"profiles":[{"id":1,"name":"a","filters":[]},{"id":1,"name":"b","filters":[]}]}"#;
        assert!(serde_json::from_str::<EqSettings>(dup).is_err());

        // Profil choisi disparu : oublié, sans refuser le reste.
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
        assert_eq!(s.active, Some(1), "le choix reste pour la prochaine fois");
        s.apply_device_choice(Some(42));
        assert!(!s.enabled, "profil supprimé depuis : désactivé");
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
        // Un autre profil, inactif, ajouté : le profil appliqué ne change pas.
        let mut more = on.clone();
        more.profiles.push(profile(2, vec![]));
        assert!(!eq.set(more.clone()));
        // Le profil appliqué modifié : changement.
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
