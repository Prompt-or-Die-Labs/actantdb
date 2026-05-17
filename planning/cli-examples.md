# Example catalog

`actant examples run <name>` is the fastest way for a developer to see ActantDB work end-to-end. Each example creates a temporary project from a template, runs a scripted interaction, and prints the next useful commands.

Examples are at the repo root under `/examples/<name>/`.

## Examples

### `coding-agent`

The flagship five-minute demo. Mirrors `/specs/10-alpha-demo.md`.

```
actant examples run coding-agent
```

Steps:

1. Spin up a temp project from the `coding-agent` template.
2. Open Studio.
3. User asks: "Fix the failing tests."
4. Agent proposes `pytest -q` → approval requested.
5. Demo prints CLI command to approve from terminal: `actant approval approve appr_001 --once`.
6. Shell worker runs pytest; results streamed back.
7. Agent proposes a file edit → approval with compensation preview.
8. After approval the file is edited and tests re-run.
9. A memory candidate (`This repo uses pytest`) lands; demo prints the approve command.
10. Demo prints a replay command (`actant replay run --from-event ... --model local:qwen-coder`).

Ships in: **Phase 1**.

### `memory-review`

Memory lifecycle demo. Candidate → review → approve → use → restrict → revoke.

Steps:

1. Generate three memory candidates of varying sensitivity.
2. Show how `medium` auto-approves, `high` requires review.
3. Demonstrate restriction: `actant memory restrict mem_X --never-cloud`.
4. Show downstream effect: a cloud model call's context inspector blocks the memory.

Ships in: **Phase 3**.

### `tool-approval`

Approval workflow demo for a high-risk tool call.

Steps:

1. Agent proposes a shell command with `risk=high`.
2. CLI shows the approval row with reversibility metadata.
3. User approves with `--once`; effect runs; result is captured.
4. A second similar call requires fresh approval (no scope grant).

Ships in: **Phase 2**.

### `context-firewall`

Shows the model-context firewall blocking sensitive memory.

Steps:

1. Workspace has a `secret`-sensitivity memory ("API key").
2. Agent issues `request_model_call` against a cloud route.
3. Context Inspector shows the memory blocked with `reason=visibility|sensitivity`.
4. The same call against a `local_only` route includes the memory.

Ships in: **Phase 3**.

### `workflow-dag`

Multi-step workflow with approval gate.

Steps:

1. Define a workflow with `fetch → summarize → approve → send`.
2. Cron-trigger fires.
3. Approval pauses at the gate.
4. CLI demonstrates `actant workflow watch <run>`.

Ships in: **Phase 4**.

### `mcp-github`

Import a GitHub MCP tool and govern calls.

Steps:

1. `actant mcp add github`.
2. `actant mcp wrap github.create_issue --approval high`.
3. Agent proposes a create-issue call.
4. Approval pauses; user approves.
5. Issue is created; reversibility note explains how to close.

Ships in: **Phase 2**.

### `replay-debugging`

Replay a failed run.

Steps:

1. Run a coding-agent session that intentionally fails (uses a wrong test command).
2. `actant replay create --from-event <id>` produces a checkpoint.
3. `actant replay run <id> --model local:qwen-coder` reruns with a different model.
4. Diff viewer (Studio) shows what changed.

Ships in: **Phase 5**.

### `swoosh-scout`

Local-first personal-data scan with reviewed memory.

Steps:

1. Local-only agent scans the user's notes directory.
2. Proposes memory candidates with `local_only` capsule.
3. User reviews. Approved memories never leave the device.

Ships in: **Phase 3**.

## Cadence

- Phase 1 ships `coding-agent` + a one-step `memory-review` stub.
- Each subsequent phase ships at least one example that exercises that phase's new primitives.
- Examples are tested in CI: `actant examples run <name> --headless` succeeds.

## Example structure

Each example directory:

```
examples/coding-agent/
├── README.md                  (what this teaches + commands shown)
├── example.yaml               (manifest: template + script + duration)
├── run.py                     (or run.ts / run.swift / run.rs)
└── expected_output.txt        (used by CI snapshot test)
```

## Adding an example

1. Create the directory + manifest.
2. Reference a template (`template: coding-agent`).
3. Add a CI snapshot test under `crates/actant-cli/tests/examples_<name>.rs`.
4. Update this catalog.
