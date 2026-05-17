# Work package: `actant-trace`

## Context

OpenTelemetry + OpenInference emitters. Linked into every ActantDB subsystem so spans for command, effect, model.call, tool.call, embedding, retrieval, reranker, context.build, approval, memory, replay, eval, workflow, agent are emitted with the right attributes — and sensitivity-aware redaction happens at the exporter chokepoint.

## Specs to read first

- `/specs/17-observability.md` — full file.
- `/specs/adr/0015-otel-genai-openinference.md`.

## Scope

```rust
pub struct Tracer { /* SpanExporter + redactor + sampler */ }

pub fn init(config: ObserveConfig) -> Result<Tracer, TraceError>;
pub fn shutdown();

pub fn span_model_call(req: &ModelCallSpan) -> SpanHandle;
pub fn span_tool_call(req: &ToolCallSpan) -> SpanHandle;
pub fn span_retrieval(req: &RetrievalSpan) -> SpanHandle;
pub fn span_embedding(req: &EmbeddingSpan) -> SpanHandle;
pub fn span_rerank(req: &RerankSpan) -> SpanHandle;
pub fn span_context_build(req: &ContextBuildSpan) -> SpanHandle;
pub fn span_workflow_step(req: &WorkflowStepSpan) -> SpanHandle;
pub fn span_approval(req: &ApprovalSpan) -> SpanHandle;
pub fn span_replay(req: &ReplaySpan) -> SpanHandle;

pub fn metric_workflow_duration(observe: f64, attrs: &Attrs);
// ... full metric set per /specs/17-observability.md §3
```

### Internal modules

```
crates/actant-trace/src/
├── lib.rs
├── init.rs                      (OTel SDK setup; OTLP gRPC + stdout exporters)
├── attributes.rs                (gen_ai.* + openinference.* + actant.* keys)
├── spans/                       (one file per span kind)
├── metrics/                     (counters, histograms, gauges)
├── redact.rs                    (sensitivity-aware payload redaction)
└── error.rs
```

### Tests

- A `model.call` span carries `gen_ai.request.model` + `openinference.span.kind=LLM` + `actant.workspace_id`.
- A high-sensitivity `tool.call` records `arguments_hash`, not `arguments`.
- Embedding spans never carry the vector or the raw text.
- Metrics export to OTLP correctly under load (1000 spans/s).

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Every span name in `/specs/17-observability.md` §1 has an emit site.
- [ ] Redaction chokepoint is the only place sensitive bytes touch the exporter buffer.

## Do NOT

- Do NOT add fields outside the OpenTelemetry GenAI + OpenInference + `actant.*` namespaces.
- Do NOT call providers from inside trace code (no enrichment fetches).
- Do NOT use `unsafe`.

## Hand-off

`just ci`. Then run `actant dev` and verify spans show up in stdout pretty-print exporter.
