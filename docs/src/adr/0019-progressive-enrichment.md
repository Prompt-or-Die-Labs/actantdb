# ADR-0019: Progressive enrichment

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

The hot kernel must commit fast (ADR-0018). But many useful pieces of information about a single event — natural-language risk explanations for an approval, source-quality scores on retrieved content, compliance mappings on a tool call, eval candidate flags, summary text on a tool output — require expensive computation: model calls, classifier runs, vector searches, regex scans.

If the hot path waits for those, commands slow down. If those are dropped entirely, ActantDB loses much of its value. The middle path: write minimal facts immediately, add richness later.

## Decision

Every projection row supports **progressive enrichment**:

- The hot path inserts the row with the small set of facts the kernel knows synchronously.
- One or more async lanes consume the row's emission event, do their work, and **update the row's enrichment fields** through their own commands.
- Subscribers see both states: a row with only minimal facts, then a row with enriched fields populated.

Concretely, for `tool_call_requested`:

```
Immediate (kernel):
  tool_name, redacted args hash, actor_id, status, risk_class (compiled)

Within seconds (lanes):
  risk_explanation       (natural-language; written by the risk-explanation lane)
  source_quality_impact  (set by the source-quality lane)
  compliance_mapping     (set by the compliance lane)
  eval_candidate         (set by the eval-candidate lane)
```

Each enrichment field has a stable name + an "enriched_at" timestamp. UI consumers render minimal facts immediately and update through subscriptions when enrichment lands. Replay can reproduce both the minimal-state and the enriched-state views.

Enrichment fields are **never** authority-bearing. Authority lives on the row at commit time; enrichment is descriptive. A risk explanation cannot demote `risk_class`; only an explicit policy update can.

## Consequences

### Positive

- Hot path stays small.
- ActantDB still surfaces the rich, human-readable context auditors and developers want.
- Adding a new enrichment lane is non-breaking: the field defaults to null until the lane fills it.
- Studio can render two-tier UI (instant + enriched) cleanly.

### Negative

- Two-tier state for every enriched row. UI must handle both views.
- Replay diffs must distinguish "missing enrichment because the lane hasn't run" from "missing enrichment because it was never computed." Mitigated by a `lane_status` field on each enrichment that records `pending | running | completed | skipped`.
- Storage growth from enrichment columns. Mitigated by keeping enrichments small (text + timestamps) and pushing large payloads to artifact refs.

### Neutral / open

- Whether enrichment writes go through full commands (with audit) or a lighter-weight "enrich" path is unresolved. Phase 2 starts with full commands for safety; Phase 4 may add an enrich-only path with the same authority bounds.

## Alternatives considered

- **All enrichments synchronous.** Rejected — kills latency.
- **All enrichments external (in tracing tool).** Rejected — fragments the audit trail; ActantDB loses its single-source-of-truth promise.
- **Enrichments in a separate table.** Rejected — joins everywhere; the per-row update pattern is simpler for consumers.

## References

- `/specs/19-performance-architecture.md` §12 (Progressive enrichment).
- `/specs/adr/0018-hot-kernel-async-lanes.md`.
- `/planning/lane-catalog.md`.
