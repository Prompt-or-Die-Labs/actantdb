# SDK design — Rust

Crate: `actant-client` (separate from the workspace; published to crates.io independently).

## Tech

- MSRV 1.75.
- tokio runtime by default.
- Builder pattern for command inputs (avoids exploding positional args).
- `Stream` for subscriptions, with `Drop` sending `unsubscribe`.
- `serde` end-to-end. Types implement `Serialize + Deserialize`.

## API

```rust
use actant_client::{ActantClient, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ActantClient::new(Config::from_env()?);

    let session = client.command()
        .create_session()
        .agent_actor_id("agent_123")
        .title("Fix failing tests")
        .send()
        .await?;

    let mut sub = client.subscribe()
        .table("approval_request")
        .eq("status", "pending")
        .open()
        .await?;

    while let Some(event) = sub.next().await {
        // ...
    }
    Ok(())
}
```

## Distribution

- Published under `actant-client` on crates.io.
- Source under `sdks/rust/`.
- Generated code under `sdks/rust/src/generated.rs`.
- Codegen from `actant-sdk-codegen --target rust --out sdks/rust/src/generated.rs`.

## Why separate from the workspace

The Rust SDK is meant for *consumers* of ActantDB, not workspace members. Pinning it inside the workspace would force consumers to depend on the entire repo. The SDK depends only on `actant-core` (for shared types) — at the published version, not via path.

## Versioning

Same schema major as TS/Python/Swift.
