use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use monopoly_deal_web::{
    engine::Engine,
    server::{router, AppState},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower::ServiceExt;

/// Each seat holds secret cards; the current seat plays one per turn.
struct Mock {
    hands: Vec<Vec<u16>>,
    turn: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Act {
    Play { card: u16 },
}

impl Engine for Mock {
    type Action = Act;
    fn num_seats(&self) -> usize {
        self.hands.len()
    }
    fn observation(&self, viewer: Option<usize>) -> Value {
        json!({
            "turn": self.turn,
            "hand": viewer.map(|v| &self.hands[v]),
            "hand_counts": self.hands.iter().map(Vec::len).collect::<Vec<_>>(),
        })
    }
    fn card_catalog(&self) -> Value {
        json!({})
    }
    fn legal_actions(&self, seat: usize) -> Vec<Act> {
        if seat != self.turn {
            return vec![];
        }
        self.hands[seat]
            .iter()
            .map(|&card| Act::Play { card })
            .collect()
    }
    fn apply(&mut self, seat: usize, Act::Play { card }: Act) -> Result<(), String> {
        if seat != self.turn {
            return Err("not your turn".into());
        }
        let i = self.hands[seat]
            .iter()
            .position(|&c| c == card)
            .ok_or("card not in hand")?;
        self.hands[seat].remove(i);
        self.turn = (self.turn + 1) % self.hands.len();
        Ok(())
    }
}

const SECRET_A: u16 = 4101;
const SECRET_B: u16 = 4202;

fn setup() -> (Router, Arc<AppState<Mock>>) {
    let addr: SocketAddr = "127.0.0.1:8787".parse().unwrap();
    let st = AppState::new(
        Box::new(|| {
            Ok(Mock {
                hands: vec![vec![SECRET_A, 1], vec![SECRET_B, 2], vec![3]],
                turn: 0,
            })
        }),
        addr,
    )
    .unwrap();
    (router(st.clone()), st)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, String) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:8787");
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::Null),
        text,
    )
}

#[tokio::test]
async fn spectator_sees_no_hands() {
    let (app, _) = setup();
    let (status, body, text) = call(&app, "GET", "/api/state", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["seat"], Value::Null);
    assert_eq!(body["actions"], json!([]));
    assert!(!text.contains(&SECRET_A.to_string()) && !text.contains(&SECRET_B.to_string()));
}

#[tokio::test]
async fn seat_sees_only_own_hand() {
    let (app, st) = setup();
    let token = st.credentials().seats[1].clone();
    let (_, body, text) = call(&app, "GET", "/api/state", Some(&token), None).await;
    assert_eq!(body["seat"], 1);
    assert!(text.contains(&SECRET_B.to_string()));
    assert!(!text.contains(&SECRET_A.to_string()));
}

#[tokio::test]
async fn bad_token_is_rejected() {
    let (app, _) = setup();
    let (status, _, _) = call(
        &app,
        "GET",
        "/api/state",
        Some("0123456789abcdef0123456789abcdef"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn actions_need_the_acting_seat_token() {
    let (app, st) = setup();
    let play = json!({ "version": 0, "action": { "type": "play", "card": 1 } });
    let (status, _, _) = call(&app, "POST", "/api/action", None, Some(play.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Seat 1 cannot play seat 0's card; the seat comes from the token, not the body.
    let seat1 = st.credentials().seats[1].clone();
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/action",
        Some(&seat1),
        Some(play.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let seat0 = st.credentials().seats[0].clone();
    let (status, body, _) = call(
        &app,
        "POST",
        "/api/action",
        Some(&seat0),
        Some(play.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], 1);
    // Replaying the same request is stale.
    let (status, _, _) = call(&app, "POST", "/api/action", Some(&seat0), Some(play)).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn malformed_action_is_rejected() {
    let (app, st) = setup();
    let seat0 = st.credentials().seats[0].clone();
    let bad = json!({ "version": 0, "action": { "type": "steal_everything" } });
    let (status, _, _) = call(&app, "POST", "/api/action", Some(&seat0), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn foreign_host_and_origin_are_rejected() {
    let (app, st) = setup();
    let req = Request::builder()
        .uri("/api/state")
        .header(header::HOST, "evil.example:8787")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    let seat0 = st.credentials().seats[0].clone();
    let req = Request::builder()
        .method("POST")
        .uri("/api/action")
        .header(header::HOST, "127.0.0.1:8787")
        .header(header::ORIGIN, "http://evil.example")
        .header(header::AUTHORIZATION, format!("Bearer {seat0}"))
        .body(Body::from(
            json!({ "version": 0, "action": { "type": "play", "card": 1 } }).to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn only_host_can_restart() {
    let (app, st) = setup();
    let seat0 = st.credentials().seats[0].clone();
    let (status, _, _) = call(&app, "POST", "/api/new-game", Some(&seat0), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let host = st.credentials().host.clone();
    let (status, _, _) = call(&app, "POST", "/api/new-game", Some(&host), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body, _) = call(&app, "GET", "/api/state", None, None).await;
    assert_eq!(body["version"], 1);
}

#[tokio::test]
async fn long_poll_wakes_on_change() {
    let (app, st) = setup();
    let waiter = {
        let app = app.clone();
        tokio::spawn(async move { call(&app, "GET", "/api/state?since=0", None, None).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!waiter.is_finished());
    let seat0 = st.credentials().seats[0].clone();
    let play = json!({ "version": 0, "action": { "type": "play", "card": 1 } });
    call(&app, "POST", "/api/action", Some(&seat0), Some(play)).await;
    let (_, body, _) = waiter.await.unwrap();
    assert_eq!(body["version"], 1);
}

#[tokio::test]
async fn static_assets_have_security_headers() {
    let (app, _) = setup();
    let req = Request::builder()
        .uri("/")
        .header(header::HOST, "localhost:8787")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    for path in ["/app.js", "/phone.js", "/style.css"] {
        let req = Request::builder()
            .uri(path)
            .header(header::HOST, "localhost:8787")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK,
            "{path}"
        );
    }
    assert!(res.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .contains("default-src 'self'"));
}
