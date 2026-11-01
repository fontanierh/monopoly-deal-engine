use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    body::Bytes,
    extract::{Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{watch, Mutex};

use crate::{auth::Credentials, engine::Engine};

const LONG_POLL: Duration = Duration::from_secs(25);
const MAX_BODY: usize = 16 * 1024;

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const PHONE_JS: &str = include_str!("../static/phone.js");
const STYLE_CSS: &str = include_str!("../static/style.css");

/// Builds a fresh game; the host may call it again to restart.
pub type NewGame<E> = Box<dyn Fn() -> Result<E, String> + Send + Sync>;

struct Slot<E> {
    game: E,
    version: u64,
}

pub struct AppState<E: Engine> {
    slot: Mutex<Slot<E>>,
    changed: watch::Sender<u64>,
    creds: Credentials,
    new_game: NewGame<E>,
    allowed_hosts: Vec<String>,
}

impl<E: Engine> AppState<E> {
    /// `addr` must be the address the listener actually bound, so that an
    /// ephemeral `--port 0` yields the real port in the Host allowlist.
    pub fn new(new_game: NewGame<E>, addr: SocketAddr) -> Result<Arc<Self>, String> {
        let game = new_game()?;
        let creds = Credentials::generate(game.num_seats());
        Ok(Arc::new(Self {
            slot: Mutex::new(Slot { game, version: 0 }),
            changed: watch::channel(0).0,
            creds,
            new_game,
            allowed_hosts: allowed_hosts(addr.port()),
        }))
    }

    pub fn credentials(&self) -> &Credentials {
        &self.creds
    }
}

/// Loopback Host header values for `port`. Browsers omit the default port 80.
fn allowed_hosts(port: u16) -> Vec<String> {
    let names = ["127.0.0.1", "localhost", "[::1]"];
    let mut hosts: Vec<String> = names.iter().map(|n| format!("{n}:{port}")).collect();
    if port == 80 {
        hosts.extend(names.iter().map(|n| n.to_string()));
    }
    hosts
}

pub fn router<E: Engine>(state: Arc<AppState<E>>) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { asset("text/html; charset=utf-8", INDEX_HTML) }),
        )
        .route(
            "/app.js",
            get(|| async { asset("text/javascript; charset=utf-8", APP_JS) }),
        )
        .route(
            "/phone.js",
            get(|| async { asset("text/javascript; charset=utf-8", PHONE_JS) }),
        )
        .route(
            "/style.css",
            get(|| async { asset("text/css; charset=utf-8", STYLE_CSS) }),
        )
        .route("/api/cards", get(get_cards::<E>))
        .route("/api/state", get(get_state::<E>))
        .route("/api/action", post(post_action::<E>))
        .route("/api/new-game", post(post_new_game::<E>))
        .layer(middleware::from_fn_with_state(state.clone(), guard::<E>))
        .with_state(state)
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

/// Rejects foreign Host headers (DNS rebinding) and cross-origin writes, and
/// sets restrictive response headers.
async fn guard<E: Engine>(
    State(st): State<Arc<AppState<E>>>,
    req: Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if !st.allowed_hosts.iter().any(|h| h == host) {
        return error(StatusCode::FORBIDDEN, "unexpected Host header");
    }
    if req.method() != Method::GET {
        if let Some(origin) = req.headers().get(header::ORIGIN) {
            if origin.to_str().ok() != Some(&format!("http://{host}")) {
                return error(StatusCode::FORBIDDEN, "cross-origin request");
            }
        }
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
        ),
    );
    res
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// No token means spectator; a token that matches no seat is an error so a
/// mistyped link does not silently become a spectator view.
fn viewer<E: Engine>(st: &AppState<E>, headers: &HeaderMap) -> Result<Option<usize>, UnknownToken> {
    match bearer(headers) {
        None => Ok(None),
        Some(t) => st.creds.seat_for(t).map(Some).ok_or(UnknownToken),
    }
}

struct UnknownToken;

impl IntoResponse for UnknownToken {
    fn into_response(self) -> Response {
        error(StatusCode::UNAUTHORIZED, "unknown seat token")
    }
}

fn snapshot<E: Engine>(slot: &Slot<E>, seat: Option<usize>) -> Value {
    let actions = match seat {
        Some(s) => serde_json::to_value(slot.game.legal_actions(s)).unwrap_or(Value::Null),
        None => json!([]),
    };
    json!({
        "version": slot.version,
        "seat": seat,
        "seats": slot.game.num_seats(),
        "observation": slot.game.observation(seat),
        "actions": actions,
    })
}

async fn get_cards<E: Engine>(State(st): State<Arc<AppState<E>>>) -> Response {
    Json(st.slot.lock().await.game.card_catalog()).into_response()
}

#[derive(Deserialize)]
struct Since {
    since: Option<u64>,
}

async fn get_state<E: Engine>(
    State(st): State<Arc<AppState<E>>>,
    Query(q): Query<Since>,
    headers: HeaderMap,
) -> Response {
    let seat = match viewer(&st, &headers) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    if let Some(since) = q.since {
        let mut rx = st.changed.subscribe();
        if *rx.borrow_and_update() == since {
            let _ = tokio::time::timeout(LONG_POLL, rx.changed()).await;
        }
    }
    let slot = st.slot.lock().await;
    Json(snapshot(&slot, seat)).into_response()
}

#[derive(Deserialize)]
struct ActionRequest {
    version: u64,
    action: Value,
}

async fn post_action<E: Engine>(
    State(st): State<Arc<AppState<E>>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let seat = match viewer(&st, &headers) {
        Ok(Some(s)) => s,
        Ok(None) => return error(StatusCode::UNAUTHORIZED, "a seat token is required"),
        Err(e) => return e.into_response(),
    };
    if body.len() > MAX_BODY {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "request too large");
    }
    let req: ActionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("bad request: {e}")),
    };
    let action: E::Action = match serde_json::from_value(req.action) {
        Ok(a) => a,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("unknown action: {e}")),
    };
    let mut slot = st.slot.lock().await;
    if req.version != slot.version {
        return error(
            StatusCode::CONFLICT,
            "the game changed; refresh and try again",
        );
    }
    if let Err(e) = slot.game.apply(seat, action) {
        return error(StatusCode::UNPROCESSABLE_ENTITY, e);
    }
    slot.version += 1;
    st.changed.send_replace(slot.version);
    Json(snapshot(&slot, Some(seat))).into_response()
}

async fn post_new_game<E: Engine>(
    State(st): State<Arc<AppState<E>>>,
    headers: HeaderMap,
) -> Response {
    if !bearer(&headers).is_some_and(|t| st.creds.is_host(t)) {
        return error(StatusCode::UNAUTHORIZED, "the host token is required");
    }
    let game = match (st.new_game)() {
        Ok(g) => g,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let mut slot = st.slot.lock().await;
    if game.num_seats() != slot.game.num_seats() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "seat count changed");
    }
    slot.game = game;
    slot.version += 1;
    st.changed.send_replace(slot.version);
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::allowed_hosts;

    #[test]
    fn hosts_include_the_bound_port() {
        let hosts = allowed_hosts(49152);
        assert!(hosts.contains(&"127.0.0.1:49152".to_string()));
        assert!(hosts.contains(&"localhost:49152".to_string()));
        assert!(!hosts.contains(&"localhost".to_string()));
    }

    #[test]
    fn port_80_also_allows_hosts_without_a_port() {
        let hosts = allowed_hosts(80);
        assert!(hosts.contains(&"localhost".to_string()));
        assert!(hosts.contains(&"127.0.0.1:80".to_string()));
        assert!(!hosts.contains(&"evil.example".to_string()));
    }
}
