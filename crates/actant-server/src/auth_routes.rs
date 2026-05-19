//! UI auth routes — linking code → session cookie → password.
//!
//! See `UI_AUTH_DESIGN.md`. This module owns:
//!   * the `/v1/auth/*` POST routes (link, password, login, logout, me),
//!   * the `GET /link` + `GET /login` HTML pages,
//!   * the cookie-or-bearer principal resolver used by extended routes,
//!   * the per-IP rate limit on `/v1/auth/link`,
//!   * the link-code lifecycle inside the server crate (mint at boot,
//!     redeem in the link route).
//!
//! Notable scope cuts vs the design doc (intentional, per user spec):
//!   * No `/v1/auth/invite` — the only way to create a second owner today
//!     is for the existing owner to issue a fresh code via
//!     [`mint_link_code_for`] (called by the binary on next restart).
//!   * No full Studio SPA mount under `/studio/*`; the Node `@actantdb/studio`
//!     package stays the canonical UI in loopback mode.
//!   * No `reset-password` subcommand — that ships with the binary changes.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use actant_auth::{
    hash_password, hash_token,
    link::{self, hash_code, validate_shape, LinkCode, DEFAULT_TTL_SECS as LINK_TTL_SECS},
    session::{SessionToken, COOKIE_NAME, CSRF_HEADER, DEFAULT_TTL_SECS as SESSION_TTL_SECS},
    verify_csrf, verify_link_code, verify_password, Principal,
};
use actant_core::{now_rfc3339, ActantError, ActorId, WorkspaceId};
use actant_storage::Storage;
use actant_reliability::throttle::{Bucket, Policy};
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use sqlx::Row;
use tokio::sync::Mutex;

use crate::AppState;

/// Bucket policy for `/v1/auth/link`: 5 requests / 60 s.
pub const LINK_RATE_LIMIT: Policy = Policy {
    limit: 5,
    refill_per_second: 5.0 / 60.0,
};

/// Login rate limit per `(workspace_id, ip)` tuple.
pub const LOGIN_RATE_LIMIT: Policy = Policy {
    limit: 10,
    refill_per_second: 10.0 / 60.0,
};

/// After this many consecutive bad-password attempts, pad the response with
/// a uniform delay to mute timing oracles.
pub const LOGIN_TIMING_PAD_AFTER: u32 = 5;

/// Uniform delay applied once an actor has crossed [`LOGIN_TIMING_PAD_AFTER`].
pub const LOGIN_TIMING_PAD: Duration = Duration::from_millis(500);

/// Default workspace served by an out-of-the-box `actantdb-server`. The
/// linking flow is scoped to this single workspace; multi-workspace boot
/// (Phase 6.5) will swap in a per-workspace claim flow.
pub const DEFAULT_WORKSPACE_ID: &str = "ws_default";

/// Per-IP buckets for the link route + per-(workspace, ip) buckets for login.
#[derive(Debug, Default)]
pub struct AuthRateLimiters {
    /// Keyed on client IP string.
    pub link: Mutex<HashMap<String, Bucket>>,
    /// Keyed on `"{workspace_id}|{ip}"`.
    pub login: Mutex<HashMap<String, Bucket>>,
    /// Consecutive failed login attempts per `(workspace_id, ip)`.
    pub login_failures: Mutex<HashMap<String, u32>>,
}

impl AuthRateLimiters {
    /// Fresh, empty.
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// HTML page handlers.
// ---------------------------------------------------------------------------

/// Serves the linking-code UI. Same content regardless of `:code` parameter
/// — the page's JS reads `location.pathname`.
pub async fn link_page() -> Response {
    html_response(include_str!("../assets/link.html"))
}

/// Serves the password login UI.
pub async fn login_page() -> Response {
    html_response(include_str!("../assets/login.html"))
}

fn html_response(body: &'static str) -> Response {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .header(
            "content-security-policy",
            "default-src 'self'; script-src 'self' 'unsafe-inline'; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
             connect-src 'self'; frame-ancestors 'none'",
        )
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .body(axum::body::Body::from(body))
        .unwrap()
}

// ---------------------------------------------------------------------------
// POST /v1/auth/link
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/auth/link`.
#[derive(Debug, Deserialize)]
pub struct LinkRequest {
    /// The one-time linking code. Accepted in dashed (`xxxx-xxxx-xxxx`) or
    /// smashed form, case-insensitive.
    pub code: String,
    /// Optional — when present, must match the workspace bound to the code.
    /// The wire shape in the user spec includes this field; the server treats
    /// it as a cross-check, not a lookup key.
    #[serde(default)]
    pub workspace_id: Option<String>,
}

/// `POST /v1/auth/link` — redeem a linking code, claim ownership, set cookie.
pub async fn link_redeem(
    State(s): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    Json(req): Json<LinkRequest>,
) -> Response {
    // Per-IP rate-limit, even on bad inputs.
    let ip = connect_info.as_ref().map(|c| c.0.ip().to_string());
    if let Err(resp) = enforce_link_rate(&s, ip.as_deref()).await {
        return resp;
    }
    if let Err(e) = validate_shape(&req.code) {
        // 404-shape: never reveal whether the shape was the problem vs the
        // value, so the response body is the same as a miss below.
        return generic_link_miss(e);
    }
    let code_hash = hash_code(&req.code);
    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT workspace_id, expires_at, claimed_at
         FROM link_code WHERE code_hash = ?",
    )
    .bind(&code_hash)
    .fetch_optional(s.storage.pool())
    .await;
    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return generic_link_miss(ActantError::NotFound("link_code".into())),
        Err(e) => return crate::err_response(ActantError::Storage(e.to_string())),
    };
    let (workspace_id, expires_at, claimed_at) = row;
    if claimed_at.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "already_claimed",
                "message": "this linking code has already been used"
            })),
        )
            .into_response();
    }
    if is_past(&expires_at) {
        return (
            StatusCode::GONE,
            Json(serde_json::json!({
                "error": "expired",
                "message": "this linking code has expired"
            })),
        )
            .into_response();
    }
    if let Some(provided) = req.workspace_id.as_deref() {
        if !provided.is_empty() && provided != workspace_id {
            return generic_link_miss(ActantError::PermissionDenied(
                "workspace_id does not match".into(),
            ));
        }
    }

    // The actor who claimed the workspace. For the single-workspace MVP we
    // reuse the seeded `act_system` row when a Human row doesn't already
    // exist — the design doc allows the link path to insert a new Human
    // actor, but that's outside the user-locked scope.
    let owner_actor_id = "act_system".to_string();

    // Constant-time recheck before mutating. Defense-in-depth — the SELECT
    // above already used `code_hash` as the lookup key, but if a future
    // refactor introduces a SELECT WHERE substring we want this here.
    if !verify_link_code(&code_hash, &req.code) {
        return generic_link_miss(ActantError::PermissionDenied("mismatch".into()));
    }

    // Transaction: insert workspace_owner (INSERT OR IGNORE so a racing
    // claim doesn't succeed twice), mark claimed_at, mint session.
    let mut tx = match s.storage.pool().begin().await {
        Ok(t) => t,
        Err(e) => return crate::err_response(ActantError::Storage(e.to_string())),
    };
    let now = now_rfc3339();
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO workspace_owner
            (workspace_id, owner_actor_id, password_hash, password_set_at, created_at)
         VALUES (?, ?, NULL, NULL, ?)",
    )
    .bind(&workspace_id)
    .bind(&owner_actor_id)
    .bind(&now)
    .execute(&mut *tx)
    .await;
    let inserted = match inserted {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.rollback().await;
            return crate::err_response(ActantError::Storage(e.to_string()));
        }
    };
    if inserted.rows_affected() == 0 {
        // Another claim won the race.
        let _ = tx.rollback().await;
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "already_owned",
                "message": "workspace ownership has already been claimed"
            })),
        )
            .into_response();
    }
    let upd = sqlx::query(
        "UPDATE link_code SET claimed_at = ?, claimed_by_actor_id = ?
         WHERE code_hash = ? AND claimed_at IS NULL",
    )
    .bind(&now)
    .bind(&owner_actor_id)
    .bind(&code_hash)
    .execute(&mut *tx)
    .await;
    match upd {
        Ok(r) if r.rows_affected() == 1 => {}
        Ok(_) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "already_claimed",
                    "message": "this linking code has already been used"
                })),
            )
                .into_response();
        }
        Err(e) => {
            let _ = tx.rollback().await;
            return crate::err_response(ActantError::Storage(e.to_string()));
        }
    }
    let session = SessionToken::generate();
    let exp = unix_now() + SESSION_TTL_SECS;
    let exp_iso = unix_to_iso(exp);
    let res = sqlx::query(
        "INSERT INTO session_token
            (token_hash, owner_actor_id, workspace_id, csrf_secret, created_at,
             expires_at, revoked_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(&session.token_hash)
    .bind(&owner_actor_id)
    .bind(&workspace_id)
    .bind(&session.csrf_secret)
    .bind(&now)
    .bind(&exp_iso)
    .execute(&mut *tx)
    .await;
    if let Err(e) = res {
        let _ = tx.rollback().await;
        return crate::err_response(ActantError::Storage(e.to_string()));
    }
    if let Err(e) = tx.commit().await {
        return crate::err_response(ActantError::Storage(e.to_string()));
    }

    let secure = is_request_secure(&s);
    let cookie = build_session_cookie(&session.plaintext, SESSION_TTL_SECS, secure);
    let body = Json(serde_json::json!({
        "status": "linked",
        "needs_password": true,
        "csrf": session.csrf_secret,
        "workspace_id": workspace_id,
        "actor_id": owner_actor_id,
    }));
    (StatusCode::OK, [("set-cookie", cookie.as_str())], body).into_response()
}

fn generic_link_miss(_inner: ActantError) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "invalid_code",
            "message": "the linking code is not valid"
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// POST /v1/auth/password
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/auth/password`.
#[derive(Debug, Deserialize)]
pub struct PasswordRequest {
    /// New plaintext password — minimum 8 chars (see [`actant_auth::password`]).
    pub password: String,
}

/// `POST /v1/auth/password` — set or rotate the owner password. Requires a
/// valid cookie session AND a matching X-CSRF-Token.
pub async fn set_password(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<PasswordRequest>,
) -> Response {
    let principal = match resolve_principal_for_mutation(&s, &headers, &Method::POST).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let phc = match hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => return crate::err_response(e),
    };
    let res = sqlx::query(
        "UPDATE workspace_owner
            SET password_hash = ?, password_set_at = ?
          WHERE workspace_id = ? AND owner_actor_id = ?",
    )
    .bind(&phc)
    .bind(now_rfc3339())
    .bind(principal.workspace_id.as_str())
    .bind(principal.actor_id.as_str())
    .execute(s.storage.pool())
    .await;
    match res {
        Ok(r) if r.rows_affected() == 1 => {
            Json(serde_json::json!({ "status": "set" })).into_response()
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "no_owner_row",
                "message": "owner row missing — link this workspace first"
            })),
        )
            .into_response(),
        Err(e) => crate::err_response(ActantError::Storage(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// POST /v1/auth/login
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/auth/login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Plaintext password to verify against the stored argon2id hash.
    pub password: String,
    /// Workspace to log in to. Optional; defaults to `ws_default`.
    #[serde(default)]
    pub workspace_id: Option<String>,
}

/// `POST /v1/auth/login` — password login, mints a fresh session cookie.
pub async fn login(
    State(s): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let workspace_id = req
        .workspace_id
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_WORKSPACE_ID)
        .to_string();
    let ip = connect_info.as_ref().map(|c| c.0.ip().to_string());
    let key = login_key(&workspace_id, ip.as_deref());

    // Rate-limit per (workspace, ip).
    {
        let mut g = s.auth_limiters.login.lock().await;
        let bucket = g
            .entry(key.clone())
            .or_insert_with(|| Bucket::new(LOGIN_RATE_LIMIT));
        if let Err(retry_after) = bucket.try_consume(1) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after.as_secs().max(1).to_string())],
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "retry_after_seconds": retry_after.as_secs()
                })),
            )
                .into_response();
        }
    }

    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT owner_actor_id, password_hash FROM workspace_owner WHERE workspace_id = ?",
    )
    .bind(&workspace_id)
    .fetch_optional(s.storage.pool())
    .await;
    let (owner_actor_id, ok) = match row {
        Ok(Some((actor, Some(phc)))) => {
            let ok = verify_password(&req.password, &phc).unwrap_or(false);
            (actor, ok)
        }
        Ok(Some((actor, None))) => (actor, false),
        Ok(None) => ("act_system".to_string(), false),
        Err(e) => return crate::err_response(ActantError::Storage(e.to_string())),
    };

    if !ok {
        // Track + pad after a streak.
        let attempts = {
            let mut g = s.auth_limiters.login_failures.lock().await;
            let entry = g.entry(key).or_insert(0);
            *entry = entry.saturating_add(1);
            *entry
        };
        if attempts >= LOGIN_TIMING_PAD_AFTER {
            tokio::time::sleep(LOGIN_TIMING_PAD).await;
        }
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "invalid_credentials",
                "message": "wrong password"
            })),
        )
            .into_response();
    }

    // Clear the failure counter on success.
    {
        let mut g = s.auth_limiters.login_failures.lock().await;
        g.remove(&key);
    }

    let session_tok = SessionToken::generate();
    let exp = unix_now() + SESSION_TTL_SECS;
    let exp_iso = unix_to_iso(exp);
    let now = now_rfc3339();
    let res = sqlx::query(
        "INSERT INTO session_token
            (token_hash, owner_actor_id, workspace_id, csrf_secret, created_at, expires_at, revoked_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(&session_tok.token_hash)
    .bind(&owner_actor_id)
    .bind(&workspace_id)
    .bind(&session_tok.csrf_secret)
    .bind(&now)
    .bind(&exp_iso)
    .execute(s.storage.pool())
    .await;
    if let Err(e) = res {
        return crate::err_response(ActantError::Storage(e.to_string()));
    }
    let secure = is_request_secure(&s);
    let cookie = build_session_cookie(&session_tok.plaintext, SESSION_TTL_SECS, secure);
    let body = Json(serde_json::json!({
        "status": "ok",
        "csrf": session_tok.csrf_secret,
        "workspace_id": workspace_id,
        "actor_id": owner_actor_id,
    }));
    (StatusCode::OK, [("set-cookie", cookie.as_str())], body).into_response()
}

fn login_key(workspace_id: &str, ip: Option<&str>) -> String {
    format!("{workspace_id}|{}", ip.unwrap_or("-"))
}

// ---------------------------------------------------------------------------
// POST /v1/auth/logout
// ---------------------------------------------------------------------------

/// `POST /v1/auth/logout` — revoke the current session.
pub async fn logout(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(plain) = extract_session_cookie(&headers) {
        let h = hash_token(&plain);
        let _ = sqlx::query(
            "UPDATE session_token SET revoked_at = ? WHERE token_hash = ? AND revoked_at IS NULL",
        )
        .bind(now_rfc3339())
        .bind(&h)
        .execute(s.storage.pool())
        .await;
    }
    let secure = is_request_secure(&s);
    let clear = clear_session_cookie(secure);
    (
        StatusCode::OK,
        [("set-cookie", clear.as_str())],
        Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// GET /v1/auth/me
// ---------------------------------------------------------------------------

/// `GET /v1/auth/me` — return the current principal (cookie or bearer).
pub async fn whoami(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    match resolve_principal(&s, &headers, &Method::GET).await {
        Ok(p) => Json(serde_json::json!({
            "workspace_id": p.workspace_id.as_str(),
            "actor_id": p.actor_id.as_str(),
            "roles": p.roles,
            "auth": "cookie_or_bearer",
        }))
        .into_response(),
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthenticated" })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Cookie + CSRF helpers.
// ---------------------------------------------------------------------------

fn build_session_cookie(token: &str, ttl_secs: i64, secure: bool) -> String {
    let mut c =
        format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl_secs}");
    if secure {
        c.push_str("; Secure");
    }
    c
}

fn clear_session_cookie(secure: bool) -> String {
    let mut c = format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        c.push_str("; Secure");
    }
    c
}

/// Detect HTTPS via whether the server is configured with TLS. We don't
/// honor `X-Forwarded-Proto` unless `trust_proxy` is on — that's what the
/// design doc calls out as a footgun.
fn is_request_secure(state: &AppState) -> bool {
    state.tls_enabled
}

/// Pull the opaque session value out of a `Cookie: actantdb_session=...; ...`
/// header.
pub fn extract_session_cookie(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("cookie")?.to_str().ok()?;
    for piece in raw.split(';') {
        let piece = piece.trim();
        if let Some(rest) = piece.strip_prefix(&format!("{COOKIE_NAME}=")) {
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Principal resolver — cookie OR bearer.
// ---------------------------------------------------------------------------

/// Resolve a Principal for any request. Mutating requests must also pass
/// CSRF; use [`resolve_principal_for_mutation`].
pub async fn resolve_principal(
    state: &AppState,
    headers: &HeaderMap,
    _method: &Method,
) -> Result<Principal, Response> {
    // 1. Bearer JWT path — unchanged from the legacy `enforce_auth`.
    if let Some(bearer) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        if let Some(secret) = &state.auth_secret {
            match actant_auth::verify(bearer, secret) {
                Ok(claims) => return Ok(actant_auth::principal_from_claims(&claims)),
                Err(_) => {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({"error": "invalid_token"})),
                    )
                        .into_response())
                }
            }
        }
    }
    // 2. Cookie path.
    let Some(token) = extract_session_cookie(headers) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthenticated"})),
        )
            .into_response());
    };
    let hash = hash_token(&token);
    let row = sqlx::query(
        "SELECT owner_actor_id, workspace_id, csrf_secret, expires_at, revoked_at
         FROM session_token WHERE token_hash = ?",
    )
    .bind(&hash)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|e| crate::err_response(ActantError::Storage(e.to_string())))?;
    let Some(row) = row else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthenticated"})),
        )
            .into_response());
    };
    let owner_actor_id: String = row.get(0);
    let workspace_id: String = row.get(1);
    let _csrf_secret: String = row.get(2);
    let expires_at: String = row.get(3);
    let revoked_at: Option<String> = row.get(4);
    if revoked_at.is_some() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "revoked"})),
        )
            .into_response());
    }
    if is_past(&expires_at) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "expired"})),
        )
            .into_response());
    }
    Ok(Principal {
        workspace_id: WorkspaceId::from_string(workspace_id),
        actor_id: ActorId::from_string(owner_actor_id),
        roles: vec!["owner".into()],
        expires_at: parse_iso_to_unix(&expires_at).unwrap_or(0),
    })
}

/// Like [`resolve_principal`], but also requires `X-CSRF-Token` to match
/// the session's `csrf_secret` when the request came via cookie. Bearer
/// requests skip CSRF (no ambient credential = no CSRF surface).
pub async fn resolve_principal_for_mutation(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
) -> Result<Principal, Response> {
    // GET/HEAD/OPTIONS shouldn't go through the mutation path; treat as a
    // bug if it does.
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return resolve_principal(state, headers, method).await;
    }
    // If a bearer is present, defer to the regular resolver. CSRF only
    // applies to cookie-authenticated requests.
    if headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some()
    {
        return resolve_principal(state, headers, method).await;
    }
    let Some(token) = extract_session_cookie(headers) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthenticated"})),
        )
            .into_response());
    };
    let presented = headers
        .get(CSRF_HEADER)
        .or_else(|| headers.get("x-csrf-token"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let Some(presented) = presented else {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "csrf_required",
                "message": "X-CSRF-Token header is required for mutating routes"
            })),
        )
            .into_response());
    };

    let hash = hash_token(&token);
    let row = sqlx::query(
        "SELECT owner_actor_id, workspace_id, csrf_secret, expires_at, revoked_at
         FROM session_token WHERE token_hash = ?",
    )
    .bind(&hash)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|e| crate::err_response(ActantError::Storage(e.to_string())))?;
    let Some(row) = row else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthenticated"})),
        )
            .into_response());
    };
    let owner_actor_id: String = row.get(0);
    let workspace_id: String = row.get(1);
    let csrf_secret: String = row.get(2);
    let expires_at: String = row.get(3);
    let revoked_at: Option<String> = row.get(4);
    if revoked_at.is_some() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "revoked"})),
        )
            .into_response());
    }
    if is_past(&expires_at) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "expired"})),
        )
            .into_response());
    }
    if !verify_csrf(&csrf_secret, &presented) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "csrf_mismatch",
                "message": "X-CSRF-Token does not match session"
            })),
        )
            .into_response());
    }
    Ok(Principal {
        workspace_id: WorkspaceId::from_string(workspace_id),
        actor_id: ActorId::from_string(owner_actor_id),
        roles: vec!["owner".into()],
        expires_at: parse_iso_to_unix(&expires_at).unwrap_or(0),
    })
}

async fn enforce_link_rate(state: &AppState, ip: Option<&str>) -> Result<(), Response> {
    let key = ip.unwrap_or("-").to_string();
    let mut g = state.auth_limiters.link.lock().await;
    let bucket = g.entry(key).or_insert_with(|| Bucket::new(LINK_RATE_LIMIT));
    if let Err(retry_after) = bucket.try_consume(1) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", retry_after.as_secs().max(1).to_string())],
            Json(serde_json::json!({
                "error": "rate_limited",
                "retry_after_seconds": retry_after.as_secs()
            })),
        )
            .into_response());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Boot-time link code minting.
// ---------------------------------------------------------------------------

/// Bootstrap helper: if there's no owner row for `workspace_id`, invalidate
/// any prior unconsumed code and mint a fresh one. Returns the displayed
/// code so the binary can print it to stderr.
pub async fn mint_link_code_for(
    storage: &Storage,
    workspace_id: &str,
) -> Result<Option<String>, ActantError> {
    let claimed: Option<(String,)> =
        sqlx::query_as("SELECT workspace_id FROM workspace_owner WHERE workspace_id = ?")
            .bind(workspace_id)
            .fetch_optional(storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
    if claimed.is_some() {
        return Ok(None);
    }
    sqlx::query("DELETE FROM link_code WHERE workspace_id = ? AND claimed_at IS NULL")
        .bind(workspace_id)
        .execute(storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    let code = LinkCode::generate();
    let exp_iso = unix_to_iso(unix_now() + LINK_TTL_SECS);
    sqlx::query(
        "INSERT INTO link_code
            (code_hash, workspace_id, expires_at, claimed_at, claimed_by_actor_id, created_at)
         VALUES (?, ?, ?, NULL, NULL, ?)",
    )
    .bind(&code.hash)
    .bind(workspace_id)
    .bind(&exp_iso)
    .bind(now_rfc3339())
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(Some(code.display))
}

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

fn unix_now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn unix_to_iso(unix: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(unix)
        .ok()
        .and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(now_rfc3339)
}

fn parse_iso_to_unix(s: &str) -> Option<i64> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|t| t.unix_timestamp())
}

fn is_past(iso: &str) -> bool {
    match parse_iso_to_unix(iso) {
        Some(t) => unix_now() > t,
        None => true,
    }
}

/// Check whether a bound address counts as loopback. Used by the binary to
/// flip `local_mode` and to refuse non-loopback binds without TLS.
pub fn is_bind_loopback(bind: &str) -> bool {
    let host = bind.rsplit_once(':').map(|(h, _)| h).unwrap_or(bind);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

// Silence unused-import lints when only sub-paths are referenced.
#[allow(dead_code)]
fn _keep_link_imports() {
    let _ = link::CODE_LEN;
    let _ = LINK_TTL_SECS;
    let _ = SESSION_TTL_SECS;
}

// Convenience for tests.
#[doc(hidden)]
pub fn build_session_cookie_for_test(token: &str, ttl_secs: i64, secure: bool) -> String {
    build_session_cookie(token, ttl_secs, secure)
}

#[doc(hidden)]
pub fn header_value_session(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("{COOKIE_NAME}={token}")).unwrap()
}
