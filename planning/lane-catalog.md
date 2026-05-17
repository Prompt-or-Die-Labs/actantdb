# Lane catalog

Every async lane in ActantDB: what it consumes, what it produces, which crate implements it, which deployment modes enable it. Defined in `/specs/19-performance-architecture.md`; ADR-0018 commits to this pattern.

A **lane** is a subscriber to `agent_event` (and sometimes projection rows) that does work asynchronously and writes back through ordinary commands.

## Catalog

| Lane                       | Subscribes to                                  | Produces                                       | Crate                  | Modes                                |
| -------------------------- | ---------------------------------------------- | ---------------------------------------------- | ---------------------- | ------------------------------------ |
| **workflow-advance**       | `workflow_step_completed`, `effect_completed`  | `complete_workflow_step` command               | `actant-flow`          | all                                  |
| **memory-candidate**       | `user_message_received`, `tool_call_finished`, `effect_observed` | `propose_memory` command          | `actant-memory`        | developer, team, enterprise, regulated, research |
| **memory-embed**           | `memory_approved`                              | `memory.embed` effect → `embedding_ref` row    | `actant-index` + `actant-embedders` | all (local-fast: local only)         |
| **entity-extract**         | `indexed_object` create, `memory_approved`     | `entity` + `entity_relation` rows              | `actant-index`         | developer+                            |
| **retrieval-trace**        | `context_build_finished`                       | enrichment on `retrieval_trace` rows           | `actant-index`         | all                                  |
| **rerank**                 | (called inline during context_build by index)  | `rerank_score` on `retrieval_candidate`        | `actant-embedders`     | balanced+ profile                    |
| **risk-explanation**       | `tool_call_requested`                          | enrichment on `tool_call.risk_explanation`     | `actant-models` + lane | developer+                            |
| **source-quality**         | `tool_call_finished`, `effect_observed`         | enrichment on `observation.confidence`         | `actant-index`         | team+                                 |
| **eval-shadow**            | every event with an open eval candidate         | `eval_run` rows                                | `actant-eval`          | developer+ (sampled in team+)         |
| **eval-from-failure**      | `dead_letter_item`                              | `eval_case` rows                               | `actant-eval`          | developer+                            |
| **regret-watch**           | `tool_call_finished` (failed), user corrections | `regret_event` rows                            | `actant-command::regret` | developer+                          |
| **trust-recalc**           | every event involving an actor decision         | updated `trust_profile` rows                   | `actant-trust`         | developer+                            |
| **drift-detect**           | `intent`, `tool_call_requested`                 | `drift_signal` rows; optional `intervention`   | `actant-policy::drift` | developer+                            |
| **context-debt**           | `context_build_finished`                        | `context_debt` row                             | `actant-context`       | developer+                            |
| **otel-export**            | every event (sampled)                           | OTLP traces to configured exporter             | `actant-trace`         | developer (stdout), team+ (OTLP)      |
| **audit-export**           | every event (nightly batch)                     | JSONL archives via `actant-audit-export`       | `actant-audit-export`  | team+                                 |
| **artifact-summary**       | `artifact_created`                              | `artifact.summary` field (progressive enrich)  | lane crate (TBD)       | developer+                            |
| **prompt-injection-scan**  | `effect_observed` with text content             | flag on `observation.verification_status`       | `actant-context::redact` | developer+                          |
| **incident-pattern**       | clusters of failed tool calls / policy denials  | `incident` candidate (Phase 4)                  | (Phase 4 crate)         | enterprise, regulated                 |
| **compliance-evidence**    | nightly                                          | rollup files for SOC 2 / similar                | `actant-audit-export`  | enterprise, regulated                 |
| **capability-token-refresh** | `permission_changed`, `policy_changed`         | flush `actant-kernel::HotState` tokens         | `actant-kernel`        | all                                  |
| **rate-limit-snapshot**    | periodic                                        | durable snapshots of in-memory counters         | `actant-throttle`      | all                                  |
| **circuit-decay**          | periodic                                        | reset / half-open transitions                   | `actant-circuit`       | all                                  |
| **dlq-notify**             | `effect.dead_lettered`                          | `human.notify` effect                          | `actant-effects::dlq`  | all                                  |
| **sync-replicate**         | every event (filtered by capsule sync_policy)   | outbound replication to peers / cloud           | `actant-sync`          | local-first + team+                    |
| **analytics-rollup**       | every event (sampled)                           | rows into columnar side store                   | (Phase 6 crate)         | team+                                 |

## Lane discipline

Every lane:

1. **Subscribes** through `actant-subscribe`; filters must be indexed.
2. **Writes back through commands**, not direct table writes. Even enrichments use a (small) command.
3. **Records its own freshness target** as an OTel metric (`lane_freshness_seconds{lane=...}`).
4. **Honors backpressure** — emits `lane_backpressure_signal` when its queue grows; the kernel responds per `/specs/19-performance-architecture.md` §9.
5. **Is sensitivity-aware** — never lifts content past its source's visibility / sync policy.
6. **Is gated by deployment mode** — checks `workspace.deployment_mode` at startup; refuses to subscribe in modes that haven't enabled it.

## Adding a new lane

1. Identify the trigger events and the writes the lane will produce.
2. Add a row in this catalog.
3. Implement the subscriber in the named crate (or a new one, if substantial).
4. Add a test asserting the lane does NOT activate in unsupported modes.
5. Add a freshness metric in `actant-trace`.
6. Update `/planning/phase-N-plan.md` if the lane requires phase-bound dependencies.

## Anti-patterns

- A lane that opens a database transaction longer than 100 ms (this includes time waiting on a model). Split into multiple commands.
- A lane that subscribes globally to all events. Use a filtered subscription.
- A lane that reads from another lane's enrichment output as if it were authoritative. Authority lives on the row at commit time; enrichments are descriptive (ADR-0019).
- A lane that calls another lane synchronously. Always go through the chronicle.
