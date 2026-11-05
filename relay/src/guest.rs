//! Côté invités : page de la Jam, appels de l'API transmis à l'hôte, flux d'état.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use futures_util::stream;

use crate::Relay;
use crate::jams::{CodeError, ForwardError};

/// La page est celle de la Jam locale : un seul fichier à maintenir.
const PAGE: &str = include_str!("../../src-tauri/src/jam/page/index.html");
const SCRIPT: &str = include_str!("../../src-tauri/src/jam/page/jam.js");
const STYLE: &str = include_str!("../../src-tauri/src/jam/page/jam.css");

/// Même politique que la Jam locale.
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
                   img-src https://resources.tidal.com; connect-src 'self'; \
                   base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

const ENDED: &str = "Cette Jam est terminée. / This Jam has ended.";

fn reply(status: StatusCode, content_type: &'static str, body: impl Into<String>) -> Response {
    let mut response = (status, body.into()).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    secure(headers);
    response
}

fn secure(headers: &mut HeaderMap) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
}

fn error(status: StatusCode, kind: &str) -> Response {
    reply(
        status,
        "application/json",
        serde_json::json!({ "error": kind }).to_string(),
    )
}

fn ended() -> Response {
    reply(StatusCode::NOT_FOUND, "text/plain; charset=utf-8", ENDED)
}

fn exists(relay: &Relay, id: &str) -> bool {
    relay.jams.get(id).is_some()
}

pub async fn redirect(State(relay): State<Arc<Relay>>, Path(id): Path<String>) -> Response {
    if !exists(&relay, &id) {
        return ended();
    }
    Redirect::permanent(&format!("/j/{id}/")).into_response()
}

pub async fn page(State(relay): State<Arc<Relay>>, Path(id): Path<String>) -> Response {
    if !exists(&relay, &id) {
        return ended();
    }
    reply(StatusCode::OK, "text/html; charset=utf-8", PAGE)
}

pub async fn script() -> Response {
    reply(StatusCode::OK, "text/javascript; charset=utf-8", SCRIPT)
}

pub async fn style() -> Response {
    reply(StatusCode::OK, "text/css; charset=utf-8", STYLE)
}

/// Appel de l'API (`join`, `search`, `add`, `state`) transmis tel quel à l'hôte.
pub async fn api(
    State(relay): State<Arc<Relay>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path((id, call)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let ip = relay.client_ip(peer, &headers);
    if !Relay::allow(&relay.requests, ip) {
        return error(StatusCode::TOO_MANY_REQUESTS, "slow");
    }
    let Some(jam) = relay.jams.get(&id) else {
        return error(StatusCode::NOT_FOUND, "ended");
    };
    let Ok(body) = String::from_utf8(body.to_vec()) else {
        return error(StatusCode::BAD_REQUEST, "body");
    };
    let code = headers
        .get("x-jam-code")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    match jam.forward(ip, format!("/api/{call}"), code, body).await {
        Ok(answer) => {
            let status = StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY);
            reply(status, "application/json", answer.body)
        }
        // L'hôte a perdu le réseau : la page le dit, il peut revenir.
        Err(ForwardError::Offline) => error(StatusCode::SERVICE_UNAVAILABLE, "offline"),
        Err(ForwardError::Busy) => error(StatusCode::SERVICE_UNAVAILABLE, "busy"),
    }
}

/// Flux de l'état (`EventSource`, code dans la query : il ne pose pas d'en-tête).
pub async fn events(
    State(relay): State<Arc<Relay>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let ip = relay.client_ip(peer, &headers);
    let Some(jam) = relay.jams.get(&id) else {
        return error(StatusCode::NOT_FOUND, "ended");
    };
    let given = query.get("code").map_or("", String::as_str);
    match jam.check_code(ip, given) {
        Ok(()) => {}
        Err(CodeError::Wrong) => return error(StatusCode::UNAUTHORIZED, "code"),
        Err(CodeError::Blocked) => return error(StatusCode::TOO_MANY_REQUESTS, "blocked"),
    }
    let Some(slot) = jam.open_stream(relay.config.max_streams) else {
        return error(StatusCode::TOO_MANY_REQUESTS, "slow");
    };
    let Some(mut updates) = jam.subscribe() else {
        return error(StatusCode::NOT_FOUND, "ended");
    };
    drop(jam);
    updates.mark_changed();
    let first = Event::default()
        .retry(Duration::from_secs(3))
        .comment("seaglass");
    let changes = stream::unfold((updates, slot), |(mut updates, slot)| async move {
        loop {
            // Jam fermée : le flux se termine.
            updates.changed().await.ok()?;
            let data = Arc::clone(&updates.borrow_and_update());
            if !data.is_empty() {
                let event = Event::default().data(&*data);
                return Some((Ok::<_, Infallible>(event), (updates, slot)));
            }
        }
    });
    let events = futures_util::StreamExt::chain(stream::once(async { Ok(first) }), changes);
    let mut response = Sse::new(events)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
        .into_response();
    secure(response.headers_mut());
    response
}
