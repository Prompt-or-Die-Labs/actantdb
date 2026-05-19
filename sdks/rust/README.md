# ActantDB Rust SDK

Rust client for an [ActantDB](../../README.md) server. `tokio`-based, async
end-to-end, with a typed error surface that mirrors the server's wire-level
error envelope (including the 2xx-with-`error`-body cases like
`approval_required` and `idempotent_replay`).

## Install

While the workspace is local, depend on the SDK via a `path` reference:

```toml
[dependencies]
actantdb-client = { path = "../actantDB/sdks/rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
url = "2"
```

Future crates.io publication will lift the path reference; in the meantime
the crate is part of the ActantDB Cargo workspace and built / tested with
`cargo test --workspace`.

## Usage

```rust
use actantdb_client::{ActantClient, ReplayMode, Sensitivity, SubscriptionKind};
use serde_json::json;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), actantdb_client::ActantError> {
    let client = ActantClient::new(Url::parse("http://127.0.0.1:4555").unwrap())
        .with_token("dev-token")          // omit in local_mode loopback
        .with_workspace_id("ws_default")  // default for convenience methods
        .with_actor_id("act_user");

    // Health
    let _ = client.healthz_ready().await?;

    // Sessions + messages
    let session_id = client
        .create_session(None, None, Some("fix failing tests"))
        .await?;
    client
        .append_user_message(None, None, &session_id, "clean up the build dir")
        .await?;

    // Tool calls + approvals
    let resp = client
        .request_tool_call(
            None, None,
            &session_id,
            "shell.run",
            json!({"cmd": "rm -rf build"}),
        )
        .await?;
    println!("verdict: {:?}", resp.result);

    let pending = client.approvals(None).await?;
    for p in pending {
        client.approve_tool_call(None, None, &p.tool_call_id, "once").await?;
    }

    // Memory candidates
    client
        .propose_memory(
            None, None,
            "user prefers dark mode",
            "preference",
            Sensitivity::Low,
            0.95,
            json!({"source": "chat"}),
        )
        .await?;

    // Replay
    let cp_id = client.replay_checkpoint(None, "evt_xyz").await?;
    let diff = client
        .replay_run(None, &cp_id, ReplayMode::Memory)
        .await?;
    println!("diff entries: {}", diff.entries.len());

    // Live subscription (WebSocket)
    use futures::StreamExt;
    let mut stream = client
        .subscribe(None, Some(&session_id), SubscriptionKind::Events)
        .await?;
    while let Some(msg) = stream.next().await {
        let msg = msg?;
        println!("topic={:?} payload={}", msg.topic.kind, msg.payload);
    }

    Ok(())
}
```

## Convenience-builder pattern

Every endpoint method accepts `Option<&str>` for `workspace_id` and `actor_id`.
Passing `None` falls back to the defaults set via `.with_workspace_id(...)`
/ `.with_actor_id(...)`. Pass `Some("ws_other")` when you need to override on
a single call. The same builder also accepts `.with_token(...)` and
`.with_http_client(...)` for custom `reqwest` configuration (timeouts, TLS,
proxies).

## Typed errors

Each call returns `Result<T, ActantError>`. The server's typed-error wire
shape (`{"error":"<kind>","message":"..."}`) is decoded **regardless of HTTP
status code** — the server reuses this envelope at 200, 202, 4xx, and 5xx.

| variant                | wire `error` kind        | typical HTTP |
| ---------------------- | ------------------------ | ------------ |
| `InvalidInput`         | `invalid_input`          | 400          |
| `NotFound`             | `not_found`              | 404          |
| `PermissionDenied`     | `permission_denied`      | 403          |
| `ApprovalRequired`     | `approval_required`      | **202**      |
| `ApprovalDenied`       | `approval_denied`        | 403          |
| `IdempotentReplay`     | `idempotent_replay`      | **200**      |
| `RateLimited`          | `rate_limited`           | 429          |
| `MissingAuthorization` | `missing_authorization`  | 401          |
| `InvalidToken`         | `invalid_token`          | 401          |
| `WorkspaceMismatch`    | `workspace_mismatch`     | 403          |
| `Internal`             | `internal`               | 500          |
| `Http`                 | _any other kind_         | _any_        |
| `Transport`            | _network failure_        | n/a          |
| `Decoding`             | _local JSON parse fail_  | n/a          |
| `WebSocket`            | _subscribe stream fail_  | n/a          |

`ApprovalRequired` and `IdempotentReplay` are **typed signals**, not failures.
The raw response body is preserved verbatim on every variant so the caller can
re-parse the approval payload or the recorded `CommandResponse`.

## Tests

```bash
cargo test -p actantdb-client          # ~25 mocked tests via wiremock
ACTANTDB_TEST_URL=http://127.0.0.1:4555 \
    cargo test -p actantdb-client --test integration_live
```

## Domain vs. wire types

Domain types (`Sensitivity`, `Risk`, `PolicyVerdict`, `ApprovalRequest`,
`ApprovalDecision`, `ReplayDiff`, …) come from the
[`actant-contracts`](../../crates/actant-contracts) crate — the single source
of truth per the F2/F3 binding rules. They are re-exported from the SDK root,
so most callers only need this one crate.

Wire envelopes (`CommandRequest`, `CommandResponse`, the storage-shaped
`AgentEvent` returned by `GET /v1/events`, `PendingApproval`, …) are defined
inside this SDK because they describe HTTP transport rather than domain. The
Swift SDK uses the same split.
