//! Pasted TIDAL links (#203): `tidal.com/browse/<type>/<id>`, `tidal.com/<type>/<id>`,
//! `listen.tidal.com/…`, with or without scheme, trailing `/u` or parameters, possibly in the
//! middle of a share text. The text is never sent as is: only the type and
//! a validated identifier (numeric, uuid or mix id) come out of it. On Android, the same
//! text also arrives through the system "Share" menu and through tapped links (`share`).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::catalog::{is_mix_id, is_uuid};
use super::{AppState, CmdResult, err};
use crate::foreign::playlists::{self, PlaylistRef};
use crate::i18n::tr;
use crate::tidal::TidalClient;
use crate::tidal::models::TrackId;

/// A longer pasted text is not a link.
const MAX_TEXT_LEN: usize = 2_000;

/// Page to open for a link.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LinkTarget {
    /// A track opens its album, with the track highlighted (`track`).
    #[serde(rename_all = "camelCase")]
    Album {
        id: u64,
        track: Option<TrackId>,
    },
    Artist {
        id: u64,
    },
    Playlist {
        uuid: String,
    },
    Mix {
        id: String,
    },
    /// Playlist from another service to import (#328): `link` = clean address, read again by
    /// `import_playlist`.
    Import {
        link: String,
    },
    /// News feed: grouped notification of new releases (#435).
    Feed,
}

/// Internal link to the feed, given by Android's grouped notification (#435) to `ShareActivity`.
pub(crate) const FEED_LINK: &str = "seaglass://feed";

/// What a link designates, before resolving a track into an album.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Link {
    Track(u64),
    Album(u64),
    Artist(u64),
    Playlist(String),
    Mix(String),
}

/// Page to open for the pasted text, or `null` if it contains no TIDAL link.
#[tauri::command]
pub async fn open_link(state: State<'_, AppState>, text: String) -> CmdResult<Option<LinkTarget>> {
    resolve(&state.tidal, &text).await
}

/// Link shared to the app on Android ("Share" menu, #203) or page of a tapped
/// notification (#435), kept until the UI takes it: the app may start with it, before
/// the WebView is listening.
static SHARED: Mutex<Option<LinkTarget>> = Mutex::new(None);

/// Event emitted when a shared link is ready (`take_shared_link` takes it).
pub const SHARED_LINK_EVENT: &str = "open-link";

/// Text received through the "Share" menu: resolved off the calling thread, kept, then
/// announced to the UI. A text without a TIDAL link gives a toast.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn share(app: &AppHandle, text: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match resolve(&state.tidal, &text).await {
            Ok(Some(target)) => open(&app, target),
            Ok(None) => toast(
                &app,
                tr(
                    "Aucun lien TIDAL dans le texte partagé",
                    "No TIDAL link in the shared text",
                )
                .into(),
            ),
            Err(e) => toast(&app, e),
        }
    });
}

/// Keeps `target` for the UI and announces it: "Share" menu, tap on a notification of
/// new releases (#435).
pub fn open(app: &AppHandle, target: LinkTarget) {
    if let Ok(mut shared) = SHARED.lock() {
        *shared = Some(target);
    }
    if let Err(e) = app.emit(SHARED_LINK_EVENT, ()) {
        tracing::warn!("shared link not announced to the UI: {e}");
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn toast(app: &AppHandle, message: String) {
    #[derive(Clone, serde::Serialize)]
    struct Toast {
        level: &'static str,
        message: String,
    }
    let toast = Toast {
        level: "error",
        message,
    };
    if let Err(e) = app.emit("toast", toast) {
        tracing::warn!("toast not emitted: {e}");
    }
}

/// Pending shared link, only once.
#[tauri::command]
pub async fn take_shared_link() -> CmdResult<Option<LinkTarget>> {
    Ok(SHARED.lock().ok().and_then(|mut s| s.take()))
}

/// `https://tidal.com/…` links given to the app on Android 12+ (#203): the domain is not
/// ours, the user must enable them in "Open by default".
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct LinkHandling {
    pub enabled: bool,
}

/// State of the TIDAL links, or `null` if there is nothing to set (desktop, Android before 12).
#[tauri::command]
pub async fn link_handling() -> CmdResult<Option<LinkHandling>> {
    #[cfg(target_os = "android")]
    {
        #[derive(serde::Deserialize)]
        struct State {
            supported: bool,
            enabled: bool,
        }
        let state: State =
            tauri_plugin_seaglass_mobile::direct::call("linkHandling", serde_json::json!({}))
                .await?;
        Ok(state.supported.then_some(LinkHandling {
            enabled: state.enabled,
        }))
    }
    #[cfg(not(target_os = "android"))]
    Ok(None)
}

/// Opens the app's "Open by default" setting (Android only).
#[tauri::command]
pub async fn open_link_settings() -> CmdResult<()> {
    #[cfg(target_os = "android")]
    {
        tauri_plugin_seaglass_mobile::direct::call::<serde::de::IgnoredAny>(
            "openLinkSettings",
            serde_json::json!({}),
        )
        .await?;
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    Err(tr("Réglage propre à Android", "Android-only setting").into())
}

async fn resolve(tidal: &TidalClient, text: &str) -> CmdResult<Option<LinkTarget>> {
    if text.trim() == FEED_LINK {
        return Ok(Some(LinkTarget::Feed));
    }
    let Some(link) = parse(text) else {
        // Playlist from another service (#328): the import page opens.
        return match crate::foreign::playlist_ref(text).await {
            Some(Ok(PlaylistRef::YouTube)) => Err(playlists::youtube_unsupported()),
            Some(Ok(playlist)) => Ok(Some(LinkTarget::Import {
                link: playlist.url(),
            })),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        };
    };
    Ok(Some(match link {
        Link::Track(id) => {
            let track = tidal.track(TrackId(id)).await.map_err(err)?;
            LinkTarget::Album {
                id: track.album.id,
                track: Some(track.id),
            }
        }
        Link::Album(id) => LinkTarget::Album { id, track: None },
        Link::Artist(id) => LinkTarget::Artist { id },
        Link::Playlist(uuid) => LinkTarget::Playlist { uuid },
        Link::Mix(id) => LinkTarget::Mix { id },
    }))
}

pub(crate) fn parse(text: &str) -> Option<Link> {
    if text.len() > MAX_TEXT_LEN {
        return None;
    }
    text.split_whitespace().find_map(parse_url)
}

fn parse_url(word: &str) -> Option<Link> {
    let lower = word.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let (host, path) = rest.split_once('/')?;
    if !matches!(host, "tidal.com" | "www.tidal.com" | "listen.tidal.com") {
        return None;
    }
    // Identifiers keep their case (mix ids): the path is read again from the word.
    let path = &word[word.len() - path.len()..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // Last valid pair: `album/1/track/2` designates the track.
    segments
        .windows(2)
        .rev()
        .find_map(|pair| link(&pair[0].to_ascii_lowercase(), pair[1]))
}

fn link(kind: &str, id: &str) -> Option<Link> {
    let number = || {
        (id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit()))
            .then(|| id.parse::<u64>().ok())
            .flatten()
            .filter(|&n| n > 0)
    };
    match kind {
        "track" => number().map(Link::Track),
        "album" => number().map(Link::Album),
        "artist" => number().map(Link::Artist),
        "playlist" => is_uuid(id).then(|| Link::Playlist(id.to_ascii_lowercase())),
        "mix" => is_mix_id(id).then(|| Link::Mix(id.to_owned())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Link, parse};

    #[test]
    fn known_forms() {
        let cases = [
            (
                "https://tidal.com/browse/track/534050211",
                Link::Track(534050211),
            ),
            (
                "https://tidal.com/track/534050211/u",
                Link::Track(534050211),
            ),
            (
                "tidal.com/browse/album/156749593?play=true",
                Link::Album(156749593),
            ),
            (
                "http://www.tidal.com/album/156749593",
                Link::Album(156749593),
            ),
            ("https://listen.tidal.com/artist/12979", Link::Artist(12979)),
            ("https://listen.tidal.com/album/1/track/2", Link::Track(2)),
            (
                "https://tidal.com/browse/playlist/4BA30155-d807-446f-9e53-b67520ccfe83",
                Link::Playlist("4ba30155-d807-446f-9e53-b67520ccfe83".into()),
            ),
            (
                "https://tidal.com/browse/mix/0017d17ec5596B20cae3eadc63b948#x",
                Link::Mix("0017d17ec5596B20cae3eadc63b948".into()),
            ),
            (
                "Écoute « Time » sur TIDAL https://tidal.com/track/12/u",
                Link::Track(12),
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(parse(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn rejected() {
        for text in [
            "",
            "bonjour",
            "https://evil.com/browse/track/1",
            "https://tidal.com.evil.com/track/1",
            "https://tidal.com/browse/track/abc",
            "https://tidal.com/browse/track/0",
            "https://tidal.com/browse/playlist/../../users/1",
            "https://tidal.com/browse/mix/abc%2F..",
            "https://tidal.com/browse/video/123",
            "https://tidal.com/browse/track/99999999999999999999999",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
        assert_eq!(
            parse(&format!("https://tidal.com/track/1 {}", "x".repeat(2_000))),
            None
        );
    }
}
