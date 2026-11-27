//! Guests' answers to the host's blind test (#441). The game (rounds, points,
//! reveal) is held by the player actor; here, the Jam's safeguards.

use std::net::IpAddr;

use serde::Deserialize;

use super::{Backend, Core, backend_error};
use crate::jam::guard::Action;
use crate::jam::guests::sanitize_name;
use crate::jam::http::Response;
use crate::player::blind::{Answer, Refusal};

/// A longer entry is cut (same cap as in the app).
const MAX_ANSWER_CHARS: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnswerBody {
    round: usize,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    choice: Option<usize>,
}

impl Core {
    /// `POST api/blind-answer {guest, name?, round, text | choice}`: empty = skip.
    pub(super) async fn blind_answer<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let Ok(body) = serde_json::from_slice::<AnswerBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.allow(ip, key, Action::Answer) {
            return Response::error(429, "slow");
        }
        let answer = match (body.choice, body.text) {
            (Some(index), _) => Answer::Choice { index },
            (None, Some(text)) if !text.trim().is_empty() => Answer::Text {
                text: text.trim().chars().take(MAX_ANSWER_CHARS).collect(),
            },
            _ => Answer::Pass,
        };
        let name = body.name.as_deref().and_then(sanitize_name);
        let result = backend
            .blind_answer(key.to_owned(), name.clone(), body.round, answer)
            .await;
        match result {
            Ok(Ok(player)) => {
                // Player known to the host, who can exclude them like a guest who adds.
                if let Ok(mut guests) = self.guests.lock() {
                    guests.members.added(key, name, ip, 0);
                    backend.guests_changed(&guests);
                }
                self.refresh.notify_one();
                Response::json(200, &serde_json::json!({ "player": player }))
            }
            Ok(Err(Refusal::Closed)) => Response::error(409, "round"),
            Ok(Err(Refusal::Duplicate)) => Response::error(409, "answered"),
            Ok(Err(Refusal::Full)) => Response::error(429, "players"),
            Err(e) => backend_error(&e),
        }
    }
}
