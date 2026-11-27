//! Activity feed: new releases from followed artists and monthly mixes.
//!
//! Endpoint checked (2026-10-30): v2 `feed/activities?userId=…` →
//! `{ activities: [{ followableActivity: { activityType, occurredAt, <object> }, seen }],
//! stats: { totalNotSeenActivities } }`. Types seen: `NEW_ALBUM_RELEASE` (with `album`,
//! same format as `albums/{id}`) and `NEW_HISTORY_MIX` (with `historyMix`, mix
//! `HISTORY_MONTHLY_MIX` in the format of `favorites/mixes`, read by `RawMix`). No
//! paging: `offset` is ignored, `limit` only truncates, the window caps around
//! 50 activities. The `seen` state is read, never modified.

use serde::Deserialize;

use super::api::{API_V2, TidalClient};
use super::date::timestamp_ms;
use super::mix::RawMix;
use super::models::{FeedItem, MediaItem};
use super::raw::RawAlbumFull;
use super::{Result, TidalError};

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
    /// Decoded item by item: an unreadable object must not empty the whole feed.
    #[serde(default)]
    album: serde_json::Value,
    #[serde(default)]
    history_mix: serde_json::Value,
}

impl RawActivity {
    /// Albums and mixes; unknown types are ignored.
    fn into_item(self) -> Option<MediaItem> {
        match self.activity_type.as_str() {
            "NEW_ALBUM_RELEASE" => serde_json::from_value::<RawAlbumFull>(self.album)
                .ok()
                .map(|a| MediaItem::Album(a.into())),
            "NEW_HISTORY_MIX" => serde_json::from_value::<RawMix>(self.history_mix)
                .ok()?
                .into_summary()
                .map(MediaItem::Mix),
            _ => None,
        }
    }
}

impl TidalClient {
    /// Recent releases from followed artists and monthly mixes, most recent first.
    pub async fn feed(&self) -> Result<Vec<FeedItem>> {
        let user = self.session().await.ok_or(TidalError::NotLoggedIn)?.user_id;
        let raw: RawFeed = self
            .get_at(
                API_V2,
                "feed/activities",
                &[
                    ("userId", user.to_string()),
                    ("deviceType", "BROWSER".to_owned()),
                    ("locale", crate::i18n::tidal_locale().to_owned()),
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
            let occurred_at = activity.occurred_at.as_deref().and_then(timestamp_ms);
            Some(FeedItem {
                occurred_at,
                seen: entry.seen,
                item: activity.into_item()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_albums_and_mixes() {
        let raw: RawFeed = serde_json::from_value(json!({
            "activities": [
                {"followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                    "occurredAt": "2026-10-30T00:00:00.000Z",
                    "album": {"id": 1, "title": "a", "type": "SINGLE",
                              "artists": [{"id": 2, "name": "b"}]}},
                 "seen": false},
                {"followableActivity": {"activityType": "NEW_HISTORY_MIX",
                    "occurredAt": "2026-10-01T00:00:00.000Z",
                    "historyMix": {"id": "m", "mixType": "HISTORY_MONTHLY_MIX",
                        "title": "September 2026",
                        "titleTextInfo": {"color": "#CBADFF", "text": "September 2026"},
                        "subTitle": "a, b et plus",
                        "subTitleTextInfo": {"color": "#CBADFF", "text": "a, b et plus"},
                        "images": {"SMALL": {"url": "https://i/s.jpg", "width": 267, "height": 267}}}},
                 "seen": true},
                {"followableActivity": {"activityType": "NEW_HISTORY_MIX",
                    "historyMix": {"id": "sans titre"}}},
                {"followableActivity": {"activityType": "NEW_VIDEO", "video": {"id": 9}}},
                {"followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                    "album": {"title": "sans id"}}},
                {"followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                    "occurredAt": "illisible", "album": {"id": 3, "title": "c"}}, "seen": true}
            ],
            "stats": {"totalNotSeenActivities": 1}
        }))
        .unwrap();
        let items = feed_items(raw);
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0].item,
            MediaItem::Album(a) if a.id == 1 && a.album_type.as_deref() == Some("SINGLE")));
        assert_eq!(items[0].occurred_at, Some(1_793_318_400_000));
        assert!(!items[0].seen);
        let MediaItem::Mix(mix) = &items[1].item else {
            panic!("mix expected");
        };
        assert_eq!(mix.title, "September 2026");
        assert_eq!(mix.subtitle.as_deref(), Some("a, b et plus"));
        assert_eq!(mix.image_url.as_deref(), Some("https://i/s.jpg"));
        assert_eq!(mix.mix_type.as_deref(), Some("HISTORY_MONTHLY_MIX"));
        assert!(matches!(&items[2].item, MediaItem::Album(a) if a.id == 3));
        assert_eq!(items[2].occurred_at, None);
        assert!(items[2].seen);
    }
}
