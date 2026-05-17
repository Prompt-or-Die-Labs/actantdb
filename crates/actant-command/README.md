# actant-command

The typed mutation surface of ActantDB.

Owns:

- A `Command` trait: every command implements `validate`, `authorize`, `execute(&mut Transaction)`, declaring its input schema, required permissions, and emitted events.
- A `Dispatcher` that takes a parsed command + an authenticated actor and runs the lifecycle from `specs/01-architecture.md` §"Command Engine" — receive, authenticate, validate, authorize, transact, append, notify.
- Implementations for the **alpha command set** (Phase 1 scope; see `specs/03-command-spec.md` and `specs/11-roadmap.md` Phase 1):

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

- `CommandError` enum aligned with the standard error codes in `specs/03-command-spec.md`.

Does **not** own: HTTP routing (`actant-server`), policy evaluation internals (`actant-policy`), effect execution (`actant-effects` + workers).

See `agents/actant-command.md` for the work package.
