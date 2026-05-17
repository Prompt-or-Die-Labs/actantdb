# 10 — Alpha Demo

The alpha demo is the smallest end-to-end scenario that proves ActantDB's core thesis: **every autonomous action becomes a governed, replayable event.**

It is a **coding agent** that fixes failing tests in a Python repo. By the end of the demo, the system has exercised:

- live state (subscriptions)
- tool approval (the approval center)
- the effect queue (model + shell + file workers)
- the context manifest (what the model saw)
- memory provenance (a memory candidate, approved by the user)
- the audit trail (every step in the Chronicle)
- replay (rerun the run with a different model)

If a stakeholder watches this demo and the surfaces above are visible and inspectable, Phase 1 has shipped.

---

## 1. Pre-conditions

A laptop with:

- `actantdb` server running locally (SQLite backend).
- One agent actor registered: `agent_coder`.
- One human actor registered: the user (`human_wes`).
- Three workers registered and online: `model-worker-01`, `shell-worker-01`, `file-worker-01`.
- One model route configured: `route_planner` → an Anthropic or OpenAI model (cloud), with `visibility_required = cloud_model_allowed`, `sensitivity_ceiling = medium`.
- A small Python repo cloned at `~/Projects/demo_repo` with a deliberately failing test in `tests/test_math.py`.
- The user has granted these scopes to `agent_coder`:

```
file.read   : ~/Projects/demo_repo/**           ceiling=medium
file.write  : ~/Projects/demo_repo/**           ceiling=medium
shell.run   : *                                  ceiling=low   (allowed_actions=["read_only"])
model.call  : route_planner                      ceiling=medium
```

Note: `shell.run` is intentionally `read_only` at scope time. Running `pytest` is read-only; later, the agent will need to edit a file, which is `file.write` (already granted) — but it will *also* request a *re-run* of `pytest` after editing, which is still read-only. The approval prompts in the demo exercise the *risk-level* path (high) rather than the *scope ceiling* path; tunable in policy.

---

## 2. The user's prompt

```
> Fix the failing tests in this repo.
```

Sent via the CLI or the Studio chat surface.

```
client.command.append_user_message({
  session_id: "<new>",
  text: "Fix the failing tests in this repo."
})
```

---

## 3. The agent builds context

The agent issues:

```
client.command.build_context({
  session_id: "sess_demo",
  purpose: "planner",
  model_route_id: "route_planner",
  token_budget: 8000
})
```

Studio's **Context Inspector** updates live. The user sees:

- 4 included items: system prompt, the user message, two memories ("user prefers pytest", "repo uses Python 3.12").
- 1 blocked item: a `~/.ssh/config` file that pattern-matched on a recent `file.read` but is `sensitivity=secret` → `blocked_reason='sensitivity'`.

This screen is the first proof of the context firewall.

---

## 4. First model call

```
client.command.request_model_call({
  session_id: "sess_demo",
  context_build_id: "ctx_001",
  route_id: "route_planner",
  purpose: "planner"
})
```

- `model_call` row appears with `status=requested`.
- An `effect` of type `model.call` is enqueued.
- `model-worker-01` claims the effect, calls the cloud model, receives a plan: "1. run pytest to see failures. 2. inspect failing test. 3. edit code. 4. re-run."
- Worker calls `complete_effect`, which chains to `record_model_result`.
- `model_call.status=completed`, latency and cost recorded.

The **Live Sessions** dashboard shows the model call's latency, cost, and token counts in real time.

---

## 5. The agent proposes its first tool call

The planner output includes a tool call: `shell.run` with `command="pytest -q"`.

```
client.command.request_tool_call({
  session_id: "sess_demo",
  tool_name: "shell.run",
  arguments: { command: "pytest -q" }
})
```

Guard evaluates:

- `actor=agent_coder` has `shell.run` (ceiling=low, allowed_actions=["read_only"]).
- `pytest -q` is read-only ✓.
- But `tool.default_risk_level = high`. Policy says `risk_level >= high` requires approval unless waived per session.

**Result.** `tool_call.status = pending_approval` and an `approval_request` row is created.

---

## 6. Live approval

The **Approval Center** in Studio updates over the WebSocket subscription. The user sees:

```
Approval requested by agent_coder
  shell.run  command="pytest -q"
  Risk: high
  Granted scope choice: [Once] [Session] [Forever]
  Required permission: shell.run
```

The user clicks **Session** (allows all `shell.run` in this session without re-prompting; the SDK records this as a session-scoped approval).

```
client.command.approve_tool_call({
  tool_call_id: "tc_001",
  scope: "session"
})
```

- `approval_request.status=approved`.
- An `authority_scope` is created with `expires_at` tied to the session end.
- `tool_call.status = approved`.
- An `effect` of type `tool.call` (sub-kind shell) is enqueued.

---

## 7. Shell worker runs the test

- `shell-worker-01` claims the effect.
- Heartbeats every 5s.
- Runs `pytest -q`. The output:

```
F.                                                       [100%]
=================== FAILURES ===================
test_math.py::test_addition - assert add(2, 3) == 6
================ 1 failed, 1 passed in 0.12s ===
```

- Worker uploads the output as an artifact, calls `complete_effect`.
- Chronicle: `tool_call_finished`. Artifact ref attached.

---

## 8. The agent reads the file

A second model call (now with the test output in context) suggests reading `tests/test_math.py` and the source file `src/math.py`.

Two `request_tool_call` calls follow:

- `file.read` on `tests/test_math.py` — auto-allowed (low risk, scoped).
- `file.read` on `src/math.py` — auto-allowed.

`file-worker-01` performs both reads. Contents become artifacts.

---

## 9. The agent edits the source

A third model call produces a patch. The agent issues:

```
client.command.request_tool_call({
  session_id: "sess_demo",
  tool_name: "file.write",
  arguments: {
    path: "src/math.py",
    contents: "def add(a, b):\n    return a + b\n"
  }
})
```

Guard:

- `file.write` on `~/Projects/demo_repo/**` is granted.
- `default_risk_level = high` (file writes are high-risk).
- Approval required.

The Approval Center shows the diff. The user clicks **Once**, scope `once`.

- `file-worker-01` writes the file.
- Chronicle records before/after hashes; the prior content is stored as an artifact for replay.

---

## 10. The agent reruns the tests

Because the user previously approved `shell.run` for the session, this proceeds without prompting.

```
shell.run  pytest -q
output: 2 passed
```

---

## 11. Memory candidate

The memory extractor (an evaluator model running as a low-risk background effect) sees the sequence and proposes:

```
{
  "text": "Repo at ~/Projects/demo_repo uses pytest; tests live under tests/.",
  "category": "fact",
  "sensitivity": "low",
  "confidence": 0.91,
  "source_event_ids": ["evt_tool_call_finished_pytest_1", "evt_file_read_test_math"]
}
```

Because `sensitivity = low` and `confidence > workspace.auto_threshold`, it would normally auto-approve. The demo workspace is configured to require review for the first 10 memories (a "training wheels" period) so it lands as `pending_review`.

The **Memory Review** screen updates. The user clicks **Approve**. A `memory` row appears, embedding is generated by `memory.embed` effect, `embedding_ref` is recorded.

The user clicks the memory and sees the provenance trail: which events, which tool calls, when each happened. This is the second proof of the core thesis.

---

## 12. Replay

The user goes to **Replay Lab** and:

1. Selects the run.
2. Picks the checkpoint `chk_after_first_model_call`.
3. Picks mode `model` with override `model_route_id = route_qwen_coder_local`.
4. Clicks **Run**.

The replay re-invokes the alternate model worker (local Qwen Coder via MLX). It then:

- Re-uses the recorded shell results (mode `model` only re-invokes models, not tools).
- Compares the resulting plan and edits.

The diff page surfaces:

```
identical: 11 events
changed: 3 events
  - model_call_finished: response_ref differs (Qwen produced a slightly different patch)
  - tool_call_requested: arguments.contents differs (different valid fix)
  - tool_call_finished: same outcome (tests still pass)
extra: 0
missing: 0

Cost summary:
  original (cloud)  : $0.027
  replay (local)    : $0.000
  latency original  : 4.8s
  latency replay    : 9.6s
```

This is the third proof: a sentence like "a local model would have solved it for free in twice the latency" is now a queryable, evidenced statement, not a guess.

---

## 13. What the demo proves

| Subsystem        | Proven by                                                       |
| ---------------- | --------------------------------------------------------------- |
| Chronicle        | Every step has a Chronicle event visible in the Audit Trail.    |
| Command Engine   | The user's actions and the agent's actions all go through commands. |
| Subscription     | Approval Center, Context Inspector, Memory Review update live.  |
| Effect Engine    | Workers claim and complete effects; lease + idempotency visible.|
| Guard            | Approval prompts appear at the right risk levels.               |
| Context Engine   | `~/.ssh/config` blocked; manifest inspectable.                  |
| Memory Engine    | Memory candidate, approval, provenance graph.                   |
| Replay Engine    | `mode=model` replay produces a diff.                            |
| Workflows        | Not exercised in this demo; covered by a second demo in §14.    |

---

## 14. Second demo: daily digest workflow

A complementary demo to prove Flow Engine:

```
trigger: cron 0 7 * * *
nodes:
  fetch_inbox   (tool.call gmail.list_unread)
  fetch_calendar(tool.call calendar.read)
  summarize     (model.call → summary)
  approval_gate (require user click)
  send_message  (message.send to user)
```

Each step appears in the Workflow Board live. The approval gate produces an approval request the next morning when the workflow runs.

---

## 15. Definition of done for Phase 1

- [ ] The coding-agent demo can be executed end-to-end on a fresh machine in under 10 minutes from `actantdb start`.
- [ ] Every Studio screen listed in the demo renders live data.
- [ ] The replay diff is reproducible across machines (within model-determinism limits).
- [ ] No demo step requires a manual SQL query or log inspection.
- [ ] A new developer can read `specs/` and run this demo without help.

---

## Verification

- [ ] Every command invoked in this demo exists in `03-command-spec.md`.
- [ ] Every Studio surface (Approval Center, Context Inspector, Memory Review, Audit Trail, Replay Lab) maps to a subscription target in `08-api-spec.md`.
- [ ] Every approval prompt corresponds to a Guard decision documented in `05-security-model.md`.
- [ ] The replay scenario in §12 only requires snapshots that `replay_checkpoint` columns in `02-data-model.sql` actually capture.
