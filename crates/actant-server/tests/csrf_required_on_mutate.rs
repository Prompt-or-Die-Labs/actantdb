//! CSRF defense on cookie-authenticated mutating routes.
//!
//! Bearer-JWT requests are CSRF-exempt by construction (no ambient cookie).
//! Cookie requests on `POST /v1/auth/password` MUST carry a matching
//! `X-CSRF-Token` header or the request is rejected with 403.

use std::net::SocketAddr;

use actant_server::{
    auth_routes::{mint_link_code_for, DEFAULT_WORKSPACE_ID},
    bootstrap_with_mode,
};

struct Bag {
    base: String,
    state: actant_server::AppState,
    client: reqwest::Client,
    cookie: String,
    csrf: String,
}

fn extract_cookie(set_cookie: &str) -> String {
    for piece in set_cookie.split(';') {
        let piece = piece.trim();
        if let Some(rest) = piece.strip_prefix("actantdb_session=") {
            return format!("actantdb_session={rest}");
        }
    }
    panic!("no actantdb_session= in {set_cookie}");
}

async fn start() -> Bag {
    let (_router, state, _) = bootstrap_with_mode(None, false, false, false)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    let router = actant_server::router(state.clone());
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let client = reqwest::Client::new();
    let base = format!("http://{bound}");
    let code = mint_link_code_for(&state.storage, DEFAULT_WORKSPACE_ID)
        .await
        .unwrap()
        .unwrap();
    let r = client
        .post(format!("{base}/v1/auth/link"))
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let cookie = extract_cookie(r.headers().get("set-cookie").unwrap().to_str().unwrap());
    let body: serde_json::Value = r.json().await.unwrap();
    let csrf = body["csrf"].as_str().unwrap().to_string();
    Bag {
        base,
        state,
        client,
        cookie,
        csrf,
    }
}

#[tokio::test]
async fn post_without_csrf_returns_403() {
    let b = start().await;
    let _ = &b.state;
    let r = b
        .client
        .post(format!("{}/v1/auth/password", b.base))
        .header("Cookie", &b.cookie)
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "csrf_required");
}

#[tokio::test]
async fn post_with_wrong_csrf_returns_403() {
    let b = start().await;
    let _ = &b.state;
    let r = b
        .client
        .post(format!("{}/v1/auth/password", b.base))
        .header("Cookie", &b.cookie)
        .header("X-CSRF-Token", "not-the-real-secret")
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "csrf_mismatch");
}

#[tokio::test]
async fn post_with_right_csrf_returns_200() {
    let b = start().await;
    let _ = &b.state;
    let r = b
        .client
        .post(format!("{}/v1/auth/password", b.base))
        .header("Cookie", &b.cookie)
        .header("X-CSRF-Token", &b.csrf)
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn get_does_not_require_csrf() {
    let b = start().await;
    let _ = &b.state;
    let r = b
        .client
        .get(format!("{}/v1/auth/me", b.base))
        .header("Cookie", &b.cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

/// Cookie + CSRF flows through `enforce_auth` on the legacy data-plane
/// routes, not just the new `/v1/auth/*` ones. Without this gate the
/// browser would never be able to call `POST /v1/command` from the UI.
#[tokio::test]
async fn legacy_command_route_accepts_cookie_plus_csrf() {
    let b = start().await;
    let r = b
        .client
        .post(format!("{}/v1/command", b.base))
        .header("Cookie", &b.cookie)
        .header("X-CSRF-Token", &b.csrf)
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "got {}: {:?}",
        r.status(),
        r.text().await
    );
}

#[tokio::test]
async fn legacy_command_route_rejects_cookie_without_csrf() {
    let b = start().await;
    let r = b
        .client
        .post(format!("{}/v1/command", b.base))
        .header("Cookie", &b.cookie)
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "csrf_required");
}
