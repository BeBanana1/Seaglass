//! Périphériques de sortie audio (#79) : liste `audio-device-list` de mpv, choix enregistré
//! dans les réglages de lecture et périphérique effectivement demandé à mpv.
//!
//! Vérifié avec mpv 0.41 : la propriété est observable (valeur initiale envoyée dès
//! `observe_property`) et renotifiée sur les événements de branchement de WASAPI
//! (`ao_hotplug_event` dans `ao_wasapi_changenotify.c`). Un `audio-device` posé pendant la
//! lecture rouvre la sortie sans interrompre le titre. Un périphérique absent au chargement
//! fait échouer le fichier (`end-file` en erreur, [`INIT_FAILED`]) : mpv ne se replie pas
//! seul sur le périphérique par défaut, c'est le lecteur qui le fait.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Valeur de `audio-device` pour le périphérique par défaut du système.
pub const AUTO: &str = "auto";
/// Seuls les périphériques de la sortie utilisée (`--ao=wasapi`) sont proposés.
const PREFIX: &str = "wasapi/";
/// Bornes d'un choix reçu de l'UI : un identifiant WASAPI fait une quarantaine de caractères.
const MAX_ID: usize = 200;
const MAX_NAME: usize = 200;
/// `file_error` de mpv quand la sortie audio ne s'ouvre pas (périphérique absent…).
pub const INIT_FAILED: &str = "audio output initialization failed";

/// Périphérique de sortie : identifiant mpv (`wasapi/{guid}`) et nom affiché par Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawDevice")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct RawDevice {
    id: String,
    name: String,
}

/// Choix reçu de l'UI ou relu en base : identifiant WASAPI, nom non vide, longueurs bornées.
impl TryFrom<RawDevice> for AudioDevice {
    type Error = String;

    fn try_from(raw: RawDevice) -> Result<Self, Self::Error> {
        let name = raw.name.trim().to_owned();
        let valid_id = raw.id.starts_with(PREFIX)
            && raw.id.len() > PREFIX.len()
            && raw.id.len() <= MAX_ID
            && !raw.id.chars().any(char::is_control);
        if !valid_id {
            return Err(format!("périphérique audio invalide : {:?}", raw.id));
        }
        if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
            return Err("nom de périphérique audio invalide".into());
        }
        Ok(Self { id: raw.id, name })
    }
}

/// Entrées WASAPI de `audio-device-list` (`[{name, description}]`), dans l'ordre de mpv.
/// `auto` et les autres sorties (`openal`…) sont écartés.
pub fn parse_list(data: &Value) -> Vec<AudioDevice> {
    data.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("name")?.as_str()?;
            if !id.starts_with(PREFIX) {
                return None;
            }
            let name = entry
                .get("description")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .unwrap_or(id);
            Some(AudioDevice {
                id: id.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// Identifiant mpv d'un point de terminaison Windows : `IMMDevice::GetId` renvoie
/// `{0.0.0.00000000}.{guid}`, mpv n'en garde que le guid. Utilisé seulement sous Windows
/// (code mort en CI Linux).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn mpv_id_from_endpoint(endpoint: &str) -> Option<String> {
    let guid = endpoint.rsplit('.').next()?;
    (guid.starts_with('{') && guid.ends_with('}') && guid.len() > 2)
        .then(|| format!("{PREFIX}{}", guid.to_ascii_lowercase()))
}

/// Retrouve un identifiant dans la liste, sans tenir compte de la casse du guid.
pub fn find<'a>(list: &'a [AudioDevice], id: &str) -> Option<&'a AudioDevice> {
    list.iter().find(|d| d.id.eq_ignore_ascii_case(id))
}

/// Périphérique à demander à mpv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Valeur de `audio-device` : l'identifiant choisi ou [`AUTO`].
    pub device: String,
    /// Le choix enregistré n'est pas utilisable : sortie sur le périphérique par défaut.
    pub fallback: bool,
}

/// Choix enregistré s'il figure dans la liste de mpv, sinon périphérique par défaut (repli
/// signalé). Liste encore inconnue (avant la première notification) : le choix est gardé.
/// `unusable` : périphérique qui a refusé de s'ouvrir alors qu'il était listé.
pub fn resolve(
    chosen: Option<&AudioDevice>,
    list: Option<&[AudioDevice]>,
    unusable: Option<&str>,
) -> Resolved {
    let Some(chosen) = chosen else {
        return Resolved {
            device: AUTO.into(),
            fallback: false,
        };
    };
    let present = list.is_none_or(|list| find(list, &chosen.id).is_some());
    let refused = unusable.is_some_and(|id| id.eq_ignore_ascii_case(&chosen.id));
    if present && !refused {
        Resolved {
            device: chosen.id.clone(),
            fallback: false,
        }
    } else {
        Resolved {
            device: AUTO.into(),
            fallback: true,
        }
    }
}

/// Identifiant mpv du périphérique de sortie par défaut de Windows, pour afficher son nom
/// quand `audio-device` vaut `auto`. Rôle `eMultimedia`, le même que mpv
/// (`OnDefaultDeviceChanged` n'écoute que lui). Appel COM bloquant mais bref.
#[cfg(windows)]
pub fn system_default_id() -> Option<String> {
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eMultimedia, eRender,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    };

    // SAFETY : appels COM documentés ; la chaîne renvoyée par `GetId` est libérée avec
    // `CoTaskMemFree` après copie. Un thread déjà initialisé (S_FALSE, autre mode) reste
    // utilisable : le résultat de `CoInitializeEx` est ignoré.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .ok()?;
        let raw = device.GetId().ok()?;
        let text = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0 as *const _));
        text.as_deref().and_then(mpv_id_from_endpoint)
    }
}

#[cfg(not(windows))]
pub fn system_default_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device(id: &str, name: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: name.into(),
        }
    }

    const DAC: &str = "wasapi/{27264a86-c734-4c85-876c-dcfeb567529f}";

    #[test]
    fn list_keeps_wasapi_entries() {
        // Valeur relevée sur mpv 0.41.
        let data = json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": DAC, "description": "Haut-parleurs (3- Focusrite USB Audio)"},
            {"name": "wasapi/{aaaa}", "description": "  "},
            {"name": "openal", "description": "Default (openal)"},
            {"description": "sans nom"}
        ]);
        assert_eq!(
            parse_list(&data),
            vec![
                device(DAC, "Haut-parleurs (3- Focusrite USB Audio)"),
                device("wasapi/{aaaa}", "wasapi/{aaaa}"),
            ]
        );
        assert!(parse_list(&Value::Null).is_empty());
    }

    #[test]
    fn choice_is_validated() {
        let ok: AudioDevice =
            serde_json::from_value(json!({"id": DAC, "name": " Focusrite "})).unwrap();
        assert_eq!(ok, device(DAC, "Focusrite"));
        for bad in [
            json!({"id": "auto", "name": "x"}),
            json!({"id": "wasapi/", "name": "x"}),
            json!({"id": "openal", "name": "x"}),
            json!({"id": DAC, "name": "  "}),
            json!({"id": format!("wasapi/{}", "a".repeat(300)), "name": "x"}),
            json!({"id": "wasapi/{a}\n", "name": "x"}),
            json!({"id": DAC}),
        ] {
            assert!(
                serde_json::from_value::<AudioDevice>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn endpoint_id_maps_to_mpv() {
        assert_eq!(
            mpv_id_from_endpoint("{0.0.0.00000000}.{27264A86-C734-4C85-876C-DCFEB567529F}")
                .as_deref(),
            Some(DAC)
        );
        assert_eq!(mpv_id_from_endpoint("garbage"), None);
        assert_eq!(mpv_id_from_endpoint("{0.0.0.00000000}.{}"), None);
    }

    #[test]
    fn resolve_falls_back_without_losing_the_choice() {
        let dac = device(DAC, "Focusrite");
        let list = vec![dac.clone()];
        let auto = Resolved {
            device: AUTO.into(),
            fallback: false,
        };
        let on_dac = Resolved {
            device: DAC.into(),
            fallback: false,
        };
        let fallback = Resolved {
            device: AUTO.into(),
            fallback: true,
        };

        assert_eq!(resolve(None, Some(&list), None), auto);
        assert_eq!(resolve(None, None, None), auto);
        // Liste pas encore reçue : on suit le choix (passé aussi en ligne de commande).
        assert_eq!(resolve(Some(&dac), None, None), on_dac);
        assert_eq!(resolve(Some(&dac), Some(&list), None), on_dac);
        // Guid en majuscules dans la liste : même périphérique.
        let upper = vec![device(&DAC.to_uppercase().replace("WASAPI", "wasapi"), "F")];
        assert_eq!(resolve(Some(&dac), Some(&upper), None), on_dac);
        // Débranché : défaut du système ; rebranché : de retour sur le choix.
        assert_eq!(resolve(Some(&dac), Some(&[]), None), fallback);
        assert_eq!(resolve(Some(&dac), Some(&list), None), on_dac);
        // Listé mais refusé par mpv : repli aussi.
        assert_eq!(resolve(Some(&dac), Some(&list), Some(DAC)), fallback);
        assert_eq!(
            resolve(Some(&dac), Some(&list), Some("wasapi/{other}")),
            on_dac
        );
    }

    /// WAV PCM 16 bits stéréo 44,1 kHz de silence (fichier de test, pas un flux).
    fn silent_wav(secs: u32) -> std::path::PathBuf {
        let data_len: u32 = 44_100 * 4 * secs;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        for (v, n) in [
            (16u32, 4),
            (1, 2),
            (2, 2),
            (44_100, 4),
            (44_100 * 4, 4),
            (4, 2),
            (16, 2),
        ] {
            wav.extend_from_slice(&v.to_le_bytes()[..n]);
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.resize(44 + data_len as usize, 0);
        let path = std::env::temp_dir().join(format!("seaglass-device-{}.wav", std::process::id()));
        std::fs::write(&path, &wav).unwrap();
        path
    }

    /// Vrai mpv sur la vraie sortie WASAPI (silence) : `audio-device-list` est notifiée dès
    /// l'observation et contient le périphérique par défaut de Windows ; un périphérique
    /// absent fait échouer le fichier avec [`INIT_FAILED`] ; changer `audio-device` pendant
    /// la lecture ne l'interrompt pas.
    /// `cargo test live_audio_devices -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "lance un vrai mpv sur la sortie audio"]
    async fn live_audio_devices() {
        use crate::mpv::CacheSize;
        use crate::mpv::events::{EndReason, MpvEvent, Property};
        use std::time::Duration;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let bogus = "wasapi/{00000000-0000-0000-0000-000000000000}";
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32, Some(bogus)).unwrap();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(256);
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();
        let cmd = |c: Vec<Value>| {
            let handle = handle.clone();
            async move { handle.command(c).await }
        };
        async fn wait_for(
            events: &mut tokio::sync::mpsc::Receiver<MpvEvent>,
            pred: &dyn Fn(&MpvEvent) -> bool,
        ) -> MpvEvent {
            tokio::time::timeout(Duration::from_secs(10), async {
                while let Some(ev) = events.recv().await {
                    if pred(&ev) {
                        return ev;
                    }
                }
                panic!("mpv fermé");
            })
            .await
            .expect("événement attendu")
        }

        let MpvEvent::Property(Property::AudioDeviceList(list)) = wait_for(&mut events, &|ev| {
            matches!(ev, MpvEvent::Property(Property::AudioDeviceList(_)))
        })
        .await
        else {
            unreachable!()
        };
        println!("périphériques : {list:#?}");
        assert!(!list.is_empty(), "aucun périphérique WASAPI");
        let default = system_default_id().expect("périphérique par défaut de Windows");
        println!("défaut de Windows : {default}");
        let default_dev = find(&list, &default).expect("défaut absent de la liste de mpv");

        // Périphérique passé en ligne de commande mais absent : le fichier échoue.
        let wav = silent_wav(6);
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        let ev = wait_for(&mut events, &|ev| matches!(ev, MpvEvent::EndFile { .. })).await;
        println!("absent : {ev:?}");
        assert_eq!(
            ev,
            MpvEvent::EndFile {
                reason: EndReason::Error,
                error: Some(INIT_FAILED.into())
            }
        );

        // Périphérique réel : lecture, puis bascule à chaud vers `auto`.
        cmd(vec![
            json!("set_property"),
            json!("audio-device"),
            json!(default_dev.id),
        ])
        .await
        .unwrap();
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        wait_for(&mut events, &|ev| {
            matches!(ev, MpvEvent::Property(Property::CurrentAo(Some(_))))
        })
        .await;
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let pos = |v: Value| v.as_f64().unwrap_or(0.0);
        let before = pos(cmd(vec![json!("get_property"), json!("time-pos")])
            .await
            .unwrap());
        cmd(vec![
            json!("set_property"),
            json!("audio-device"),
            json!(AUTO),
        ])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let after = pos(cmd(vec![json!("get_property"), json!("time-pos")])
            .await
            .unwrap());
        println!("position avant bascule {before:.2} s, après {after:.2} s");
        assert!(before > 0.5 && after > before + 0.5, "{before} → {after}");
        let ao = cmd(vec![json!("get_property"), json!("current-ao")])
            .await
            .unwrap();
        assert_eq!(ao, json!("wasapi"));

        let _ = child.start_kill();
        let _ = std::fs::remove_file(&wav);
    }
}
