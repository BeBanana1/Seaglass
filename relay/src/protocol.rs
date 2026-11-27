//! Messages exchanged between the host's app and the relay, as JSON over a WebSocket
//! (`GET /host`, header `Authorization: Bearer <build key>`). The app (step 3b)
//! reuses these types as they are.
//!
//! 1. The host sends `hello` (access code, and the Jam to resume after a cut).
//! 2. The relay answers `ready` (identifier, resume secret, public address).
//! 3. Each guest call to the API arrives as a `request`; the host answers `response`
//!    with the same `rid`. It applies its own rules (code, limits, suggested titles),
//!    `client` standing in for the guest's address.
//! 4. The host sends `state` when the state changes: the relay pushes it to the open pages
//!    (`GET /j/<id>/api/events`), which it serves itself.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resume {
    pub id: String,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FromHost {
    Hello {
        /// Access code of the Jam: the relay checks it for the state streams.
        code: String,
        #[serde(default)]
        resume: Option<Resume>,
    },
    Response {
        rid: u64,
        status: u16,
        body: String,
    },
    /// State of the guests (JSON of `GuestState`), pushed as is to the pages.
    State {
        data: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToHost {
    Ready {
        id: String,
        secret: String,
        /// `https://<relay>/j/<id>/`: to put in the QR code, followed by `#<code>`.
        url: String,
        /// True if the Jam from before the cut was resumed (same address).
        resumed: bool,
    },
    Request {
        rid: u64,
        /// Address of the guest, for the host's limits.
        client: String,
        path: String,
        code: Option<String>,
        body: String,
    },
    Error {
        message: String,
    },
}

/// Device tunnel (#430, `GET /device`): a user's PC, reachable by its paired phone.
/// The bodies are end-to-end encrypted packets, opaque here.
///
/// 1. The PC sends `hello` with the identifier and the secret it drew itself.
/// 2. The relay answers `ready`, or `error` (identifier taken, relay full).
/// 3. Each `POST /d/<id>` from the phone arrives as a `request`; the PC answers `response`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FromDevice {
    Hello { id: String, secret: String },
    Response { rid: u64, status: u16, body: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToDevice {
    Ready {
        /// `https://<relay>/d/<id>`: the address the phone calls.
        url: String,
    },
    Request {
        rid: u64,
        /// Address of the phone, for the PC's limits.
        client: String,
        body: String,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_use_a_type_tag() {
        let hello: FromHost = serde_json::from_str(r#"{"type":"hello","code":"AB23CD"}"#).unwrap();
        assert_eq!(
            hello,
            FromHost::Hello {
                code: "AB23CD".into(),
                resume: None
            }
        );
        let ready = serde_json::to_value(ToHost::Ready {
            id: "x".into(),
            secret: "s".into(),
            url: "u".into(),
            resumed: false,
        })
        .unwrap();
        assert_eq!(ready["type"], "ready");
    }
}
