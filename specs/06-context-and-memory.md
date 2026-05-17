# 06 — Context and Memory

This document is the joint specification of the **Context Engine** and the **Memory Engine**. They are documented together because every model call uses both: the Context Engine builds the manifest of what the model will see; the Memory Engine is one of several sources the Context Engine draws from, and the only one with a full provenance graph.

Sections:

1. The context manifest
2. The context build pipeline
3. The context firewall (visibility enforcement)
4. Redaction
5. Memory lifecycle
6. Memory categories
7. Provenance graph
8. Embedding policy (the Semantic Index companion store)
9. User-facing memory affordances
10. Worked examples

---

## 1. The context manifest

Every model call has exactly one `context_build` row plus N `context_item` rows. Together they constitute the **context manifest** — an inspectable, replayable record of what the model was shown.

`context_build` captures the build-level summary:

```
purpose            'planner' | 'executor' | 'critic' | ...
token_budget       integer
final_prompt_hash  SHA-256 of the canonical prompt the worker sent
blocked_item_count number of candidates the firewall rejected
included_item_count number of items in the prompt
redaction_summary  short text summary of redactions performed
```

`context_item` captures each candidate (included or blocked):

```
source_type        'memory' | 'message' | 'file' | 'artifact'
                   | 'tool_result' | 'workflow_state' | 'system_prompt'
source_id          row id in the source table
included           0 / 1
blocked_reason     enum (see §3) — non-null iff included = 0
sensitivity        public | low | medium | high | secret | regulated
visibility         space-separated visibility tags
rank_score         relevance score (higher = more relevant)
reason_included    short text explaining why selected
token_count        approximate tokens consumed (for included items only)
content_hash       SHA-256 of the canonical text of the item
```

**Why this design.** The manifest is the only way to definitively answer "did the model see X?". Replay uses it to reconstruct the prompt. The redaction summary lets a reviewer understand what was removed without re-exposing the removed content.

---

## 2. The context build pipeline

Triggered by `build_context` (`03-command-spec.md`). Phases:

```
1. Gather candidates
2. Score candidates
3. Firewall filter
4. Redact
5. Truncate to budget
6. Emit rows + finalize
```

### 2.1 Gather

Candidate sources, in priority order:

| Source           | Default selection                                                 |
| ---------------- | ----------------------------------------------------------------- |
| System prompts   | The session's pinned system prompts.                              |
| Recent messages  | Last N messages in the session (default N = 50, configurable).    |
| Memories         | All `active` memories whose `scope` matches the session context.  |
| Files            | Files the session has touched (`tool_call` results referencing files), plus files explicitly attached to the session. |
| Artifacts        | Artifacts referenced in recent messages or tool calls.            |
| Tool results     | Recent tool outputs in the session.                               |
| Workflow state   | If the session is bound to a workflow run, the run's current state. |

The `candidate_filters` field on `build_context` lets the caller narrow or extend this set.

### 2.2 Score

Each candidate gets a `rank_score`. Scoring inputs:

```
semantic similarity to the session's recent intent  (vector search)
recency                                              (decayed weight)
explicit pins                                        (user said "remember this")
prior-use signal                                     (memory_use.outcome)
purpose match                                        (planner vs executor)
```

Scoring is pluggable. Phase 1 ships a simple weighted sum; Phase 2+ supports custom rankers per workspace.

### 2.3 Firewall filter

For each candidate, compute:

```
candidate_visibility  ⊇  route_required_visibility
candidate_sensitivity ≤  route_sensitivity_ceiling
candidate_residency   ⊇  route_residency
```

If any inequality fails, the candidate is blocked. `blocked_reason` is set to one of:

```
visibility           candidate cannot leave its visibility scope
sensitivity          candidate exceeds route's allowed sensitivity
residency            candidate cannot leave its data-residency scope
expired              candidate is past its expires_at
revoked              candidate has been revoked
budget               candidate would exceed the token budget (set in §2.5)
duplicate            another included candidate covers the same content
manual_exclude       the caller's candidate_filters excluded it
```

### 2.4 Redact

Redaction rewrites included candidates to remove things that would leak in their current form even though the candidate is admissible overall. Examples:

- A message that mentions an API key in passing → redact the key, keep the rest.
- A file that contains a phone number → mask the number.
- A tool result that contains a `prompt_injection_marker` → wrap with a delimiter and an instruction to ignore embedded directives.

Redactions are recorded in `redaction_summary` (short text) on `context_build`. The unredacted content is *not* stored back in `context_item`; the `content_hash` reflects the redacted form, so replay reproduces the redacted prompt.

### 2.5 Truncate

After firewall and redaction, sort by `rank_score` desc, accumulate token counts, stop when reaching `token_budget`. Items beyond the budget become `included=0, blocked_reason='budget'` so the manifest still shows what was considered.

System prompts and pinned items are non-truncatable; if they alone exceed the budget the build fails with `precondition_failed`.

### 2.6 Emit

Inside the `build_context` transaction:

```
INSERT context_build (...)
INSERT context_item × N
emit agent_event context_build_finished
```

The build is then ready to attach to a `request_model_call`.

---

## 3. The context firewall

The firewall is the join of three things:

```
The model route's requirements           (where can this content go?)
The candidate's sensitivity & visibility (what does this content require?)
The workspace's policy                   (any overrides?)
```

A route looks like:

```
route_id            'route_planner_cloud'
provider            'openai'
visibility_required 'cloud_model_allowed'
sensitivity_ceiling 'medium'
residency           'us'
```

A candidate looks like:

```
sensitivity  'medium'
visibility   'local_model_allowed cloud_model_allowed'
residency    'us'
```

This candidate passes for this route. A second candidate with `visibility='local_model_allowed'` only would fail with `blocked_reason='visibility'`.

**Why this matters.** Without the firewall, "local-only memories" leak the moment a developer adds a cloud route. With the firewall, leaks are structurally prevented — and if policy is changed, replay can show whether old behavior would now leak.

---

## 4. Redaction

Redaction is **not** the firewall. The firewall decides admissibility. Redaction modifies admissible content to be safer.

Redaction passes (configurable per workspace):

| Pass                | Removes                                                |
| ------------------- | ------------------------------------------------------ |
| `secret_patterns`   | API keys, AWS keys, GitHub tokens, JWTs, RSA blocks.   |
| `pii_patterns`      | Phone numbers, SSNs, credit card numbers.              |
| `email_patterns`    | Email addresses (configurable; off by default).        |
| `path_normalization`| Replaces user-specific paths (`/Users/foo/...`) with `~`. |
| `injection_wrapping`| Wraps tool outputs in delimiters with a system note.    |

Redaction is loss-y. To inspect what was redacted, an authorized reviewer can fetch the original (`source_type`, `source_id`) — which lives in its source table and is not modified by the redaction.

---

## 5. Memory lifecycle

```
observed → candidate → pending_review → approved → active
        → used → superseded → expired → revoked → deleted
```

State definitions:

| State             | Meaning                                                                |
| ----------------- | ---------------------------------------------------------------------- |
| `observed`        | The memory extractor saw something noteworthy. No row yet.             |
| `candidate`       | `memory_candidate` row exists, `status='proposed'`.                    |
| `pending_review`  | Sensitivity required human review; awaiting `approve_memory`.          |
| `approved`        | `memory` row exists; will be considered by future context builds.      |
| `active`          | Equivalent to `approved` for queries; explicit when surfaced in UI.    |
| `used`            | Has at least one `memory_use` row.                                     |
| `superseded`      | A newer memory contradicts this one; supersession is recorded.        |
| `expired`         | `expires_at` has passed; excluded from new context builds.            |
| `revoked`         | `revoked_at` is set; cannot return without re-proposal.               |
| `deleted`         | `deleted_at` is set; text and embedding gone.                          |

Promotion rules:

```
sensitivity in {public, low, medium}    →  proposed → approved (auto, gated by confidence ≥ workspace threshold)
sensitivity in {high, secret, regulated}→  proposed → pending_review (always; human approves or rejects)
```

A workspace can override the auto-approval threshold up or down via policy. The most conservative setting is "all proposals require human review" — useful in enterprise contexts.

---

## 6. Memory categories

`memory.category` is a soft taxonomy that drives UI grouping and per-category retention. Phase 0 set:

```
preference     'User prefers pytest over unittest'
fact           'User's primary editor is Neovim'
goal           'Ship Swoosh alpha by end of June'
constraint     'Never run pip install in this repo'
relationship   'Project Tonic is owned by team Sigma'
pattern        'When fixing a flaky test, also re-run it three times'
profile        'User is left-handed (for ergonomic suggestions)'
opinion        'User dislikes single-letter variable names'
context        'Current sprint is "M5 release"'
```

Workspaces may extend the taxonomy. Each category has a default sensitivity and a default scope.

---

## 7. Provenance graph

The full provenance traversal:

```
agent_event(s)
   → memory_candidate (source_event_ids[])
        → memory (source_candidate_id)
             → memory_use (memory_id × context_build_id × model_call_id)
                  → model_call
                       → tool_call
                            → effect → effect_result
```

Backwards: from any tool call, you can find the model call, the context build, every memory that appeared in the prompt, the candidate that produced each, and the original events that justified each candidate.

This is what makes the user-facing question "why do you remember this?" answerable in O(1) clicks.

---

## 8. Embedding policy

ActantDB does not store vectors. It stores `embedding_ref` rows that point to a vector in a companion store (Qdrant, LanceDB, Chroma, FAISS, pgvector, SQLite vector extension).

**Rules.**

1. **Only approved memories may be embedded.** A `memory_candidate` is not embedded; only an approved `memory` row triggers an embedding.
2. **Sensitivity governs eligibility.** Memories above the workspace's `max_embedding_sensitivity` threshold are not embedded at all (e.g. `regulated` memories must be retrieved exactly, never by similarity).
3. **Visibility governs query scope.** A query against the semantic index passes the route's required visibility; candidates whose `embedding_ref.sensitivity` exceeds the route's ceiling are dropped at retrieve time.
4. **Deletion cascades.** `delete_memory` deletes the embedding; `revoke_memory` removes it from queries but keeps the vector available for forensic inspection by authorized roles, configurable.
5. **Embedding model is recorded.** `embedding_ref.embedding_model` makes it trivial to roll over a model version: re-embed approved memories, swap `embedding_ref.embedding_model`, drop the old vectors.

---

## 9. User-facing memory affordances

Because memory has provenance and a lifecycle, these questions are answerable as direct queries:

| Question                                       | Answered by                                                  |
| ---------------------------------------------- | ------------------------------------------------------------ |
| "Why do you remember this?"                    | `memory.source_event_ids` → events                            |
| "When did you learn this?"                     | `memory_candidate.created_at`                                 |
| "When did you last use this?"                  | `memory.last_used_at` / latest `memory_use.used_at`           |
| "Stop using this in work contexts."            | `restrict_memory` adding `visibility=never_model` for `scope=work` |
| "Never send this to a cloud model."            | `restrict_memory` adding `visibility=local_model_allowed`     |
| "Delete this and its embeddings."              | `delete_memory`                                               |
| "Show me everything you remember about X."     | search over `memory.text` filtered by `category`/`scope`      |
| "What memory caused this tool call?"           | Provenance traversal from `tool_call`                         |

---

## 10. Worked examples

### 10.1 A safe context build

A coding agent is about to call a cloud planner model for a sensitive repo.

Candidates:

```
M1: memory  'User prefers pytest'      sensitivity=low   visibility=local_model_allowed,cloud_model_allowed
M2: memory  'Repo Swoosh uses Convex'  sensitivity=low   visibility=local_model_allowed,cloud_model_allowed
F1: file    'src/api/secrets.ts'       sensitivity=secret visibility=local_model_allowed
T1: tool    'pytest output (passing)'  sensitivity=low   visibility=local_model_allowed,cloud_model_allowed
```

Route requires `cloud_model_allowed` and ceiling `medium`.

Outcome:

- M1, M2, T1 → included.
- F1 → blocked, `blocked_reason='visibility'` (not `cloud_model_allowed`) and `sensitivity` (`secret` > `medium`).

The agent's plan is generated using M1, M2, T1. F1's existence is logged in the manifest but its content stays local.

### 10.2 A memory candidate that requires review

The agent observes the user saying "I have a peanut allergy" during a casual conversation.

Memory extractor proposes:

```
text:        "User has a peanut allergy."
category:    "fact"
sensitivity: "high"           # health-adjacent
confidence:  0.92
```

Because `sensitivity = high`, the candidate moves to `pending_review`. The user gets a subscription update in the approval center: "Add this memory?" with options approve/edit/reject and a checkbox "never send to cloud models".

### 10.3 A memory that should never be used in work contexts

The user has a personal preference (`liked sushi`) they don't want appearing when the agent helps with work documents.

```
restrict_memory({
  memory_id: "mem_42",
  add_visibility: "never_model",
  scope: "work"
})
```

Future `build_context` calls with `purpose` mapped to a work session will exclude this memory. The user can see this restriction in the memory's detail view in Studio.

---

## Verification

- [ ] Every blocked_reason value used in the pipeline appears in §3.
- [ ] Every visibility / sensitivity label is consistent with `05-security-model.md`.
- [ ] Every memory state appears in `02-data-model.sql` (as `memory.deleted_at`, `memory.revoked_at`, `memory.expires_at`, or `memory_candidate.status`).
- [ ] Every provenance traversal in §7 is supported by foreign keys in `02-data-model.sql`.
- [ ] Every user-facing affordance in §9 maps to a command in `03-command-spec.md`.
