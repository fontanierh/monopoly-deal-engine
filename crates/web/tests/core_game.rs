//! Drives a real core game through the HTTP API.

use std::net::SocketAddr;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use monopoly_deal_web::{
    core_engine::CoreGame,
    server::{router, AppState},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:8787")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let req = match body {
        Some(b) => req.body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn plays_through_http_with_core() {
    let addr: SocketAddr = "127.0.0.1:8787".parse().unwrap();
    let st = AppState::new(Box::new(|| CoreGame::new(3, 42)), addr).unwrap();
    let app = router(st.clone());
    let seats = st.credentials().seats.clone();

    let mut applied = 0;
    'game: for step in 0..400 {
        for token in &seats {
            let (status, s) = call(&app, "GET", "/api/state", token, None).await;
            assert_eq!(status, StatusCode::OK);
            if s["observation"]["phase"]["type"] == "game_over" {
                break 'game;
            }
            let actions = s["actions"].as_array().unwrap();
            if actions.is_empty() {
                continue;
            }
            let action = actions[step % actions.len()].clone();
            let body = json!({ "version": s["version"], "action": action });
            let (status, after) = call(&app, "POST", "/api/action", token, Some(body)).await;
            assert_eq!(status, StatusCode::OK, "{after}");
            applied += 1;
            break;
        }
    }
    assert!(applied > 20);
}
