# actant-core

Shared core types for the ActantDB workspace.

Owns:

- ID newtypes (`WorkspaceId`, `ActorId`, `EventId`, `CommandId`, `EffectId`, etc. — opaque `String` wrappers, ULID/UUIDv7 generation helpers).
- Enums: `ActorKind`, `EventType`, `CausalityKind`, `Sensitivity`, `Visibility`, `RiskLevel`, `EffectType`, `CommandType`.
- Error type: `ActantError` (mapped from the standard error codes in `specs/03-command-spec.md`).
- Event-hash helpers (`event_hash(parent_hash, payload_hash, metadata)`).
- Canonical JSON serialization for hashing.

Does **not** own: SQL access, command dispatch, policy evaluation, network types. Those live in their respective crates.

See `agents/actant-core.md` for the work package.
