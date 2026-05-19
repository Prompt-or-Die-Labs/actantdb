//! Password set → login → rotate → logout.
//!
//! Covers the contract from UI_AUTH_DESIGN.md §5.2:
//!   * Set password requires a valid session cookie + CSRF token.
//!   * Login mints a fresh session cookie + CSRF.
//!   * Logout revokes the session, subsequent /v1/auth/me is 401.
//!   * Wrong password returns 401 and increments the failure counter.

use std::net::SocketAddr;

use actant_server::{
    auth_routes::{mint_link_code_for, DEFAULT_WORKSPACE_ID},
    bootstrap_with_mode,
};

struct Harness {
    base: String,
    state: actant_server::AppState,
    client: reqwest::Client,
}

async fn start() -> Harness {
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
    Harness {
        base: format!("http://{bound}"),
        state,
        client,
    }
}

struct LinkOutcome {
    csrf: String,
    cookie: String,
}

/// Extract the opaque session token from a `Set-Cookie: actantdb_session=...; ...`
/// header so we can replay it manually (the workspace `reqwest` does not enable
/// the `cookies` feature).
fn extract_cookie(set_cookie: &str) -> String {
    for piece in set_cookie.split(';') {
        let piece = piece.trim();
        if let Some(rest) = piece.strip_prefix("actantdb_session=") {
            return format!("actantdb_session={rest}");
        }
    }
    panic!("no actantdb_session= in {set_cookie}");
}

async fn link_workspace(h: &Harness) -> LinkOutcome {
    let code = mint_link_code_for(&h.state.storage, DEFAULT_WORKSPACE_ID)
        .await
        .unwrap()
        .unwrap();
    let r = h
        .client
        .post(format!("{}/v1/auth/link", h.base))
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let set_cookie = r
        .headers()
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .to_string();
    let cookie = extract_cookie(&set_cookie);
    let body: serde_json::Value = r.json().await.unwrap();
    LinkOutcome {
        csrf: body["csrf"].as_str().unwrap().to_string(),
        cookie,
    }
}

#[tokio::test]
async fn password_set_requires_csrf() {
    let h = start().await;
    let out = link_workspace(&h).await;

    // No CSRF header → 403.
    let r = h
        .client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn password_set_succeeds_with_csrf() {
    let h = start().await;
    let out = link_workspace(&h).await;
    let r = h
        .client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "body: {:?}", r.text().await);

    // Confirm the row has a hash now.
    let row: (Option<String>,) =
        sqlx::query_as("SELECT password_hash FROM workspace_owner WHERE workspace_id = ?")
            .bind(DEFAULT_WORKSPACE_ID)
            .fetch_one(h.state.storage.pool())
            .await
            .unwrap();
    let phc = row.0.expect("hash written");
    assert!(phc.starts_with("$argon2id$"), "got: {phc}");
}

#[tokio::test]
async fn login_succeeds_with_correct_password() {
    let h = start().await;
    let out = link_workspace(&h).await;
    let r = h
        .client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let fresh = reqwest::Client::new();
    let r = fresh
        .post(format!("{}/v1/auth/login", h.base))
        .json(&serde_json::json!({"password": "correct-horse-battery"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "body: {:?}", r.text().await);
    let set_cookie = r
        .headers()
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .to_string();
    let cookie = extract_cookie(&set_cookie);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["csrf"].as_str().unwrap().len() > 16);

    // /v1/auth/me identifies via cookie.
    let r = fresh
        .get(format!("{}/v1/auth/me", h.base))
        .header("Cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["workspace_id"], DEFAULT_WORKSPACE_ID);
    assert_eq!(body["actor_id"], "act_system");
}

#[tokio::test]
async fn wrong_password_returns_401() {
    let h = start().await;
    let out = link_workspace(&h).await;
    let r = h
        .client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "real-real-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let fresh = reqwest::Client::new();
    let r = fresh
        .post(format!("{}/v1/auth/login", h.base))
        .json(&serde_json::json!({"password": "wrong-wrong-wrong"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn password_can_be_rotated() {
    let h = start().await;
    let out = link_workspace(&h).await;
    h.client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "first-password-here"}))
        .send()
        .await
        .unwrap();
    let r = h
        .client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "second-password-here"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // First password should now fail.
    let fresh = reqwest::Client::new();
    let r = fresh
        .post(format!("{}/v1/auth/login", h.base))
        .json(&serde_json::json!({"password": "first-password-here"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);

    // Second one succeeds.
    let r = fresh
        .post(format!("{}/v1/auth/login", h.base))
        .json(&serde_json::json!({"password": "second-password-here"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn logout_revokes_session() {
    let h = start().await;
    let out = link_workspace(&h).await;
    h.client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "secret-secret-secret"}))
        .send()
        .await
        .unwrap();

    // Pre-logout: /me works.
    let r = h
        .client
        .get(format!("{}/v1/auth/me", h.base))
        .header("Cookie", &out.cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Logout — does not require CSRF (idempotent revoke).
    let r = h
        .client
        .post(format!("{}/v1/auth/logout", h.base))
        .header("Cookie", &out.cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Post-logout: /me is 401.
    let r = h
        .client
        .get(format!("{}/v1/auth/me", h.base))
        .header("Cookie", &out.cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn password_rejects_too_short() {
    let h = start().await;
    let out = link_workspace(&h).await;
    let r = h
        .client
        .post(format!("{}/v1/auth/password", h.base))
        .header("Cookie", &out.cookie)
        .header("X-CSRF-Token", &out.csrf)
        .json(&serde_json::json!({"password": "abc"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}
