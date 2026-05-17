# ActantDB

The autonomous-action backend.

This documentation is generated from the canonical specs under [`/specs`](https://github.com/actantdb/actantdb/tree/main/specs) and the operational docs at the repo root (CHANGELOG, GATES, RELEASE_CHECKLIST, SPECS_STATUS).

Build locally with:

```bash
mdbook build docs
open docs/book/index.html
```

## What's in here

- **Specs 00–19**: the canonical product surface. Each spec ships a
  `## Verification` section that is enforced by a Rust test under
  `crates/<crate>/tests/spec_NN_verification.rs`. Run them with
  `cargo test --workspace`.
- **SLOs**: production targets the v1 substrate is held to.
- **Release checklist**: the step-by-step that lands a release.
- **SPECS_STATUS**: which specs are verified and what the verifiers assert.

## Quick start

```bash
cargo run -p actant-server --bin actantdb-server -- --bind 127.0.0.1:4555
```

```bash
curl http://127.0.0.1:4555/v1/healthz/ready
curl -X POST http://127.0.0.1:4555/v1/command \
  -H 'content-type: application/json' \
  -d '{"workspace_id":"ws_default","actor_id":"act_system","command_type":"create_session","input":{}}'
```
