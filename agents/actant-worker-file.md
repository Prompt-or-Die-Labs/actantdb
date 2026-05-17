# Work package: `actant-worker-file`

## Context

Reference worker for `file.read` and `file.write` effects. Path-bounded, sensitivity-aware, with atomic writes and pre-state capture for reversibility.

## Specs to read first

- `/specs/04-effect-protocol.md` §7 (`file.read`, `file.write`).
- `/specs/14-extended-primitives.md` §2 (Observation), §11 (Compensation plan).
- `/specs/05-security-model.md` §5 (resource patterns), §7 T3.

## Scope

### Behavior

- Resolve the path against the lease's `permission_scope_ref`. Refuse paths that escape (symlinks, `..`, hard links across the boundary).
- For `file.read`: read bytes, hash them, build an observation with `evidence_type='file_content'`. If `sensitivity` of the content exceeds the route ceiling, upload only the hash; otherwise upload the artifact.
- For `file.write`: capture `pre_state_artifact_ref` (the existing content or "absent"), write to a tmp file, fsync, rename atomically.
- Honor capsule policy if `lease.permission_scope_ref` includes a `capsule_id` (Phase 3 wiring; Phase 2 reads the field but no-ops if absent).

### Internal modules

```
crates/actant-worker-file/src/
├── main.rs
├── lib.rs
├── path.rs              // pattern matching, symlink resolution, escape detection
├── read.rs
├── write.rs             // atomic writes + pre-state capture
└── sensitivity.rs       // heuristic content classifier (regex-based; pluggable)
```

### Tests

- Symlink escape rejected.
- `..` escape rejected.
- Atomic write under crash: simulate a crash between write-tmp and rename; the original file is intact.
- Pre-state restore: capture, modify, then read the artifact and restore — produces byte-identical original.
- Sensitivity classifier: a file containing an obvious AWS key returns `sensitivity=secret`, suppressing artifact upload.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Property test: 1000 random path strings against a fixed pattern produce zero out-of-bound writes.
- [ ] A demo file edit + restore round-trip produces a byte-identical file.

## Do NOT

- Do NOT follow symlinks across the resource_pattern boundary.
- Do NOT upload artifacts for `sensitivity >= secret` content; hash-only is the rule.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
