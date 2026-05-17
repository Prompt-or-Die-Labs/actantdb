# actant-audit-export

Nightly Chronicle export for compliance / external audit. Phase 6.

Owns:

- A `Destination` trait with implementations: local filesystem, S3-compatible, GCS, Azure Blob.
- `ExportPlan` that takes a workspace, a time window, a retention policy, and a sensitivity ceiling — outputs JSONL artifacts grouped by day.
- Bytes-identical re-run guarantee for the same `(workspace, window, policy)` triple.
- Tombstone preservation: redacted payloads still appear (audit skeleton) with hashes intact.
- A reusable manifest format so receivers can verify export completeness.

Does **not** own: real-time streaming exports (Phase 7+ if demanded). Nightly batch is the Phase 6 product.

See `agents/actant-audit-export.md`.
