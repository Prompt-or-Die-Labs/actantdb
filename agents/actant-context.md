# Work package: `actant-context`

## Context

`actant-context` is the **Context Engine** — model-context firewall and manifest builder. Every model call produces a `context_build` + N `context_item` rows. Phase 1 ships a minimal version that the alpha demo exercises.

## Specs to read first

- `/specs/06-context-and-memory.md` — full file.
- `/specs/05-security-model.md` §3 (sensitivity), §4 (visibility).
- `/specs/02-data-model.sql` — `context_build`, `context_item`, `model_route`.

## Scope (Phase 1)

### Public API surface

```rust
pub struct ContextRequest {
    pub session_id: SessionId,
    pub purpose: String,                  // "planner", "executor", "critic", ...
    pub model_route_id: String,
    pub token_budget: u32,
    pub candidate_filters: CandidateFilters,
}

pub struct ContextBuilder { /* holds Storage + scorer + redactors */ }

impl ContextBuilder {
    pub async fn build(&self, tx: &mut actant_storage::Transaction<'_>, req: ContextRequest)
        -> Result<ContextBuild, ContextError>;
}

pub struct ContextBuild { pub id: ContextBuildId, pub items: Vec<ContextItem>, /* ... */ }
```

### Pipeline stages

1. Gather: messages window + active memories + system prompt + session-attached files.
2. Score: weighted recency + keyword overlap (Phase 1 simple scorer).
3. Firewall: drop items whose sensitivity/visibility doesn't fit the route.
4. Redact: secret patterns, basic PII.
5. Truncate: greedy by rank within token budget; pinned items are non-truncatable.
6. Emit: write `context_build` + `context_item` rows inside the caller's `Transaction`.

### Internal modules

```
crates/actant-context/src/
├── lib.rs
├── request.rs
├── pipeline.rs
├── gather.rs
├── score.rs                 // pluggable Scorer trait
├── firewall.rs
├── redact.rs                // Phase 1: secret patterns + basic PII
├── truncate.rs
└── error.rs
```

### Tests

- Firewall correctly drops items with insufficient visibility/sensitivity for a cloud route.
- Pinned items never truncated even when budget is exceeded.
- Redaction removes obvious secrets (AWS access keys, GitHub tokens, `RSA PRIVATE KEY` blocks).
- `context_build.blocked_item_count` matches the number of blocked candidates.
- Token-budget accounting respects pinned-only-exceed → returns `precondition_failed`.

## Acceptance criteria

- [ ] `cargo build -p actant-context` zero warnings.
- [ ] `cargo test -p actant-context` passes.
- [ ] `cargo clippy -p actant-context -- -D warnings` passes.
- [ ] For a candidate set containing a `secret`-sensitivity item against a `cloud_model_allowed`-only route, the item appears in `context_item` with `included=0`, `blocked_reason="sensitivity"` (or `"visibility"`, whichever applied first).

## Do NOT

- Do NOT call out to an LLM for scoring in Phase 1. The scorer is a pluggable trait but the default impl is local-only.
- Do NOT write to vector stores. That's Phase 3.
- Do NOT inline policy logic. Use `actant-policy`.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
