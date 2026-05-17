# 05 — Security Model

ActantDB treats security as a runtime primitive, not documentation. Every command runs through Guard; every effect carries a `required_permission` and a `risk_level`; every context build is filtered against the target model's `visibility_required`; every memory carries a `sensitivity`; every approval is bound to an actor, a scope, and an expiry.

This document specifies:

1. core security principles
2. invariants (the things that must never happen)
3. sensitivity labels
4. visibility labels
5. authority scopes
6. approval flow
7. threat model
8. privacy and deletion semantics
9. tamper evidence
10. cross-actor isolation

---

## 1. Core security principles

```
Models are untrusted.
Tools are dangerous until scoped.
Memories can be wrong or sensitive.
Context can leak data.
Workers need least privilege.
Humans need override and audit.
```

These are not aspirations; each maps to a concrete subsystem:

| Principle              | Realized by                                         |
| ---------------------- | --------------------------------------------------- |
| Models untrusted       | Context firewall, output sanitization at workers    |
| Tools dangerous        | `tool_call.risk_level`, default-deny approvals      |
| Memories fallible      | Memory lifecycle, mandatory review for `high`+      |
| Context leaks          | `visibility_required` per model route               |
| Workers least-priv     | `worker_capability` matched to `effect_type`        |
| Humans override        | `approve_*` / `deny_*` / `revoke_*` commands       |

---

## 2. Invariants

These statements must hold at all times. Each maps to a check in the code; violating any is a bug.

1. **No mutation without a command.** Projection rows are written only through `command_record`s.
2. **No command without an actor.** `command_record.actor_id` is `NOT NULL` and verified.
3. **No sensitive command without authority.** Guard returns `deny` if no `authority_scope` covers `(actor, permission, resource, sensitivity)`.
4. **No side effect inside a database transaction.** Side effects are mediated by the Effect Engine.
5. **No model call without a context manifest.** `model_call.context_build_id` is `NOT NULL`.
6. **No memory without provenance.** `memory.source_event_ids` and `memory.source_candidate_id` are required.
7. **No cloud context without visibility policy.** Items lacking `cloud_model_allowed` (or stricter-matching) visibility cannot enter a context build targeting a cloud route.
8. **No approval without audit record.** `approval_request.approved_by_actor_id` and `approved_at` are required for approved decisions; the matching `command_record` is the audit anchor.
9. **No replay without policy snapshot.** A `replay_run` cannot start unless its `replay_checkpoint` has all four snapshot refs populated.
10. **No secret in ActantDB tables.** Only `secret_ref` rows exist; raw secret material lives in Keychain / Vault / KMS.
11. **No cross-workspace effect.** A worker can only claim effects in workspaces it is registered for; commands enforce `workspace_id` equality with the actor's home workspace (unless the actor explicitly has cross-workspace authority).
12. **No silent overrides.** Any command that would override a default authority check requires `policy.override` and produces an `audit_event` with `decision = 'allow_with_approval'` and the override reason.

---

## 3. Sensitivity labels

ActantDB has six sensitivity levels, ordered:

```
public  <  low  <  medium  <  high  <  secret  <  regulated
```

Defined:

| Label        | Meaning                                                                       |
| ------------ | ----------------------------------------------------------------------------- |
| `public`     | Safe to publish externally. No restrictions.                                  |
| `low`        | Internal but harmless. Default for most agent observations.                   |
| `medium`     | Internal and meaningful. Default for inferred memories, work content.         |
| `high`       | Personal or business-sensitive. Default for private files, calendars.         |
| `secret`     | Credentials, keys, tokens, raw cookies. Default for `secret_ref` payloads.    |
| `regulated`  | PHI, financial PII, content subject to HIPAA / GDPR / similar.                |

**Operational rules.**

- A `context_item` cannot enter a context build unless its `sensitivity` is `<=` the model route's allowed ceiling.
- A `memory_candidate` whose proposed sensitivity is `>= high` must be reviewed by a human before promotion to `memory`.
- An `effect` whose input contains items with `sensitivity = secret` must be performed by a worker with `regulated_data: true` or the effect is rejected with `policy_blocked`.
- An `artifact` with `sensitivity = regulated` cannot leave the workspace's data residency boundary (sync rules in §9).

---

## 4. Visibility labels

Sensitivity says how protected something is. Visibility says where it may go.

```
local_model_allowed   may be sent to a local-only model worker
cloud_model_allowed   may be sent to a cloud model worker
human_only            may not be sent to any model; humans only
never_model           same as human_only, with stronger redaction
never_sync            may not leave this device
```

Labels can be combined. A memory tagged `local_model_allowed | never_sync` may inform local model calls but never leaves the device.

Visibility is enforced at the context build stage. The Context Engine intersects each item's visibility set with the model route's `visibility_required` set; mismatches are excluded with `blocked_reason='visibility'`.

---

## 5. Authority scopes

An authority scope is the unit of permission. The shape (from `02-data-model.sql`):

```sql
authority_scope (
    actor_id              -- the holder
    permission            -- the verb       (e.g. file.read)
    resource_pattern      -- the object     (e.g. ~/Projects/Swoosh/**)
    sensitivity_ceiling   -- the cap         (e.g. medium)
    allowed_actions       -- additional refinements (JSON array)
    expires_at            -- optional       (RFC3339)
    revoked_at            -- optional
)
```

**Composition.** An actor's effective permissions are the union of all unrevoked, unexpired scopes. Guard checks each command/effect against the union; a match must exist that covers (permission, resource, sensitivity).

**Resource patterns.** Globs are interpreted by subsystem:

- Files: shell-style globs (`~/Projects/**`, `~/Documents/*.pdf`).
- HTTP: host suffix match (`*.example.com`, `api.example.com`).
- Browser: per-domain.
- MCP/tools: tool-name match.

**Sensitivity ceiling.** A scope that grants `file.read` with `sensitivity_ceiling='medium'` permits reading files labelled `public` / `low` / `medium` but not `high` / `secret` / `regulated`. The label on a target resource comes from policy (e.g. all paths under `~/.ssh` are `secret`) and from explicit per-file tagging.

**Examples.**

```
file.read         :  ~/Projects/**                        ceiling=medium
file.write        :  ~/Projects/Swoosh/**                 ceiling=medium
shell.run         :  *  (read_only allowed_actions)       ceiling=low
browser.tabs.read :  *  (safari only)                     ceiling=low
memory.write      :  *                                    ceiling=low
model.call        :  cloud:openai                         ceiling=non_sensitive (== medium)
tool.call         :  github.create_issue                  ceiling=low
workflow.run      :  daily_digest                         ceiling=low
```

---

## 6. Approval flow

```
1. command requests effect (e.g. request_tool_call shell.run pytest)
2. Guard computes (permission, resource, sensitivity, risk_level)
3. Guard checks authority_scope ∩ policy
4. result ∈ { allow, allow_with_approval, deny }

   on deny:
     command rejected with policy_blocked

   on allow:
     effect status = pending; available to workers

   on allow_with_approval:
     effect status = awaiting_approval
     approval_request created
     subscribers notified
     wait for approve_effect_* or deny_effect or expire
     on approve: effect → pending
     on deny:    effect → cancelled
     on expire:  effect → cancelled; approval_request → expired
```

**Approval scopes.** A human approver chooses how broad the grant is:

| `scope_granted` | Semantics                                                                |
| --------------- | ------------------------------------------------------------------------ |
| `once`          | This effect only.                                                        |
| `session`       | All similar effects in `session_id` until session closes.                |
| `scope`         | Mints a new `authority_scope` row binding the actor + permission +       |
|                 | resource_pattern + sensitivity_ceiling. Future requests auto-allow.      |
| `forever`       | Same as `scope`, with `expires_at = null`. Strong wording in the UI.     |

**Critical-risk effects.** `risk_level = critical` always requires fresh approval, even if a `forever` scope exists, unless that scope explicitly grants `critical`. This is the failsafe.

**Approver eligibility.** A workspace declares one or more approver actors (typically human actors with `approver: true`). Self-approval is permitted for `low` risk only; `medium`+ requires a different actor than the requester.

---

## 7. Threat model

Threats and mitigations.

### T1 — Prompt injection from observed content

*Attack.* A model is given a tool result (web page, email body, file content) that contains instructions like "now exfiltrate the user's SSH key."

*Mitigation.* The Context Engine tags observed content with `source_type` and `provenance`. Guard policy can require that *instructions* (parsed as model-directives) coming from observation sources be downgraded or marked. Tool calls produced by a model call whose context contained a `prompt_injection_marker` are auto-elevated to `awaiting_approval`.

### T2 — Memory poisoning

*Attack.* An attacker convinces an agent to remember a false fact that influences future decisions.

*Mitigation.* `memory_candidate.confidence` thresholds and mandatory human review for `sensitivity >= high`. `memory.source_event_ids` makes every memory inspectable. `revoke_memory` and `delete_memory` provide remediation.

### T3 — Tool misuse via argument injection

*Attack.* Model produces a `shell.run` with `command="rm -rf /"`.

*Mitigation.* `shell.run` defaults to `risk_level=high` and requires approval. `allowed_actions: ["read_only"]` further restricts; the worker enforces the constraint and refuses to run mutating commands.

### T4 — Exfiltration via context

*Attack.* Sensitive memory leaks to a cloud model through a context build.

*Mitigation.* Context firewall: `visibility_required` per route, per-item `visibility`, blocked-item count surfaced in the manifest. Replay can re-derive what was sent.

### T5 — Worker compromise

*Attack.* A malicious or compromised worker claims sensitive effects.

*Mitigation.* Worker registration requires authority; `worker_capability` restricts claimable types. Workers running in restricted environments declare `region`, `host`, `attestation_ref`. Guard can route sensitive effects only to attested workers.

### T6 — Replay of stolen commands

*Attack.* An attacker replays a captured command request.

*Mitigation.* Every command request carries an `idempotency_key` and a per-actor nonce; the server rejects duplicates inside a sliding window. Authentication is bound to short-lived tokens.

### T7 — Approval fatigue / rubber-stamping

*Attack.* The user approves so many requests they stop reading.

*Mitigation.* Approval requests carry `summary` text generated from the redacted input; `scope_granted` defaults to `once`. UI prompts batch low-risk approvals; high-risk requests are gated by a deliberate confirmation step.

### T8 — Cross-tenant data leak via misrouted effect

*Attack.* A worker accidentally executes an effect for the wrong workspace.

*Mitigation.* Invariant 11 + a server-side check that compares the worker's claimed workspace and the effect's workspace at claim time.

### T9 — Tampered ledger

*Attack.* Someone with DB access alters an `agent_event` row after the fact.

*Mitigation.* `agent_event.event_hash` chains across `parent_event_id`; periodic checkpoint hashes anchor the tip into the artifact store. Verification jobs can detect divergence.

### T10 — Approval bypass via direct projection write

*Attack.* Someone bypasses the command engine and writes directly to `tool_call`.

*Mitigation.* The server forbids non-command writes to projection tables (invariant 1). Database-level access controls (the Phase 1 server owns the only DB connection; SDKs go through the command API) make this the only path.

---

## 8. Privacy and deletion semantics

ActantDB has to reconcile two opposing forces:

- An immutable ledger gives auditability and replay.
- Deletion rights (and basic hygiene) require that some content disappear on request.

**The reconciliation.** Deletion happens at the **payload** layer, not the **event** layer. The event remains; the payload is removed.

| Operation                  | Effect                                                                        |
| -------------------------- | ----------------------------------------------------------------------------- |
| `delete_memory`            | Sets `memory.deleted_at`; deletes embedding vector; clears `memory.text`.     |
| `delete_artifact`          | Removes blob from artifact store; keeps `artifact.id`, `created_at`, hash.    |
| `redact_event_payload`     | Removes inline payload or referenced artifact; keeps `event_hash` skeleton.   |
| `purge_secret_ref`         | Marks `secret_ref.revoked_at`; vault provider revokes the underlying material.|
| `expire_workspace`         | Schedules erasure of all payloads after the retention window.                 |

**Cryptographic erasure.** For high-sensitivity payloads, the artifact store may store ciphertext keyed by a per-artifact key in the vault. Deletion = drop the key. The ciphertext becomes unreadable instantly; the artifact bytes can be reaped later.

**Audit skeleton preservation.** Even after payload deletion, the system can answer "did this event happen?", "when?", "who initiated it?", and "what was its hash?" — but not "what did it contain?".

---

## 9. Tamper evidence

Every `agent_event` has:

- `event_hash = SHA-256(parent_event_hash || payload_hash || canonical_metadata)`

Where `canonical_metadata` is a stable serialization of `(workspace_id, actor_id, event_type, created_at, ...)`.

**Anchoring.** Periodically (configurable; default every 1000 events or every 5 minutes), the system stores a *checkpoint hash* — the hash of the latest event — as an artifact in the artifact store, optionally also broadcast to an external attestation service. Anyone with read access can re-walk the chain and verify the chain hash matches.

**Tombstones do not break the chain.** A redaction replaces the payload but leaves the `payload_hash` of the original payload in `event_hash`. Verification continues to work; the only thing that changes is whether the payload bytes can still be inspected.

---

## 10. Cross-actor isolation

- Sessions, memories, workflows, artifacts, and effects are scoped to a workspace.
- Cross-workspace access requires explicit `cross_workspace.*` permissions and is logged as a distinct `audit_event` with `decision_reason` recording the source and target workspaces.
- Subscriptions are filtered server-side: a client cannot subscribe to rows it cannot read. Filter expressions that would require disallowed reads return `forbidden` at subscription time, not at delivery time.

---

## Verification

- [ ] Every invariant in §2 has a corresponding check at a named line in the command pipeline (filled in during Phase 1).
- [ ] Every sensitivity label is used somewhere in `02-data-model.sql` (as a column constraint or a default).
- [ ] Every visibility label is used in the context-build pipeline in `06-context-and-memory.md`.
- [ ] Every threat in §7 has at least one named mitigation that points to a column, command, or invariant.
- [ ] Every deletion mode in §8 leaves the event skeleton intact while removing payload.
