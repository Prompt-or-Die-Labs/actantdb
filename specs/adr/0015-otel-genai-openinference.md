# ADR-0015: Observability follows OpenTelemetry GenAI + OpenInference

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Agent systems need traces. Two standards have converged in 2025–2026:

- **OpenTelemetry GenAI semantic conventions** — model spans, agent spans, exceptions, metrics, provider-specific conventions for OpenAI / Anthropic / Bedrock / Azure AI.
- **OpenInference** — span kinds for LLM, embedding, retriever, reranker, tool, agent, guardrail, evaluator, prompt; widely adopted by Phoenix / Arize and LangSmith-compatible tools.

Inventing a private trace format would isolate ActantDB from the broader ecosystem and force users to choose between ActantDB's audit story and their existing observability stack.

## Decision

ActantDB emits OpenTelemetry-compatible traces with attributes that satisfy **both** OTel GenAI and OpenInference semantic conventions. Specifically:

- Every model call, tool call, embedding, retrieval, reranker, context build, approval, replay, eval, workflow node, and effect emits a span.
- Attributes follow the OTel GenAI namespace (`gen_ai.*`) plus the OpenInference `openinference.span.kind` attribute.
- ActantDB-specific context lives under `actant.*` (workspace, actor, session, sensitivity, policy).
- Sensitivity-aware redaction happens at the exporter chokepoint in `actant-trace`; sensitive payloads become hashes before bytes leave the process.
- Default exporter is OTLP-over-gRPC; stdout exporter for `actant dev`.

ActantDB does **not** ship a dashboard. Use Phoenix / Arize, LangSmith, Datadog, Grafana, Honeycomb, or any OTLP collector.

## Consequences

### Positive

- ActantDB integrates with every major LLM observability tool.
- No proprietary trace format. No lock-in.
- The agent-developer audience already knows the conventions.
- Compliance evidence (SOC 2, ISO) maps to existing telemetry pipelines.

### Negative

- Two conventions to satisfy. We accept the cost; the attribute sets are largely additive.
- Convention drift (both specs are still evolving). Mitigated by keeping the attribute set declarative in `actant-trace::attributes` so updates are local.

### Neutral / open

- Whether to bundle a default sampling policy (e.g. `parent_based(traceidratio(0.1))`) is a per-workspace decision; defaults documented but not enforced.

## Alternatives considered

- **Private trace format.** Rejected — silos ActantDB from the rest of the agent ecosystem.
- **OpenInference only.** Rejected — misses provider-specific GenAI conventions that ecosystem tools depend on.
- **OTel GenAI only.** Rejected — Phoenix/Arize-class tools rely on OpenInference span kinds.

## References

- `/specs/17-observability.md` — full span and metric catalog.
- `/agents/actant-trace.md`.
- OpenTelemetry semantic conventions for GenAI.
- OpenInference semantic conventions.
