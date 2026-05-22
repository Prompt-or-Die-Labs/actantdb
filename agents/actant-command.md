# Work package: `actant-command`

## Context

`actant-command` is the typed mutation surface. It is the only path to mutate state. The command engine authenticates the actor, validates input, calls Guard, opens a `Transaction`, runs the command's `execute`, appends events, and notifies subscribers.

Phase 1 implements the **alpha command set** named in `/specs/11-roadmap.md` Phase 1. It also owns the hot-path kernel module, semantic cache, prompt registry, and model route registry because those are backend command surfaces, not a separate agent runtime.

## Specs to read first

- `/specs/03-command-spec.md` — full file.
- `/specs/01-architecture.md` §"Command Engine".
- `/specs/05-security-model.md` §2 (invariants 1, 2, 3, 4).

## Scope (Phase 1)

### Public API surface

```rust
#[async_trait]
pub trait Command {
    type Input: serde::de::DeserializeOwned + schemars::JsonSchema + Send;
    type Output: serde::Serialize + Send;

    const COMMAND_TYPE: &'static str;

    fn validate(input: &Self::Input) -> Result<(), CommandError>;

    fn authorize<'a>(&'a self, input: &'a Self::Input, ctx: &'a CommandContext<'a>)
        -> futures::future::BoxFuture<'a, Result<actant_policy::Decision, CommandError>>;

    async fn execute(&self, input: Self::Input, ctx: &mut CommandContext<'_>)
        -> Result<Self::Output, CommandError>;
}

pub struct Dispatcher { /* registry + storage + policy + subscribe handle */ }

impl Dispatcher {
    pub async fn dispatch(&self, raw: RawCommand, actor: AuthenticatedActor)
        -> Result<DispatchResult, CommandError>;
}

pub struct RawCommand { pub command: String, pub workspace_id: String, pub input: serde_json::Value, pub idempotency_key: Option<String> }
pub struct DispatchResult { pub command_id: CommandId, pub events: Vec<EmittedEvent>, pub result: serde_json::Value }
```

### Alpha command set (Phase 1)

Implement each as a module under `commands/` exposing a struct that implements `Command`:

- `create_session`
- `append_user_message`
- `append_agent_message`
- `request_tool_call`
- `approve_tool_call`
- `deny_tool_call`
- `record_tool_result`
- `propose_memory`
- `approve_memory`
- `reject_memory`

### Internal modules

```
crates/actant-command/src/
├── lib.rs
├── dispatcher.rs
├── context.rs                     // CommandContext = Transaction + actor + policy + clock
├── command_trait.rs
├── error.rs                       // CommandError -> ActantError mapping
├── registry.rs
├── kernel.rs
├── cache.rs
├── models.rs
├── prompts.rs
└── commands/
    ├── mod.rs
    ├── create_session.rs
    ├── append_user_message.rs
    ├── append_agent_message.rs
    ├── request_tool_call.rs
    ├── approve_tool_call.rs
    ├── deny_tool_call.rs
    ├── record_tool_result.rs
    ├── propose_memory.rs
    ├── approve_memory.rs
    └── reject_memory.rs
```

### Tests

- For every command:
  - Schema validation rejects malformed input.
  - Authorization denial returns `forbidden` with the policy reason.
  - Successful path produces the spec-listed projection writes and emits the spec-listed events.
  - Idempotency key replays return the original result.
- Transaction safety: an `authorize` denial inserts a `command_record` with `status='rejected'` and emits no events.
- Subscriber notification fires exactly once per committed command, with all emitted events.

## Acceptance criteria

- [ ] `cargo build -p actant-command` zero warnings.
- [ ] `cargo test -p actant-command` passes.
- [ ] `cargo clippy -p actant-command -- -D warnings` passes.
- [ ] Every command in `/specs/03-command-spec.md`'s alpha set has a module here and a test exercising every documented event emission.
- [ ] Invariant 1 in `/specs/05-security-model.md` is structurally true: a grep for `INSERT INTO` outside `actant-storage` (excluding tests) returns nothing.

## Do NOT

- Do NOT bypass `actant-storage`'s `Transaction`. All writes go through it.
- Do NOT skip Guard. Every command calls `authorize`.
- Do NOT add commands outside the Phase 1 alpha set. Other commands ship in later phases per the roadmap.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
