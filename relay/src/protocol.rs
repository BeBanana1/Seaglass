//! Messages échangés entre l'app de l'hôte et le relais, en JSON sur un WebSocket
//! (`GET /host`, en-tête `Authorization: Bearer <clé de build>`). L'app (étape 3b)
//! reprend ces types à l'identique.
//!
//! 1. L'hôte envoie `hello` (code d'accès, et la Jam à reprendre après une coupure).
//! 2. Le relais répond `ready` (identifiant, secret de reprise, adresse publique).
//! 3. Chaque appel d'un invité à l'API arrive en `request` ; l'hôte répond `response`
//!    avec le même `rid`. Il applique ses propres règles (code, limites, titres proposés),
//!    `client` tenant lieu d'adresse de l'invité.
//! 4. L'hôte envoie `state` quand l'état change : le relais le pousse aux pages ouvertes
//!    (`GET /j/<id>/api/events`), qu'il sert lui-même.

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

/// Tunnel d'appareil (#430, `GET /device`) : le PC d'un utilisateur, joignable par son
/// téléphone lié. Les corps sont des paquets chiffrés de bout en bout, opaques ici.
///
/// 1. Le PC envoie `hello` avec l'identifiant et le secret qu'il a tirés lui-même.
/// 2. Le relais répond `ready`, ou `error` (identifiant pris, relais plein).
/// 3. Chaque `POST /d/<id>` du téléphone arrive en `request` ; le PC répond `response`.
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
        /// `https://<relais>/d/<id>` : l'adresse que le téléphone appelle.
        url: String,
    },
    Request {
        rid: u64,
        /// Adresse du téléphone, pour les limites du PC.
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
