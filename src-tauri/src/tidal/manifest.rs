//! Décodage des manifestes de `playbackinfopostpaywall` et détection de DRM.
//!
//! Constat du spike : le MPD déclare toujours `xmlns:cenc`, même en clair. Seuls les
//! éléments et attributs ci-dessous signalent un vrai chiffrement.

use base64::Engine;
use serde::Deserialize;

use super::models::StreamSource;
use super::{Result, TidalError};

const MPD_MIME: &str = "application/dash+xml";
const BTS_MIME: &str = "application/vnd.tidal.bts";

const MPD_DRM_MARKERS: [&str; 4] = [
    "<ContentProtection",
    "cenc:default_KID",
    "cenc:pssh",
    "edef8ba9-79d6-4ace-a3c8-27dcd51d21ed", // UUID système Widevine
];

/// Boîtes MP4 présentes dans un segment d'initialisation chiffré (CENC).
const INIT_DRM_BOXES: [&str; 4] = ["pssh", "tenc", "sinf", "enca"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BtsManifest {
    mime_type: Option<String>,
    codecs: Option<String>,
    encryption_type: Option<String>,
    urls: Vec<String>,
}

pub struct Decoded {
    pub source: StreamSource,
    pub codec: String,
    /// URL du segment d'initialisation DASH, à inspecter avant lecture.
    pub init_url: Option<String>,
    /// Débit moyen déclaré par le MPD (`bandwidth`), en kbps (#3). Absent des manifestes BTS.
    pub bitrate: Option<u32>,
}

pub fn decode(mime: &str, manifest_b64: &str) -> Result<Decoded> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(manifest_b64)
        .map_err(|e| TidalError::Decode(format!("manifeste base64 : {e}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|e| TidalError::Decode(format!("manifeste UTF-8 : {e}")))?;

    if mime.starts_with(MPD_MIME) {
        if let Some(marker) = MPD_DRM_MARKERS.iter().find(|m| text.contains(**m)) {
            return Err(TidalError::Drm((*marker).to_owned()));
        }
        let codec = xml_attr(&text, "codecs").unwrap_or_else(|| "?".to_owned());
        let init_url = xml_attr(&text, "initialization");
        let bitrate = xml_attr(&text, "bandwidth")
            .and_then(|b| b.parse::<u64>().ok())
            .and_then(|bps| u32::try_from((bps + 500) / 1000).ok());
        Ok(Decoded {
            source: StreamSource::Dash(text),
            codec,
            init_url,
            bitrate,
        })
    } else if mime.starts_with(BTS_MIME) {
        let bts: BtsManifest = serde_json::from_str(&text)
            .map_err(|e| TidalError::Decode(format!("manifeste BTS : {e}")))?;
        match bts.encryption_type.as_deref() {
            None | Some("NONE") => {}
            Some(other) => return Err(TidalError::Drm(other.to_owned())),
        }
        let url = bts
            .urls
            .into_iter()
            .next()
            .ok_or_else(|| TidalError::Decode("manifeste BTS sans URL".into()))?;
        Ok(Decoded {
            source: StreamSource::Direct(url),
            codec: bts.codecs.or(bts.mime_type).unwrap_or_else(|| "?".into()),
            init_url: None,
            bitrate: None,
        })
    } else {
        Err(TidalError::UnsupportedStream(mime.to_owned()))
    }
}

/// Renvoie la boîte de chiffrement trouvée dans un segment d'initialisation, le cas échéant.
pub fn find_init_drm_box(init: &[u8]) -> Option<&'static str> {
    INIT_DRM_BOXES
        .into_iter()
        .find(|b| init.windows(4).any(|w| w == b.as_bytes()))
}

/// Lecture minimale d'un attribut XML (`name="…"`), entités `&amp;` décodées.
fn xml_attr(xml: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = xml.find(&needle)? + needle.len();
    let end = xml[start..].find('"')? + start;
    Some(xml[start..end].replace("&amp;", "&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    const CLEAR_MPD: &str = r#"<?xml version='1.0' encoding='UTF-8'?><MPD xmlns="urn:mpeg:dash:schema:mpd:2011" xmlns:cenc="urn:mpeg:cenc:2013"><Period><AdaptationSet><Representation id="FLAC,44100,24" codecs="flac" bandwidth="892938" audioSamplingRate="44100"><SegmentTemplate initialization="https://x/0.mp4?a=1&amp;b=2" media="https://x/$Number$.mp4"/></Representation></AdaptationSet></Period></MPD>"#;

    #[test]
    fn cenc_namespace_alone_is_not_drm() {
        let d = decode(MPD_MIME, &b64(CLEAR_MPD)).unwrap();
        assert_eq!(d.codec, "flac");
        assert_eq!(d.init_url.as_deref(), Some("https://x/0.mp4?a=1&b=2"));
        assert_eq!(d.bitrate, Some(893), "bandwidth en bit/s, arrondi en kbps");
        assert!(matches!(d.source, StreamSource::Dash(_)));
    }

    #[test]
    fn content_protection_is_drm() {
        let mpd = CLEAR_MPD.replace(
            "<Representation",
            r#"<ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"/><Representation"#,
        );
        assert!(matches!(
            decode(MPD_MIME, &b64(&mpd)),
            Err(TidalError::Drm(_))
        ));
    }

    #[test]
    fn bts_clear_and_encrypted() {
        let clear = r#"{"mimeType":"audio/mp4","codecs":"mp4a.40.2","encryptionType":"NONE","urls":["https://x/a.m4a"]}"#;
        let d = decode(BTS_MIME, &b64(clear)).unwrap();
        assert!(matches!(d.source, StreamSource::Direct(u) if u == "https://x/a.m4a"));
        assert_eq!(d.bitrate, None);

        let enc = clear.replace("\"NONE\"", "\"OLD_AES\"");
        assert!(matches!(
            decode(BTS_MIME, &b64(&enc)),
            Err(TidalError::Drm(_))
        ));
    }

    #[test]
    fn unknown_mime_is_rejected() {
        assert!(matches!(
            decode("application/vnd.tidal.emu", &b64("{}")),
            Err(TidalError::UnsupportedStream(_))
        ));
    }

    #[test]
    fn init_segment_boxes() {
        assert_eq!(find_init_drm_box(b"\0\0\0\x20ftypiso6moov"), None);
        assert_eq!(find_init_drm_box(b"....moov....pssh...."), Some("pssh"));
    }
}
