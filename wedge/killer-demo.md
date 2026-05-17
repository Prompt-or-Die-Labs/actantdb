# Killer demo

One demo. One story. It defines the wedge.

## Title

**Why did this agent delete the wrong file?**

## Setup

A Mastra "test-cleanup agent" wired through `@actant/mastra`. The agent's job is to delete build artifacts after `pytest` passes. It has access to `shell.run` and `file.write`. It's running on a project the demo viewer can see.

The agent's recent memory contains one stale fact:

```
"This project's build artifacts live under both /build and /dist."
```

The reality is `/build` is build artifacts; `/dist` is the **release artifacts**. The memory is wrong but plausible.

## The run (live, recorded, replayable)

1. The user asks: "Clean up the test artifacts."
2. The Mastra agent retrieves the stale memory + context.
3. It proposes `shell.run` with `rm -rf build dist`.
4. **Actant Guard** classifies the command as destructive (regex match on `rm -rf` + the path includes `/dist`).
5. Verdict: `require_approval` with a constrain hint: "drop `dist/`?"
6. The viewer approves the **constrained variant** `rm -rf build` from Studio. The constrain rewrite is recorded.
7. The shell worker executes the constrained command. The agent continues. Tests still fail because the original task description was ambiguous — the agent now wants to also delete `node_modules/.cache`. It proposes that.
8. Approved. Executed. The agent reports completion. The demo viewer sees the run "succeed" — but it succeeded with a constrained command path; the original proposal would have nuked `/dist`.

## Open Studio

The viewer scrolls the timeline:

```
14:02:01  user_message_received          "Clean up the test artifacts."
14:02:02  context_build                  3 items included, 0 blocked
                                          ↳ memory: "build artifacts live under /build and /dist"  ⚠️
14:02:03  model_call_finished            planner: shell.run "rm -rf build dist"
14:02:03  tool_call_requested            shell.run    risk: high
14:02:03  guard_verdict                  require_approval, constrain hint: "drop dist/"
14:02:08  approval_decision              approver: wes, scope: once, constrained variant accepted
14:02:08  tool_call_started              shell.run "rm -rf build"      ◀ constrained
14:02:09  effect_completed               exit=0
14:02:09  observation                    "Removed 14 files."
14:02:10  ...
```

Click the **context_build** row. The viewer sees the manifest — three included items, the stale memory ⚠️ flagged because Actant's heuristic noticed `/dist` appearing in both memory text and the proposed command argument.

Click the **guard_verdict** row. The viewer sees the exact policy that fired, the constrain hint logic, and the audit-event ID.

## The replay

Click **Replay** on the `model_call_finished` row.

```
Replay options:
  ☐ same policy            ✓ stricter policy: "no shell.run without explicit dist guard"
  ☐ same memory set        ✓ exclude memory: mem_42 ("/build and /dist")
  ☐ same model             ☐ alternate model
```

Run.

Studio renders a side-by-side diff:

```
event              original (recorded)            replay
─────────────────────────────────────────────────────────────────────
context_build      3 included, 0 blocked           2 included, 1 blocked  ◀ mem_42 blocked
model_call         "rm -rf build dist"             "rm -rf build"
guard_verdict      require_approval (constrain)    allow
tool_call          shell.run "rm -rf build"        shell.run "rm -rf build"
effect_completed   exit=0                          exit=0  (recorded reuse)
```

The punchline appears as a callout:

> Without `mem_42`, the planner would have proposed the safe command directly. The memory caused the risky proposal; Guard caught it; replay proves the causal link.

The user clicks "Demote memory mem_42" — Studio writes a memory event into the local store; the agent's future runs won't pull it.

## What this proves

- **Guard authority** is not a trace; it intercepts and rewrites.
- **Chronicle replay** is not logs; it reruns the planner under different conditions.
- **Context manifests** make the model's input inspectable.
- **Memory provenance** lets a single bad memory be traced from observation to action.

None of these are services running in some cloud. They run inside the developer's `actant studio` against `@actant/mastra`-wrapped agents.

## Demo deliverables

By 2026-06-30:

- A repo `actant-demo-test-cleanup` that scaffolds the agent.
- A 90-second screen recording of the run + Studio + replay.
- A README that walks through it in ≤ 200 words.
- A one-screen image of the diff for the homepage hero.

The demo must run locally on the viewer's laptop in under 5 minutes from `git clone`. If it doesn't, the demo is broken and we keep fixing until it does.

## What the demo is NOT

- It is not a multi-agent orchestration showcase.
- It is not a vector retrieval showcase.
- It is not a Mastra replacement showcase. The agent is a stock Mastra agent.
- It is not a "look at our architecture" showcase. The architecture is invisible; the value is visible.
