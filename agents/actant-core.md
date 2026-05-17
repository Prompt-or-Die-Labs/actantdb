# Work package: `actant-core`

## Context

`actant-core` is the foundation crate. Every other ActantDB crate depends on it for IDs, enums, error types, and the event-hash helpers. It has no internal dependencies. Get this crate right and the rest of the workspace falls into place.

See `/specs/01-architecture.md` for where `actant-core` sits. There is no subsystem named "core" — this crate exists because the same primitives are needed by Chronicle, Command Engine, Effect Engine, Guard, etc.

## Specs to read first

- `/specs/02-data-model.sql` — full table list and the canonical column types (TEXT IDs, TEXT timestamps, INTEGER booleans).
- `/specs/05-security-model.md` §3 (sensitivity), §4 (visibility).
- `/specs/04-effect-protocol.md` §1 (effect lifecycle), §7 (effect-type catalog).
- `/specs/03-command-spec.md` §"Standard errors" for the error code enum.

## Scope

### Public API surface

```rust
// IDs — opaque newtypes around String. Display = inner value.
pub struct WorkspaceId(String);
pub struct ActorId(String);
pub struct SessionId(String);
pub struct MessageId(String);
pub struct EventId(String);
pub struct CommandId(String);
pub struct ModelCallId(String);
pub struct ToolCallId(String);
pub struct EffectId(String);
pub struct MemoryId(String);
pub struct MemoryCandidateId(String);
pub struct WorkflowId(String);
pub struct WorkflowRunId(String);
pub struct WorkflowStepRunId(String);
pub struct ArtifactId(String);
pub struct ApprovalRequestId(String);
pub struct ReplayCheckpointId(String);
pub struct ReplayRunId(String);
pub struct PolicyId(String);
pub struct AuthorityScopeId(String);
pub struct ContextBuildId(String);
pub struct ContextItemId(String);
pub struct WorkerId(String);

// Every ID gets:
impl WorkspaceId { pub fn new() -> Self; pub fn from_string(s: String) -> Self; pub fn as_str(&self) -> &str; }
// (use a macro to dedupe across IDs)

// Enums, all #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)].
pub enum ActorKind { Human, Agent, Subagent, Model, Tool, Worker, System }
pub enum Sensitivity { Public, Low, Medium, High, Secret, Regulated }
pub enum Visibility { LocalModelAllowed, CloudModelAllowed, HumanOnly, NeverModel, NeverSync }
pub enum RiskLevel { Low, Medium, High, Critical }
pub enum CausalityKind { Observation, Intent, Effect, Control, Audit }

// EventType and EffectType — enum with #[serde(rename_all = "snake_case")].
pub enum EventType { /* ... full list from specs/01-architecture.md §1 ... */ }
pub enum EffectType { ModelCall, ToolCall, ShellRun, BrowserAct, FileRead, FileWrite,
                      HttpRequest, CalendarRead, EmailDraft, EmailSend, MessageSend,
                      MemoryEmbed, WorkflowDispatch, HumanNotify }

// Sensitivity ordering: public < low < medium < high < secret < regulated.
impl Sensitivity { pub fn rank(&self) -> u8; }
impl PartialOrd for Sensitivity { /* by rank */ }

// Visibility is a SET. Use bitflags.
bitflags! { pub struct VisibilitySet: u8 { ... } }

// Error enum.
pub enum ActantError {
    Unauthenticated, Forbidden { decision_reason: String },
    InvalidInput { errors: Vec<FieldError> },
    PreconditionFailed { what: String },
    Conflict, NotFound { what: String },
    PolicyBlocked { reason: String },
    InternalError(anyhow::Error),
}

// Event-hash helpers.
pub fn payload_hash(bytes: &[u8]) -> String;        // SHA-256 hex lowercase
pub fn canonical_metadata(...) -> String;           // stable JSON for hashing
pub fn event_hash(parent: Option<&str>, payload_hash: &str, meta: &str) -> String;

// Time helpers.
pub fn now_rfc3339() -> String;                     // RFC3339 UTC, second precision
pub fn parse_rfc3339(s: &str) -> Result<OffsetDateTime, ActantError>;
```

### Internal modules

```
crates/actant-core/src/
├── lib.rs                       // re-exports
├── ids.rs                       // newtype IDs (macro-generated)
├── enums.rs                     // ActorKind, Sensitivity, Visibility, RiskLevel, ...
├── event_type.rs                // EventType enum
├── effect_type.rs               // EffectType enum
├── error.rs                     // ActantError + FieldError
├── hash.rs                      // payload_hash, canonical_metadata, event_hash
└── time.rs                      // RFC3339 helpers
```

### Tests

- Round-trip serde for every enum and ID.
- Sensitivity ordering: `public < low < medium < high < secret < regulated`.
- `event_hash` is deterministic for the same inputs.
- Two events with different `parent` hashes produce different `event_hash`.
- IDs generated with `::new()` are unique across many calls (small property test).

## Acceptance criteria

- [ ] `cargo build -p actant-core` zero warnings.
- [ ] `cargo test -p actant-core` passes.
- [ ] `cargo clippy -p actant-core -- -D warnings` passes.
- [ ] Every `Sensitivity` value from `/specs/05-security-model.md` §3 maps to a variant; round-trip via serde matches the snake_case form used in the spec.
- [ ] Every `EffectType` from `/specs/04-effect-protocol.md` §7 is a variant.
- [ ] No public function panics on valid input; invalid input returns `ActantError`.

## Do NOT

- Do NOT add a database dependency. Storage lives in `actant-storage`.
- Do NOT add a network dependency. HTTP/WS lives in `actant-server`.
- Do NOT inline policy logic. Policy lives in `actant-policy`.
- Do NOT introduce a "PrimitiveId" generic. The newtypes exist precisely to prevent passing a `MessageId` where an `EventId` is required.
- Do NOT use `unsafe`.

## Hand-off

Run `just ci` from the workspace root and ensure all green. Then mark the work package done by adding a one-line PR note: `actant-core: complete; <git sha>`.
