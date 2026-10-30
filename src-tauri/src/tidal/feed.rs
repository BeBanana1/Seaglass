//! Fil d'actualité : nouveautés des artistes suivis.
//!
//! Endpoint vérifié (2026-10-30) : v2 `feed/activities?userId=…` →
//! `{ activities: [{ followableActivity: { activityType, occurredAt, <objet> }, seen }],
//! stats: { totalNotSeenActivities } }`. Types vus : `NEW_ALBUM_RELEASE` (avec `album`,
//! même format que `albums/{id}`) et `NEW_HISTORY_MIX` (avec `historyMix`). Pas de
//! pagination : `offset` est ignoré, `limit` tronque seulement, la fenêtre plafonne vers
//! 50 activités. L'état `seen` est lu, jamais modifié.

use serde::Deserialize;

use super::api::{API_V2, TidalClient};
use super::date::timestamp_ms;
use super::models::FeedItem;
use super::raw::RawAlbumFull;
use super::{Result, TidalError};

const FEED_LOCALE: &str = "fr_FR";

#[derive(Deserialize)]
struct RawFeed {
    #[serde(default)]
    activities: Vec<RawFeedEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFeedEntry {
    followable_activity: RawActivity,
    #[serde(default)]
    seen: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawActivity {
    activity_type: String,
    occurred_at: Option<String>,
    /// Décodé élément par élément : un album illisible ne doit pas vider tout le fil.
    #[serde(default)]
    album: serde_json::Value,
}

impl TidalClient {
    /// Sorties récentes des artistes suivis, de la plus récente à la plus ancienne.
    /// Les mixes (`NEW_HISTORY_MIX`) et les types inconnus sont ignorés pour l'instant.
    pub async fn feed(&self) -> Result<Vec<FeedItem>> {
        let user = self.session().await.ok_or(TidalError::NotLoggedIn)?.user_id;
        let raw: RawFeed = self
            .get_at(
                API_V2,
                "feed/activities",
                &[
                    ("userId", user.to_string()),
                    ("deviceType", "BROWSER".to_owned()),
                    ("locale", FEED_LOCALE.to_owned()),
                    ("platform", "WEB".to_owned()),
                ],
            )
            .await?;
        Ok(feed_items(raw))
    }
}

fn feed_items(raw: RawFeed) -> Vec<FeedItem> {
    raw.activities
        .into_iter()
        .filter_map(|entry| {
            let activity = entry.followable_activity;
            if activity.activity_type != "NEW_ALBUM_RELEASE" {
                return None;
            }
            let album = serde_json::from_value::<RawAlbumFull>(activity.album).ok()?;
            Some(FeedItem {
                occurred_at: activity.occurred_at.as_deref().and_then(timestamp_ms),
                seen: entry.seen,
                album: album.into(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_album_releases_only() {
        let raw: RawFeed = serde_json::from_value(json!({
            "activities": [
                {"followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                    "occurredAt": "2026-10-30T00:00:00.000Z",
                    "album": {"id": 1, "title": "a", "type": "SINGLE",
                              "artists": [{"id": 2, "name": "b"}]}},
                 "seen": false},
                {"followableActivity": {"activityType": "NEW_HISTORY_MIX",
                    "occurredAt": "2026-10-01T00:00:00.000Z", "historyMix": {"id": "m"}},
                 "seen": true},
                {"followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                    "album": {"title": "sans id"}}},
                {"followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                    "occurredAt": "illisible", "album": {"id": 3, "title": "c"}}, "seen": true}
            ],
            "stats": {"totalNotSeenActivities": 1}
        }))
        .unwrap();
        let items = feed_items(raw);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].album.id, 1);
        assert_eq!(items[0].album.album_type.as_deref(), Some("SINGLE"));
        assert_eq!(items[0].occurred_at, Some(1_793_318_400_000));
        assert!(!items[0].seen);
        assert_eq!(items[1].occurred_at, None);
        assert!(items[1].seen);
    }
}
