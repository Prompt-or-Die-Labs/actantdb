# Work package: `actant-worker-model`

## Context

Reference worker for `model.call` effects. Adapts OpenAI-compatible HTTP endpoints (OpenAI, Anthropic via shim, vLLM, Ollama, MLX local server, LM Studio). Streams tokens, accounts cost.

## Specs to read first

- `/specs/04-effect-protocol.md` §7 (`model.call`).
- `/specs/06-context-and-memory.md` §1 (context manifest — the worker reads `final_prompt_ref`).
- `/specs/14-extended-primitives.md` §12 (Model route decision — Phase 4; Phase 2 worker records inputs that make Phase 4 capture trivial).
- `/specs/05-security-model.md` §4 (visibility — refuses cloud route on `local_only` lease).

## Scope

### Behavior

- Load secret material from `secret_ref` via the server (the server passes the materialized header / env var inside the lease response, never the raw token).
- Read `final_prompt_ref` from the lease, fetch via the artifact API.
- Send request to provider. Stream chunks back; each chunk becomes an `observe` call carrying token-count progress.
- On completion: emit a single final observation (`evidence_type='model_output'`), call `complete` with response_ref, token counts, cost, latency.
- Refuse to target a cloud route if the lease's visibility is `local_only`.

### Internal modules

```
crates/actant-worker-model/src/
├── main.rs
├── lib.rs
├── provider/
│   ├── mod.rs
│   ├── openai.rs       // OpenAI Chat Completions + Responses
│   ├── anthropic.rs    // Anthropic Messages (with shim to OpenAI-compat where possible)
│   ├── ollama.rs
│   ├── mlx.rs          // MLX local server
│   └── compat.rs       // generic OpenAI-compatible adapter
├── stream.rs           // SSE / NDJSON streaming
└── cost.rs             // per-route cost-per-1k mapping
```

### Tests

- Streaming: a 3-chunk SSE response produces 3 observations.
- Visibility refusal: a `local_only` lease against an `openai` route returns `precondition_failed`.
- Cost accounting: a recorded 1000-input/500-output completion under a route with `cost_per_input_1k=$0.01, cost_per_output_1k=$0.03` reports `cost_usd=0.025`.
- Resilience: a 502 from the provider followed by a 200 produces one effect completion, not two.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Smoke test against at least one local provider (e.g. Ollama in CI service container).
- [ ] Cost math matches the documented `model_route` rates to within 1e-6.

## Do NOT

- Do NOT call providers that aren't whitelisted in the lease.
- Do NOT log raw API keys. They appear only via env vars on the spawn.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
