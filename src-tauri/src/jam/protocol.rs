//! Messages exchanged with the relay (#25), copy of `relay/src/protocol.rs`: the two
//! must stay identical (same JSON). The host connects by WebSocket to `/host`
//! (`Authorization: Bearer <build key>`), says `hello`, receives `ready`, then answers the
//! guests' `request`s and pushes `state` when the state changes.
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
        /// The Jam's access code: the relay checks it for the state streams.
        code: String,
        #[serde(default)]
        resume: Option<Resume>,
    },
    Response {
        rid: u64,
        status: u16,
        body: String,
    },
    /// Guests' state (`GuestState` JSON), pushed as is to the pages.
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
        /// The guest's address, for the host's limits.
        client: String,
        path: String,
        code: Option<String>,
        body: String,
    },
    Error {
        message: String,
    },
}

/// Device tunnel (#430, `GET /device`): the PC reachable by its linked phone outside the
/// local network. The bodies are end-to-end encrypted packets (`handoff::sealed`).
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
        /// The phone's address, for the PC's limits.
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

    #[test]
    fn device_messages_match_the_relay() {
        let hello = serde_json::to_value(FromDevice::Hello {
            id: "i".into(),
            secret: "s".into(),
        })
        .unwrap();
        assert_eq!(
            hello,
            serde_json::json!({"type": "hello", "id": "i", "secret": "s"})
        );
        let request: ToDevice =
            serde_json::from_str(r#"{"type":"request","rid":3,"client":"1.2.3.4","body":"x"}"#)
                .unwrap();
        assert!(matches!(request, ToDevice::Request { rid: 3, .. }));
    }
}
