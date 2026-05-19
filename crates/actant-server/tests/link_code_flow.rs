//! End-to-end test of the linking-code flow.
//!
//! Boots an in-process router in `remote` mode, mints a fresh code via the
//! library API (mirroring what the binary does at startup), then walks the
//! full UX:
//!   1. GET /link returns the HTML page.
//!   2. POST /v1/auth/link with the wrong code returns 404.
//!   3. POST /v1/auth/link with the right code returns 200 + Set-Cookie +
//!      writes a `workspace_owner` row.
//!   4. Re-redeeming the same code returns 409.

use std::net::SocketAddr;

use actant_server::{
    auth_routes::{mint_link_code_for, DEFAULT_WORKSPACE_ID},
    bootstrap_with_mode, serve,
};

async fn start_remote() -> (String, actant_server::AppState, String) {
    let (_router, state, _maybe) = bootstrap_with_mode(None, false, false, false)
        .await
        .unwrap();
    // Mint an explicit code so we own the plaintext for the assertion.
    let code = mint_link_code_for(&state.storage, DEFAULT_WORKSPACE_ID)
        .await
        .expect("mint")
        .expect("unowned workspace should yield a code");
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
    (format!("http://{bound}"), state, code)
}

// Avoid the unused-import warning when this test binary doesn't reference
// `serve`. (Keeps the `use` line tied to the public surface we care about.)
#[allow(dead_code)]
fn _link_serve(_: ()) -> fn(_: ()) -> () {
    let _ = serve;
    |_| ()
}

#[tokio::test]
async fn link_page_serves_html() {
    let (base, _state, _code) = start_remote().await;
    let c = reqwest::Client::new();
    let r = c.get(format!("{base}/link")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let ct = r
        .headers()
        .get("content-type")
        .map(|h| h.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(ct.starts_with("text/html"));
    let body = r.text().await.unwrap();
    assert!(body.contains("Claim this workspace"));
    assert!(body.contains("/v1/auth/link"));
}

#[tokio::test]
async fn wrong_code_returns_404() {
    let (base, _state, _code) = start_remote().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/auth/link"))
        .json(&serde_json::json!({"code": "abcd-efgh-jkmn"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn malformed_code_returns_404_not_400() {
    // Generic no-info miss — we never echo back whether the shape was the
    // problem vs the value (design doc §6).
    let (base, _state, _code) = start_remote().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/auth/link"))
        .json(&serde_json::json!({"code": "this is not a code"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn correct_code_sets_cookie_and_owner_row() {
    let (base, state, code) = start_remote().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/auth/link"))
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let set_cookie = r
        .headers()
        .get("set-cookie")
        .expect("must set the session cookie")
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.contains("actantdb_session="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    assert!(!set_cookie.contains("Secure"), "no TLS = no Secure flag");
    assert!(set_cookie.contains("Path=/"));

    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["status"], "linked");
    assert_eq!(body["needs_password"], true);
    assert!(body["csrf"].as_str().unwrap().len() > 16);
    assert_eq!(body["workspace_id"], DEFAULT_WORKSPACE_ID);

    // workspace_owner row exists.
    let owner: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT owner_actor_id, password_hash FROM workspace_owner WHERE workspace_id = ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .fetch_optional(state.storage.pool())
    .await
    .unwrap();
    let owner = owner.expect("owner row");
    assert_eq!(owner.0, "act_system");
    assert!(owner.1.is_none(), "password not set yet at this stage");

    // session_token row exists, expires_at in the future.
    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_token")
        .fetch_one(state.storage.pool())
        .await
        .unwrap();
    assert_eq!(n.0, 1);
}

#[tokio::test]
async fn second_redemption_returns_409() {
    let (base, _state, code) = start_remote().await;
    let c = reqwest::Client::new();
    let r1 = c
        .post(format!("{base}/v1/auth/link"))
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), 200);

    let r2 = c
        .post(format!("{base}/v1/auth/link"))
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), 409);
}

#[tokio::test]
async fn link_route_is_rate_limited_per_ip() {
    // 5 requests / 60 s. The 6th wrong code should be 429, not 404.
    let (base, _state, _code) = start_remote().await;
    let c = reqwest::Client::new();
    let bad = serde_json::json!({"code": "abcd-efgh-jkmn"});
    for _ in 0..5 {
        let r = c
            .post(format!("{base}/v1/auth/link"))
            .json(&bad)
            .send()
            .await
            .unwrap();
        // Any of the first 5 may be 404 (not in DB), but never 429.
        assert!(r.status() == 404 || r.status() == 200);
    }
    let r6 = c
        .post(format!("{base}/v1/auth/link"))
        .json(&bad)
        .send()
        .await
        .unwrap();
    assert_eq!(r6.status(), 429);
}

#[tokio::test]
async fn mint_returns_none_when_workspace_already_owned() {
    let (_router, state, _maybe) = bootstrap_with_mode(None, false, false, false)
        .await
        .unwrap();
    let first = mint_link_code_for(&state.storage, DEFAULT_WORKSPACE_ID)
        .await
        .unwrap();
    assert!(first.is_some(), "first call mints a code");

    // Forge a workspace_owner row.
    sqlx::query(
        "INSERT INTO workspace_owner (workspace_id, owner_actor_id, created_at)
         VALUES (?, 'act_system', ?)",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(actant_core::now_rfc3339())
    .execute(state.storage.pool())
    .await
    .unwrap();

    let second = mint_link_code_for(&state.storage, DEFAULT_WORKSPACE_ID)
        .await
        .unwrap();
    assert!(second.is_none(), "owned workspace must not mint a new code");
}
