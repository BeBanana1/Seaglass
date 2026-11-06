//! Messages échangés avec le relais (#25), copie de `relay/src/protocol.rs` : les deux
//! doivent rester identiques (même JSON). L'hôte se connecte en WebSocket à `/host`
//! (`Authorization: Bearer <clé de build>`), dit `hello`, reçoit `ready`, puis répond aux
//! `request` des invités et pousse `state` quand l'état change.
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
        /// Code d'accès de la Jam : le relais le vérifie pour les flux d'état.
        code: String,
        #[serde(default)]
        resume: Option<Resume>,
    },
    Response {
        rid: u64,
        status: u16,
        body: String,
    },
    /// État des invités (JSON de `GuestState`), poussé tel quel aux pages.
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
        /// `https://<relais>/j/<id>/` : à mettre dans le QR code, suivi de `#<code>`.
        url: String,
        /// Vrai si la Jam d'avant la coupure a été reprise (même adresse).
        resumed: bool,
    },
    Request {
        rid: u64,
        /// Adresse de l'invité, pour les limites de l'hôte.
        client: String,
        path: String,
        code: Option<String>,
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
