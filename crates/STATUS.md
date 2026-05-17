# STATUS — mixed (post-pivot 2026-05-17)

This directory contains **both** active and frozen crates.

## Active (post-pivot — invisible to TS developers, bundled inside `@actantdb/core`)

- `actant-contracts/` — single source of truth for every public type. New crates must consume from here.
- `actant-kernel/` — fast Rust implementation.
- `actant-napi/` — Node native addon (NAPI-RS), bundled per-platform inside `@actantdb/core`.
- `actant-wasm/` — WASM fallback, bundled inside `@actantdb/core`.
- `actant-server/` — optional scale-out (later; not Phase 1).

## Frozen — v2 substrate roadmap

Every other crate under `crates/` is **v2 substrate roadmap**, not the v0.1 architecture:

```
actant-storage, actant-command, actant-effects, actant-context, actant-memory,
actant-flow, actant-replay (Rust v2), actant-subscribe, actant-cli,
actant-sdk-codegen, actant-templates, actant-schema-dsl, actant-codegen-project,
actant-worker-protocol, actant-worker-shell, actant-worker-file,
actant-worker-model, actant-worker-mcp, actant-embed, actant-embedders,
actant-capsule, actant-trust, actant-trigger, actant-eval, actant-sync,
actant-audit-export, actant-prompts, actant-models, actant-cache, actant-trace,
actant-protocol, actant-throttle, actant-circuit, actant-lock, actant-ingress,
actant-policy (Rust v2)
```

Their work packages live in `/agents/`. **Do not pick them up.** See [`/wedge/f2-f3-prevention.md`](../wedge/f2-f3-prevention.md) for why.

## How the active and frozen sets coexist

The Cargo workspace currently lists every crate. The frozen crates compile to empty `src/lib.rs` stubs; this is intentional — they reserve names and keep the workspace honest. New code goes only into the active crates above.

If the wedge passes Gate 3, the frozen crates begin to fill in as the v2 substrate roadmap. Until then they are name reservations.
