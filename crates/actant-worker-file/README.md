# actant-worker-file

Reference file-read/write worker for Phase 2.

Owns:

- Pattern-bound file access (`authority_scope.resource_pattern`) — refuses paths outside.
- Atomic write (write-to-tmp, rename) with `pre_state_artifact_ref` capture on writes for compensation.
- Sensitivity-aware reads: refuses to upload to artifact store as raw content when the read returns content whose sensitivity exceeds the route ceiling; passes hash + metadata only.
- Symlink and hard-link resolution (no escape via symlinks).
- Path normalization (`~` expansion handled here, recorded in observation).

Binary: `actant-worker-file`.

See `agents/actant-worker-file.md`.
