# actant-subscribe

The Subscription Engine — live row replication to clients.

Owns:

- `Subscriber::subscribe(table, filter, actor)` returning an async stream.
- Snapshot-then-incremental delivery contract from `specs/08-api-spec.md` §5.
- Per-subscription buffer with backpressure and `lag` notifications.
- Server-side filter evaluation against the actor's authority (subscriptions cannot return rows the actor cannot read).
- An in-process change-feed that command-engine commits notify on commit.

Phase 1 scope: changefeed + the subscription targets used by the alpha demo (`approval_request`, `agent_event`, `tool_call`, `memory_candidate`, `memory`).

See `agents/actant-subscribe.md` for the work package.
