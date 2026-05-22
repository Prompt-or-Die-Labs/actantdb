# 12 — Glossary

Every defined term used across the spec set. Terms are listed alphabetically; cross-references in italics. New definitions belong here first, then in the spec where they apply.

---

**Actant.** Any entity capable of action. Used as the root of the product name and as the prefix for crates (`actant-core`) and types (`ActantEvent`). The set of actants in a workspace is the set of *actors*.

**Actor.** A row in the `actor` table. Each actor has a `kind`: `human`, `agent`, `subagent`, `model`, `tool`, `worker`, `system`. Every *command* names the actor that initiated it.

**Agent.** An autonomous process that issues *commands* on its own. The `actor.kind = 'agent'`. Distinguished from a *subagent* (which has scoped authority delegated from a parent agent) and from a *model* (which is a single model call, not an autonomous loop).

**Approval request.** A row in `approval_request`. Created by Guard when a *command* requires human approval. Resolved by an `approve_*` or `deny_*` command.

**Artifact.** A blob of content stored out-of-band, referenced from a row by `*_ref` columns. Has its own row in `artifact` (sensitivity, kind, URI, hash). Lives in the *Artifact Store*.

**Artifact Store.** A *companion store* for blobs (screenshots, tool outputs, files, transcripts, large model outputs). Phase 1 uses a local filesystem; later phases support S3-compatible storage.

**Audit event.** A denormalized projection over `agent_event` for fast auditor queries. Rebuildable from the Chronicle.

**Authority scope.** A row in `authority_scope`. Grants one *actor* one *permission* over one *resource pattern* up to one *sensitivity ceiling*. Composes by union.

**Backpressure.** The subscription system's response to a slow client. When the per-subscription buffer overflows, the server emits a `lag` notification and forces a re-snapshot.

**Candidate.** A *memory* in its pre-approval state. Lives in `memory_candidate`. Promoted to `memory` by `approve_memory`.

**Causality kind.** An `agent_event` column. One of `observation`, `intent`, `effect`, `control`, `audit`. Distinguishes "the world reported X" from "the agent decided X" from "X was performed."

**Checkpoint.** A row in `replay_checkpoint` plus four snapshot artifacts (state, model_route, permission, memory). Sufficient to reconstruct the world at the anchor event for any *replay mode*.

**Chronicle.** The append-only causal event ledger; the `agent_event` table. The source of truth for everything that happened.

**Command.** A typed, permission-checked mutation. The only path to write state. Recorded as a row in `command_record` and emits one or more *events*.

**Command engine.** The subsystem that authenticates, validates, authorizes, and commits *commands*.

**Companion store.** A storage system that ActantDB references but does not own: Artifact Store, Secret Vault, Semantic Index.

**Context build.** A row in `context_build`. The act of assembling a manifest for an upcoming *model call*. Produces N `context_item` rows.

**Context engine.** The subsystem that gathers, scores, firewalls, and redacts context for *model calls*.

**Context firewall.** The phase of context build that excludes candidates whose *visibility* or *sensitivity* is incompatible with the target *model route*.

**Context item.** A row in `context_item`. Records one candidate — included or blocked — that was considered for a *context build*.

**Context manifest.** The pair `(context_build, [context_item ...])`. The inspectable, replayable record of what a model saw.

**Cryptographic erasure.** A *deletion* mode in which the payload remains encrypted under a per-payload key in the *Secret Vault*; dropping the key renders the payload unreadable instantly. Used for high-sensitivity content where the audit skeleton must survive.

**Dead letter.** A terminal effect status reached after attempts are exhausted. Requires human re-entry to retry.

**Decision gate.** A binary pass/fail milestone at the end of each *phase* in `11-roadmap.md`. The next phase does not start until the gate passes.

**Deletion.** Removing payload while preserving the event skeleton. Modes: `delete_memory`, `delete_artifact`, `redact_event_payload`, `purge_secret_ref`, `expire_workspace`, *cryptographic erasure*.

**DAG.** Directed acyclic graph. The shape of a *workflow*.

**Effect.** A row in `effect`. Records the intent to perform a side effect (model call, tool call, shell, file, HTTP, etc.). Claimed and executed by a *worker* outside the database transaction that created it.

**Effect engine.** The subsystem owning the effect queue, claim protocol, retries, and dead-lettering.

**Effect result.** A row in `effect_result`. Per-attempt outcome record; allows replay to reconstruct effect outcomes without re-executing.

**Effect type.** The kind of side effect: `model.call`, `tool.call`, `shell.run`, `browser.act`, `file.read`, `file.write`, `http.request`, `calendar.read`, `email.draft`, `email.send`, `message.send`, `memory.embed`, `workflow.dispatch`, `human.notify`.

**Embedding ref.** A row in `embedding_ref`. Pointer to a vector in the *Semantic Index*; the vector itself does not live in ActantDB.

**Event.** A row in `agent_event`. Immutable record of something that happened. Carries actor, parent, type, payload hash, sensitivity, and a chained `event_hash` for tamper evidence.

**Event hash.** `SHA-256(parent_event_hash || payload_hash || canonical_metadata)`. Each event extends the chain; periodic checkpoint hashes anchor the chain externally.

**Firewall.** Short for *context firewall*.

**Flow engine.** The subsystem implementing *workflows*.

**Guard.** The policy/permissions/approval subsystem. Evaluates `(actor, permission, resource, sensitivity)` and returns `allow`, `allow_with_approval`, or `deny`.

**Heartbeat.** A worker's periodic message to extend its lease on a claimed effect. Missing two consecutive heartbeats forfeits the claim.

**Idempotency key.** A caller-provided string that pairs with `workspace_id` to deduplicate commands and effects. Required on every effect; recommended on commands.

**Invariant.** A statement that must hold for the system to be correct. Catalogued in `05-security-model.md` §2.

**Ledger.** Short for *Chronicle*.

**Local-only.** A *replay mode* and a *visibility* tag. As a mode, forbids cloud context and cloud model calls during replay. As a visibility tag (`never_sync`), forbids the content from leaving the device.

**Manifest.** Short for *context manifest*.

**Memory.** A row in `memory`. An approved, durable piece of knowledge about the world. Has provenance (`source_event_ids`, `source_candidate_id`), sensitivity, visibility, and a usage history (`memory_use`).

**Memory candidate.** See *Candidate*.

**Memory engine.** The subsystem owning the memory lifecycle.

**Memory use.** A row in `memory_use`. Records that a memory appeared in a specific `context_build`, optionally associated with a `model_call`.

**Model call.** A row in `model_call`. A single invocation of a model route, with attached `context_build`, request/response refs, tokens, cost, latency.

**Model route.** A row in `model_route`. Names a provider + model + minimum required visibility + sensitivity ceiling + cost metadata. Routes are the unit Guard checks for model permissions.

**Node.** A row in `workflow_node`. One step type in a *workflow*.

**Observation.** An *event* with `causality_kind = 'observation'`. Records something the world did (tool result, webhook callback, model response).

**Permission.** A verb used in `authority_scope.permission` and `effect.required_permission`. Examples: `file.read`, `shell.run`, `memory.write`, `tool.call:github.create_issue`.

**Phase.** A delivery slice in `11-roadmap.md`. Phases run sequentially with *decision gates*.

**Policy.** A row in `policy`. A versioned bundle of rules that Guard consults. Its body lives in an artifact; ActantDB stores metadata and a hash.

**Policy snapshot.** The `permission_snapshot_ref` on a *checkpoint*. Used by `mode=policy` replay to compare alternate policies against the original decision path.

**Projection.** A row in any non-ledger table derived from `agent_event`. Updated inside the command transaction so that immediately-following reads see the change.

**Provenance.** The graph linking every *memory* back to the events that justified it, every model call's *context build* to the memories it included, every tool call to the model call that proposed it. Fully traversable.

**Reducer.** SpacetimeDB's term for the equivalent of an ActantDB *command*. Used in `00-overview.md` for the inspiration comparison; not used elsewhere.

**Redaction.** The phase of context build that rewrites included content to remove things that would leak in their current form (API keys, PII, prompt injections).

**Replay.** Re-running prior events to debug, compare, or audit. Modes: `recorded`, `experimental`, `policy`, `model`, `memory`, `tool`, `local_only`.

**Replay diff.** A row in `replay_diff`. One comparison point between original and replayed events: `identical`, `changed`, `missing`, `extra`.

**Replay engine.** The subsystem owning checkpoints and replay runs.

**Replay run.** A row in `replay_run`. One execution of a replay over a checkpoint with a mode and optional overrides.

**Replay mode.** See *Replay*.

**Required permission.** The `permission` an *effect* or *command* consumes. Guard checks it against the actor's *authority scopes*.

**Resource pattern.** The object half of an authority scope (e.g. `~/Projects/**`, `*.example.com`). Interpreted by the subsystem the permission belongs to.

**Risk level.** An `effect`/`tool_call` column. One of `low`, `medium`, `high`, `critical`. Drives the default need for approval.

**Scope.** Three different uses, disambiguated by context:
- *Authority scope*: a permission grant row.
- *Approval scope* (`scope_granted`): `once`, `session`, `scope`, `forever`.
- *Memory scope*: a label on a memory (`global`, `session`, `project`, …).

**Secret ref.** A row in `secret_ref`. Pointer to material in the *Secret Vault*; the material itself never lives in ActantDB.

**Secret Vault.** A *companion store* for secrets. macOS Keychain, 1Password, HashiCorp Vault, cloud KMS.

**Semantic Index.** A *companion store* for vector search. Qdrant, LanceDB, Chroma, FAISS, pgvector, SQLite vector extension.

**Sensitivity.** A label on content: `public` < `low` < `medium` < `high` < `secret` < `regulated`. Compared against an *authority scope*'s `sensitivity_ceiling` and a *model route*'s ceiling.

**Session.** A row in `session`. A conversation between an *initiator* (typically a human) and an *agent*. Bounds many other rows (messages, model calls, tool calls).

**Snapshot (subscription).** The first message a client receives on a new subscription: the current set of matching rows.

**Snapshot (replay).** One of four artifacts on a `replay_checkpoint`: state, model_route, permission, memory.

**Source of truth.** The Chronicle. Every other row in the database is rebuildable from it.

**Studio.** The dashboard product (`actant-studio`). Renders: Approval Center, Memory Review, Context Inspector, Workflow Board, Replay Lab, Audit Trail, etc.

**Subagent.** An *actor* (`kind='subagent'`) with authority delegated from a parent agent and a narrower scope.

**Subscription.** A live, filtered, server-pushed view of a table. Delivered over WebSocket.

**Subscription engine.** The subsystem owning subscriptions: snapshot, incremental delivery, backpressure, reconnection.

**Tamper evidence.** The property of the *Chronicle* that any change to a past event invalidates the chained *event hash*. Verified by periodic checkpoint hashing.

**Tombstone.** An event that marks a prior payload deleted while keeping the audit skeleton intact.

**Tool.** A row in `tool`. A named, versioned external capability with a `required_permission`, `default_risk_level`, and an `input_schema`.

**Tool call.** A row in `tool_call`. One invocation of a *tool* by an agent. Subject to Guard; usually requires approval at `risk_level >= high`.

**Trigger.** A row in `trigger`. Causes a *workflow run* to start: `cron`, `event`, `webhook`, `manual`.

**Visibility.** A set of tags on content that says where it may go: `local_model_allowed`, `cloud_model_allowed`, `human_only`, `never_model`, `never_sync`. Compared against a *model route*'s `visibility_required` set.

**Worker.** An *actor* (`kind='worker'`) that claims and executes *effects*. Reference workers: shell, file, browser, model, MCP.

**Worker capability.** A row in `worker_capability`. Declares which *effect types* a worker can claim.

**Workflow.** A row in `workflow`. A versioned DAG of nodes and edges. Workflows are executed as *workflow runs*.

**Workflow run.** A row in `workflow_run`. One execution of a workflow. Survives process restarts; advances through node states; gates on approval.

**Workspace.** A row in `workspace`. The unit of tenancy, policy scope, and audit. Every other row except `workspace` itself carries a `workspace_id`.

---

## Extended primitives (Phase 2+)

Defined in `/specs/13-actant-contract.md` and `/specs/14-extended-primitives.md`.

**Actant Contract.** The eight obligations every operation must answer: actor, intent, context, authority, action, observation, memory, replay. A subsystem, command, or product feature that cannot satisfy them is structurally rejected.

**Autonomy drift.** The condition where an actant's behavior departs from its declared intent. Quantified by a `drift_signal` score from intent-mismatch, resource-mismatch, risk-escalation, and unusual-sequence components. Crossing the workspace `drift_threshold` produces an intervention.

**Behavioral trust.** Operational-signal-derived `trust_profile` per `(actor, capability_area)`. May escalate Guard's `risk_level` but never grants additional authority.

**Bounded autonomy.** The system property that emerges from per-actor `budget` rows: an agent must halt or escalate when budgets are exhausted.

**Budget.** A row in `budget`. Bounds an actor / session / workflow_run / delegation on `tokens`, `cost_usd`, `time_seconds`, `tool_calls`, `risk_score`, `approvals`, `memory_writes`, `file_writes`, or `external_messages`.

**Causal DAG.** The structure of the Chronicle when `agent_event.causal_parent_ids` is populated — events have potentially many causal parents, not one. Distinguished from the simpler `parent_event_id` chain.

**Capsule.** A row in `capsule`. A policy bundle (sensitivity, visibility, sync, retention, cloud-allowed, memory-allowed) that travels with content. Anything derived from a capsule-bound object inherits the capsule's policy.

**Capsule membership.** A row in `capsule_membership` binding an object (memory, context_item, artifact, observation, message) to a capsule. The basis of *sensitivity lineage*.

**Compensation plan.** A row in `compensation_plan` attached to an effect with `undo_capability != 'irreversible'`. Captures the pre-state artifact and the compensation effect type that would undo it.

**Context debt.** A score `[0.0, 1.0]` per `context_build` measuring reliance on stale, low-confidence, summarized, or under-provenance context. Surfaced as a `context_debt` row.

**Delegation.** A row in `delegation`. Explicit, scoped authority transfer from a parent actor to a child (subagent). The child's effective authority during the delegation is the union of its own scopes plus the delegated subset.

**Drift signal.** A row in `drift_signal`. Captures a score and component breakdown for an actant's recent behavior, plus the intervention (if any) that resulted.

**Eval case.** A row in `eval_case`. A reusable test minted from a replay or a regret, with success/forbidden criteria and a checkpoint. Runs from then on.

**Eval run.** A row in `eval_run`. One execution of an eval case, optionally linked to a `replay_run` that produced its inputs.

**Evidence-backed memory.** Memory whose `source_event_ids` include `observation` rows, not just message events. Lets the system answer "why do you believe this?" with a concrete observation.

**Intent.** A row in `intent`. The agent's declared goal + proposed action class + resource targets + expected risk, formed before a tool call. Guard's evaluation includes an intent–action alignment check.

**Intent–action alignment.** The check Guard performs against a proposed effect to verify it falls within the open `intent`'s declared scope. Failure produces `intent_action_mismatch`.

**Intervention.** A row in `intervention`. A first-class human steering action: pause, cancel, redirect, edit_context, deny_tool, change_model, add_instruction, restrict_permission, fork_replay, take_over_node. Interventions enter the causal graph.

**Memory conflict.** A row in `memory_conflict`. Two approved memories whose content contradicts; carries a `resolution_policy` the context engine consults before retrieval.

**Model route decision.** A row in `model_route_decision`. Records why a specific `model_route` was selected for a specific `model_call`, including candidates considered, privacy constraints, and fallbacks.

**Observation.** A row in `observation`. Structured evidence produced by an effect, with `evidence_type`, `confidence`, `verification_status`, optional `capsule_id`. Memory candidates reference observations to back beliefs with evidence.

**Regret event.** A row in `regret_event`. A captured bad outcome with causal event IDs and a suggested corrective action. Must be resolved by linking to a concrete change (policy, memory demotion, eval case, tool-schema update).

**Sensitivity lineage.** The property that sensitivity (and other capsule policy) flows through derivations. Realized by `capsule_membership` rows attached to derived objects, with strictest-wins composition when sources differ.

**Session phase.** The `session.phase` column. An explicit enum representing the agent's current state: `idle`, `planning`, `building_context`, `calling_model`, `awaiting_approval`, `executing_tool`, `observing`, `reflecting`, `writing_memory`, `blocked`, `completed`, `failed`, `cancelled`.

**Trust profile.** A row in `trust_profile`. Behavior-derived score per `(actor, capability_area)`. Advisory only — escalates Guard's scrutiny but does not grant authority.

---

## AI-native + reliability primitives (Phase 1+)

Defined in `/specs/15-actant-index.md`, `/specs/16-protocols.md`, `/specs/17-observability.md`, `/specs/18-reliability-primitives.md`.

**ActantIndex.** The retrieval subsystem. Owns hybrid dense+sparse+graph retrieval, reranking, semantic chunking, context packing, retrieval traces, retrieval evals.

**Adaptive rate limiting.** Ingests `RateLimit-*` headers from upstream providers and updates `rate_limit_state` so the next request knows to wait.

**A2A.** Agent-to-Agent protocol. ActantDB records peers (`a2a_card`), interactions (`a2a_interaction`), and gates outbound delegations through `delegation` rows + authority scopes.

**AP2.** Agent Payments Protocol. ActantDB records mandates (`ap2_mandate`), intents (`ap2_intent`), and transactions (`ap2_transaction`). Does NOT process payments.

**ActantThrottle.** Multi-axis rate limiter with token bucket / leaky bucket / fixed window / sliding window / concurrency / weighted-fair-queue algorithms.

**ActantCircuit.** Circuit breaker registry per `(workspace, dependency_key)`. States: `closed | open | half_open | degraded`. Drives `actant-models` fallback.

**ActantLock.** Lease-bounded resource locks (`lock:file:<path>`, `lock:ticket:<id>`, etc.).

**ActantIngress.** External event ingestion: webhooks, email, calendar, filesystem, MCP resources, A2A messages.

**Cache entry.** A row in `cache_entry`. Sensitivity-aware: secret-class never cached, high-class local-only, cross-actor reuse forbidden.

**Capsule policy on retrieval.** The rule that retrieval results inherit their source chunk's capsule policy; a `local_only` chunk cannot enter a cloud-route retrieval result.

**Chunk.** A row in `index_chunk`. The unit of embedding. Modes: fixed-token, semantic, AST-code, markdown-section, PDF-layout, conversation-turn, tool-result, workflow-step, memory-evidence, multimodal-frame.

**Compatible embedding space.** A row in `embedding_space` declaring that two models produce interchangeable vectors (e.g. Voyage 4 family). Cross-space queries are rejected unless this declaration exists.

**Dead-letter item.** A row in `dead_letter_item`. An effect that exhausted retries; convertible to an `eval_case` via `actant dlq convert-to-eval`.

**Effect lease.** A row in `effect_claim`. Input-hash-bound mandate that a specific worker may execute a specific effect under a specific sandbox profile.

**Embedder.** A model that produces a vector from text/image/audio. Registered in `actant-embedders`.

**Entity / Entity relation.** Rows in `entity` and `entity_relation`. The graph layer of ActantIndex.

**Failure-to-eval.** The pipeline that converts a `dead_letter_item` into an `eval_case`: `actant dlq convert-to-eval <id>`.

**Hybrid retrieval.** Default retrieval mode: dense + sparse + reranker. See ADR-0012.

**Idempotency record.** A row in `idempotency_record`. Universal — every command + effect.

**Ingress event.** A row in `ingress_event`. A verified external event awaiting trigger dispatch.

**Local-first embedder.** An embedder that runs on-device (FastEmbed, MLX, llama.cpp). Default per ADR-0014.

**MCP server / resource / prompt.** Records in `mcp_server`, `mcp_resource`, `mcp_prompt`. MCP tools register as ordinary `tool` rows with `kind='mcp'`.

**Model registry.** A row in `model_registry`. Per-model capability metadata used by the router.

**Model route decision.** A row in `model_route_decision`. Records why a specific route was chosen for a specific `model_call`.

**Multivector ref.** A row in `multivector_ref`. Late-interaction (ColBERT-class) retrieval vectors per chunk.

**OpenInference span kind.** The attribute used by trace consumers (Phoenix/Arize/LangSmith) to classify a span. ActantDB sets it on every span per ADR-0015.

**OTel GenAI conventions.** OpenTelemetry's semantic conventions for generative-AI spans, metrics, and events. ActantDB emits compatible spans per ADR-0015.

**Prompt template / version.** Rows in `prompt_template` and `prompt_version`. Versioned artifacts that replay re-renders for any prior model call.

**Rate limit policy / state.** Rows in `rate_limit_policy` and `rate_limit_state`. The configuration and runtime state of a single throttle rule.

**Reranker.** A model that scores candidate documents against a query directly. Local cross-encoder default; Cohere / Voyage / Jina available. Always emits a `reason_selected`.

**Retrieval trace / candidate.** Rows in `retrieval_trace` and `retrieval_candidate`. Inspectable record of every retrieval: included, blocked, why.

**Retry policy.** A row in `retry_policy`. Declarative retry configuration referenced by effects and workflow nodes.

**Sparse encoder.** An encoder that produces a sparse vector (BM25, SPLADE-v3, BM42).

**Universal idempotency.** ADR-0017 — every command and effect carries an idempotency key.

---

## Performance discipline (Phase 1+)

Defined in `/specs/19-performance-architecture.md`. ADRs 0018-0020.

**Admission control.** The kernel's policy of refusing expensive work on the hot path (e.g. deep retrieval, graph expansion, compliance trace) and redirecting it to async lanes. Profiles in `/planning/performance-budgets.md`.

**Async lane.** A subscriber to the chronicle that does asynchronous work and writes back through commands. Lane catalog: `/planning/lane-catalog.md`. ADR-0018.

**Capability token.** A compact compiled representation of an actor's authority, refreshed at session/workflow start. Hot-path authority checks read the token in microseconds rather than re-evaluating policy.

**Compiled policy decision DAG.** Policy bundles compile into a decision graph indexed by `(effect_type, actor_kind, resource_kind, sensitivity, workflow_mode, approval_mode)`. Lookup is array-indexed, not string-matched.

**Deployment mode.** A workspace-level flag (`local-fast`, `developer`, `team`, `enterprise`, `regulated`, `research`) controlling which async lanes are enabled. ADR-0020.

**Freshness target.** A per-lane latency goal measured between trigger event and lane completion. Reported as the `lane_freshness_seconds` metric.

**Hot kernel.** `actant-command::kernel`. The only module that runs synchronously in the command path. ADR-0018.

**Hot projection tier (L0).** In-memory cache of active sessions, pending approvals, leases, budgets, worker health, capability tokens, rate-limit/circuit state, hot memory shortlists. WAL-backed and reconstructable from the event log.

**Latency budget.** The p50 / p99 wall-clock target for a hot-path operation. Enforced in CI by the `bench/` harness. Catalog: `/planning/performance-budgets.md`.

**Lane.** Short for *async lane*.

**Progressive enrichment.** ADR-0019 — write minimal facts immediately; lanes fill in richness later through enrichment fields with `lane_status`.

**Retrieval profile.** Admission-controlled retrieval configuration (`fast`, `balanced`, `deep`, `local_private`, `degraded`) chosen per request by `actant-memory::index::plan` based on latency goal, sensitivity, budget, complexity, and backpressure.

**Storage tier (L0-L5).** L0 in-memory hot projections, L1 event log WAL, L2 projection tables, L3 semantic/sparse indexes, L4 artifact/blob store, L5 cold archive.

---

## Verification

- [ ] Every term used in another spec file appears here.
- [ ] Every term here is used at least once outside this file (so the glossary doesn't drift from the specs).
- [ ] Definitions reference rows or tables by their exact names from `02-data-model.sql`.
