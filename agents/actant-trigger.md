# Work package: `actant-trigger`

## Context

Trigger engine for workflows. Phase 4.

## Specs to read first

- `/specs/07-workflows-and-replay.md` §4 (Triggers).
- `/specs/02-data-model.sql` — `trigger`.

## Scope

```rust
pub enum Trigger { Cron(CronTrigger), Event(EventTrigger), Webhook(WebhookTrigger), Manual }

pub struct TriggerEngine { storage: Arc<actant_storage::Storage> }

impl TriggerEngine {
    pub async fn start(self) -> Result<TriggerHandle, TriggerError>;     // returns a join handle
    pub async fn enable(&self, id: &str) -> Result<(), TriggerError>;
    pub async fn disable(&self, id: &str) -> Result<(), TriggerError>;
    pub async fn fire(&self, id: &str, payload: serde_json::Value) -> Result<WorkflowRunId, TriggerError>;
}
```

Cron precision: 1 minute (uses the `cron` crate). Event-trigger filter language matches `actant-subscribe`'s flat-equality filter.

### Internal modules

```
crates/actant-trigger/src/
├── lib.rs
├── engine.rs
├── cron.rs
├── event.rs
├── webhook.rs              // logic; the HTTP route lives in actant-server
└── error.rs
```

### Tests

- Cron precision within ±60s of the configured time over 10 fires.
- Event trigger: an `agent_event` matching the filter fires exactly once per match.
- Webhook: HMAC-signed payload accepted; unsigned/bad-signed rejected.
- Disabled trigger ignored even when its cron expression hits.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Surviving a process restart: a paused-at-restart cron trigger fires at its next scheduled time, not retroactively.
- [ ] No webhook trigger fires without HMAC verification.

## Do NOT

- Do NOT execute workflows here — call `actant-flow::start_workflow_run`.
- Do NOT poll the database. Use the `actant-subscribe` changefeed for event triggers.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
