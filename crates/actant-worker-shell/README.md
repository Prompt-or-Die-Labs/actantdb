# actant-worker-shell

Reference shell-effect worker for ActantDB Phase 2.

Owns:

- Child process spawning under a project-rooted sandbox (macOS: `sandbox-exec` profile; Linux: `bubblewrap` / `firejail`; Windows: `JobObject`).
- Resource-pattern enforcement (commands must be within an `authority_scope.resource_pattern`).
- Read-only / read-write modes via the lease's `sandbox_policy_ref`.
- Stdout/stderr capture → artifact uploads + structured observation (`evidence_type='shell_result'`).
- `pre_state_artifact_ref` capture for reversible operations (Phase 3 enhancement; Phase 2 limited to git-tracked dirs).
- Network denial by default; effect-grant required for `allow_network`.

Binary: `actant-worker-shell`.

See `agents/actant-worker-shell.md` and `planning/worker-fleet.md`.
