# 17 — Observability

ActantDB emits OpenTelemetry-compatible spans, metrics, and events for every operation. The conventions follow:

- **OpenTelemetry GenAI semantic conventions** — model spans, agent spans, exceptions, metrics.
- **OpenInference** — span kinds for LLM, embedding, retriever, reranker, tool, agent, guardrail, evaluator, prompt.

This makes ActantDB integrate with Phoenix/Arize, LangSmith, Datadog, Grafana, Honeycomb, and any OTLP-compatible collector. **ActantDB is not a telemetry silo.**

## 1. Span catalog

Every span emitted carries the matching OpenInference/OTel attribute keys plus an `actant.*` namespace for ActantDB-specific context.

| Span name                | Kind (OpenInference) | Emitted by                  |
| ------------------------ | -------------------- | --------------------------- |
| `workflow.run`           | agent                | actant-flow                 |
| `workflow.step`          | agent                | actant-flow                 |
| `agent.turn`             | agent                | actant-command (sessions)   |
| `model.call`             | llm                  | actant-effects (model.call) |
| `tool.call`              | tool                 | actant-effects (tool.call)  |
| `embedding`              | embedding            | actant-embedders            |
| `retrieval.search`       | retriever            | actant-index                |
| `reranker.call`          | reranker             | actant-embedders            |
| `context.build`          | guardrail            | actant-context              |
| `memory.propose`         | evaluator            | actant-memory               |
| `memory.approve`         | guardrail            | actant-command              |
| `approval.wait`          | guardrail            | actant-policy               |
| `rate_limit.decision`    | guardrail            | actant-throttle             |
| `retry.attempt`          | tool                 | actant-effects              |
| `circuit.breaker`        | guardrail            | actant-circuit              |
| `replay.run`             | agent                | actant-replay               |
| `eval.run`               | evaluator            | actant-eval                 |
| `effect.claim`           | tool                 | actant-effects              |
| `effect.execute`         | tool                 | worker-protocol             |
| `protocol.mcp`           | tool                 | actant-protocol             |
| `protocol.a2a`           | agent                | actant-protocol             |
| `protocol.ap2`           | guardrail            | actant-protocol             |

## 2. Required attributes

For every span:

```
actant.workspace_id
actant.actor_id
actant.session_id (when applicable)
actant.workflow_run_id (when applicable)
actant.event_id (link to chronicle)
actant.sensitivity (highest sensitivity touched)
actant.policy_id
```

For `model.call`:

```
gen_ai.system                openai|anthropic|local|...
gen_ai.request.model
gen_ai.response.model
gen_ai.usage.input_tokens
gen_ai.usage.output_tokens
gen_ai.request.temperature
gen_ai.request.max_tokens
gen_ai.response.finish_reasons
```

For `embedding`:

```
openinference.span.kind       EMBEDDING
embedding.model               ...
embedding.input_type          query|document
embedding.dimension           ...
embedding.count               ...
```

For `retrieval.search`:

```
openinference.span.kind                 RETRIEVER
retrieval.query                         (redacted per policy)
retrieval.mode                          basic|hybrid|graph|deep
retrieval.candidate_count
retrieval.selected_count
retrieval.blocked_count
retrieval.reranker
```

For `tool.call`:

```
openinference.span.kind   TOOL
tool.name
tool.kind
tool.risk_level
tool.idempotency_key
```

## 3. Metrics

```
# Workflow + effects
workflow_success_rate                  histogram by workflow_name
workflow_duration_seconds              histogram
effect_queue_depth                     gauge by queue_name
approval_wait_seconds                  histogram
worker_in_flight                       gauge by worker_id

# Models + retrieval
model_call_latency_ms                  histogram by provider + model
model_tokens_input_total               counter by provider + model
model_tokens_output_total              counter
model_cost_usd_total                   counter by provider + model
embedding_latency_ms                   histogram
retrieval_recall_at_k                  histogram by eval_case
reranker_lift                          gauge by reranker_model
cache_hit_rate                         gauge by cache_type

# Reliability
rate_limit_delays_seconds              histogram by policy
retry_count_total                      counter by effect_type
circuit_state_total                    counter by dependency + state
budget_remaining                       gauge by scope_type + scope_id

# Memory / replay
memory_approval_rate                   gauge
memory_correction_rate                 gauge
replay_divergence_rate                 gauge
eval_pass_rate                         gauge by eval_case
```

## 4. Sensitivity-aware redaction

OTel spans MUST NOT carry sensitive payload bytes. Redaction rules:

- `model.call` records prompt hash, not prompt text, when `actant.sensitivity >= medium`.
- `tool.call` records arguments hash, not arguments, when the tool's `default_risk_level >= high`.
- `embedding` never records the raw vector or text; counts + model + dimension only.
- `retrieval.search` records the query as hash when the query came from a `high`+ source.

Redaction is implemented in `actant-trace`. The exporter pipeline applies redaction before bytes leave the process.

## 5. Exporters

```
OTLP gRPC                actant-trace default; works with Phoenix, Arize, LangSmith, Datadog, Grafana, Honeycomb, Tempo, etc.
OTLP HTTP                same data; convenient for browser-side or restricted networks.
Local file               JSON-lines for dev / replay.
stdout                   pretty-printed for `actant dev` developer view.
```

Configuration via `actant.yaml`:

```yaml
observe:
  exporter: otlp
  otlp:
    endpoint: ${OTEL_EXPORTER_OTLP_ENDPOINT}
    protocol: grpc
  sampling: parent_based(traceidratio(0.1))
  redaction:
    enforce_sensitivity_floor: medium
```

## 6. Correlation with the Chronicle

Every chronicle event carries an OTel `trace_id` + `span_id` link when one is active. Replay reconstructs the exact span tree by re-emitting spans with the recorded IDs in a sandboxed exporter.

Schema addition:

```sql
ALTER TABLE agent_event ADD COLUMN otel_trace_id TEXT;
ALTER TABLE agent_event ADD COLUMN otel_span_id  TEXT;
```

## 7. What ActantDB does NOT do

- Does **not** ship a dashboard. Use Phoenix / Arize / Grafana / LangSmith for trace UI.
- Does **not** invent a private trace format. OpenTelemetry GenAI + OpenInference are the contract.
- Does **not** require an OTel collector. The OTLP exporter is optional; `actant dev` defaults to stdout pretty-print.

## 8. Phase staging

| Phase | Observability                                                  |
| ----- | -------------------------------------------------------------- |
| 1     | OTel SDK wired; stdout exporter; spans for command, effect, model.call, tool.call. |
| 2     | retrieval, reranker, embedding, approval, rate_limit, retry, circuit spans. Worker-side spans. |
| 3     | Memory + replay spans; metrics dashboards (sample Grafana JSON in `deploy/`). |
| 4     | Workflow + eval spans + cost metrics; full metric set above.   |
| 5     | Replay-mode spans (synthetic; tagged `actant.replay_run_id`).  |
| 6     | Production exporters (Datadog, Honeycomb), sampling policies per workspace. |

## Verification

- [ ] Every span name in §1 has an emit site in the named crate.
- [ ] Every required attribute in §2 is set or explicitly omitted with a comment.
- [ ] Redaction in §4 is implemented as a single chokepoint in `actant-trace`.
- [ ] `agent_event.otel_*` columns are populated for every event emitted under an active span.
