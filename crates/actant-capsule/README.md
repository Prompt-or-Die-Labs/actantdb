# actant-capsule

Capsule + sensitivity-lineage library. Phase 3.

Owns:

- `Capsule` and `CapsuleMembership` types + storage helpers.
- `resolve_capsules(object_type, object_id) -> Vec<Capsule>` — the function the context engine and memory extractor call.
- `compose_strictest(capsules: &[Capsule]) -> CapsulePolicy` — strictest-wins composition across multiple parents.
- `attach_to_capsule` operations that bind new derivations to their source capsules.
- Sensitivity-upgrade rules (e.g. low + personal-identifier → medium) configured per workspace.

Does **not** own: redaction (`actant-context` calls capsule policy then applies redaction).

See `agents/actant-capsule.md` and `specs/adr/0005-data-capsules.md`.
