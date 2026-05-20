//! The server returns typed errors via `{"error":"<kind>","message":"..."}`
//! regardless of HTTP status code. The SDK must surface each kind as a
//! distinct `ActantError` variant. These tests pin the wire shapes the
//! server actually emits so a future server-side rewrite that changes them
//! breaks loudly here.

use actantdb_client::{ActantClient, ActantError};
use serde_json::json;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn fresh_client() -> (ActantClient, MockServer) {
    let server = MockServer::start().await;
    let url = Url::parse(&server.uri()).expect("mock server uri parses");
    let client = ActantClient::new(url)
        .with_workspace_id("ws_demo")
        .with_actor_id("act_user");
    (client, server)
}

#[tokio::test]
async fn approval_required_is_202_typed_error() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "error": "approval_required",
            "message": "tool_call_id=tc_1 awaiting approval"
        })))
        .mount(&server)
        .await;
    let err = client
        .request_tool_call(
            None,
            None,
            "sess_1",
            "shell.run",
            json!({"cmd": "rm -rf /"}),
        )
        .await
        .expect_err("approval_required must surface as an error");
    match err {
        ActantError::ApprovalRequired { message, body } => {
            assert!(message.contains("awaiting approval"), "{message}");
            assert!(!body.is_empty(), "body preserved for the caller");
        }
        other => panic!("expected ApprovalRequired, got {other:?}"),
    }
}

#[tokio::test]
async fn rate_limited_is_429_typed_error_with_retry_after() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "12")
                .set_body_json(json!({
                    "error": "rate_limited",
                    "retry_after_seconds": 12u64
                })),
        )
        .mount(&server)
        .await;
    let err = client
        .append_user_message(None, None, "sess_1", "hi")
        .await
        .expect_err("rate_limited must surface as an error");
    match err {
        ActantError::RateLimited {
            retry_after_seconds,
            body,
        } => {
            assert_eq!(retry_after_seconds, Some(12));
            assert!(!body.is_empty());
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[tokio::test]
async fn idempotent_replay_is_200_typed_error() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "idempotent_replay",
            "message": "command_id=cmd_42 replayed"
        })))
        .mount(&server)
        .await;
    let err = client
        .create_session(None, None, Some("dup"))
        .await
        .expect_err("idempotent_replay must surface even at HTTP 200");
    match err {
        ActantError::IdempotentReplay { message, body } => {
            assert!(message.contains("replayed"), "{message}");
            // The body is the full original CommandResponse — preserved
            // verbatim so the caller can decode the recorded result.
            assert!(!body.is_empty());
        }
        other => panic!("expected IdempotentReplay, got {other:?}"),
    }
}

#[tokio::test]
async fn not_found_404() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": "not_found",
            "message": "session_id=sess_404 not found"
        })))
        .mount(&server)
        .await;
    let err = client
        .events("sess_404")
        .await
        .expect_err("404 -> NotFound");
    match err {
        ActantError::NotFound { message, .. } => assert!(message.contains("sess_404")),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn permission_denied_403() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": "permission_denied",
            "message": "actor_id=act_user lacks scope=memory.write"
        })))
        .mount(&server)
        .await;
    let err = client
        .approve_memory(None, None, "cand_1")
        .await
        .expect_err("403 -> PermissionDenied");
    match err {
        ActantError::PermissionDenied { message, .. } => {
            assert!(message.contains("memory.write"));
        }
        other => panic!("expected PermissionDenied, got {other:?}"),
    }
}

#[tokio::test]
async fn invalid_input_400() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_input",
            "message": "command_type is required"
        })))
        .mount(&server)
        .await;
    let err = client
        .append_user_message(None, None, "sess_1", "x")
        .await
        .expect_err("400 -> InvalidInput");
    assert!(matches!(err, ActantError::InvalidInput { .. }));
}

#[tokio::test]
async fn internal_500() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": "internal",
            "message": "storage write failed"
        })))
        .mount(&server)
        .await;
    let err = client
        .append_user_message(None, None, "sess_1", "x")
        .await
        .unwrap_err();
    assert!(matches!(err, ActantError::Internal { .. }));
}

#[tokio::test]
async fn approval_denied_403_distinct_from_permission_denied() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": "approval_denied",
            "message": "approver rejected"
        })))
        .mount(&server)
        .await;
    let err = client
        .approve_tool_call(None, None, "tc_1", "once")
        .await
        .unwrap_err();
    assert!(
        matches!(err, ActantError::ApprovalDenied { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn unknown_error_kind_falls_through_to_http() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .respond_with(ResponseTemplate::new(418).set_body_json(json!({
            "error": "teapot",
            "message": "I'm a teapot"
        })))
        .mount(&server)
        .await;
    let err = client
        .append_user_message(None, None, "sess_1", "x")
        .await
        .unwrap_err();
    match err {
        ActantError::Http {
            status,
            kind,
            message,
            ..
        } => {
            assert_eq!(status, 418);
            assert_eq!(kind, "teapot");
            assert_eq!(message, "I'm a teapot");
        }
        other => panic!("expected Http catch-all, got {other:?}"),
    }
}

#[tokio::test]
async fn naked_5xx_without_envelope_falls_through_to_http() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/healthz"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
        .mount(&server)
        .await;
    let err = client.healthz().await.unwrap_err();
    match err {
        ActantError::Http { status, kind, .. } => {
            assert_eq!(status, 502);
            assert_eq!(kind, "http_502");
        }
        other => panic!("expected Http catch-all, got {other:?}"),
    }
}
