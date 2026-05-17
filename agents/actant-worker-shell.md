# Work package: `actant-worker-shell`

## Context

Reference worker for `shell.run` effects. Executes a shell command in an OS-sandboxed environment, captures stdout/stderr, returns a structured observation. Phase 2.

## Specs to read first

- `/specs/04-effect-protocol.md` §7 (`shell.run` row).
- `/specs/14-extended-primitives.md` §2 (Observation), §11 (Compensation plan), §15 (Effect lease rich form).
- `/specs/05-security-model.md` §7 T3 (Tool misuse via argument injection).
- `/agents/actant-worker-protocol.md` — its public API.
- `/planning/worker-fleet.md` for the cross-phase sandboxing rules.

## Scope

### Behavior

1. Boot reads config (`ACTANT_SERVER_URL`, `ACTANT_WORKER_TOKEN`, `ACTANT_WORKER_NAME`, optional `ACTANT_SANDBOX_PROFILE`).
2. Calls `register_worker` with capabilities `["shell.run"]` (or a custom subset).
3. Loops on `claim` for `shell.run` effects; on each lease:
   - Verify `final_input_hash` matches the lease's `input_hash`.
   - Spawn the child process under the sandbox specified by `lease.sandbox_policy_ref` (defaults to project-rooted read-only).
   - Stream stdout/stderr to an artifact (size-capped; large outputs truncated with a marker line).
   - Stream periodic `observation` rows of type `shell_result` with intermediate summaries on long-running commands (>30s).
   - On exit: build the final `NewObservation` (exit code, stderr summary, suggested next actions when detected — e.g. "2 failing tests" parsed from pytest output).
   - Capture `pre_state_artifact_ref` for git-tracked directories (Phase 2 limited to "git stash" / "git diff" snapshot).
   - Call `complete`.

### Sandbox profiles

| Profile             | macOS                                  | Linux                              | Windows               |
| ------------------- | -------------------------------------- | ---------------------------------- | --------------------- |
| `project_read_only` | `sandbox-exec` deny-write              | `bwrap --ro-bind / /`              | `JobObject` read-only |
| `project_read_write`| `sandbox-exec` allow-write-in-project  | `bwrap --bind project project`     | `JobObject` rw-scoped |
| `network_denied`    | applied to both above                  | `bwrap --unshare-net`              | tcpip blocked         |
| `network_allowed`   | (requires explicit grant)              | network NS shared                  | network allowed       |

### Internal modules

```
crates/actant-worker-shell/src/
├── main.rs
├── lib.rs
├── sandbox/
│   ├── mod.rs
│   ├── macos.rs
│   ├── linux.rs
│   └── windows.rs
├── exec.rs
├── parse.rs                 // detect "N failing tests" etc. → suggested_next_actions
└── pre_state.rs             // git-stash-based capture
```

### Tests

- Sandbox denial: trying to write outside the project root fails the spawn, not the worker.
- Network-denied: `curl example.com` fails inside the sandbox.
- Streaming observation: `sleep 5 && echo done` produces ≥1 progress observation before the final one.
- Pre-state capture: writing a tracked file then completing leaves an artifact whose URI can be fetched to restore the prior content.
- Input-hash guard: hand-crafted command with a different hash than the lease declares is rejected.

## Acceptance criteria

- [ ] `cargo build -p actant-worker-shell` zero warnings.
- [ ] `cargo test -p actant-worker-shell` passes on at least one OS (CI matrix: ubuntu, macOS).
- [ ] `cargo clippy -p actant-worker-shell -- -D warnings` passes.
- [ ] A killed-mid-execution scenario (SIGKILL the worker between heartbeats) results in lease loss → re-claim → second worker completes; idempotency-key plumbing means external state doesn't double-mutate (where applicable).

## Do NOT

- Do NOT bypass the sandbox. Even read-only operations go through it.
- Do NOT expand the network-denied default. Effects that need network must say so explicitly in the lease.
- Do NOT use `unsafe` outside the sandbox FFI layer; if any FFI is needed (macOS sandbox_init), isolate it in a single `unsafe fn` with safety comments.

## Hand-off

`just ci`. Smoke test against `actantdb-server` running locally.
