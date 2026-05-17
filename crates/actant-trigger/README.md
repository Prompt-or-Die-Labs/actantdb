# actant-trigger

Workflow trigger engine. Phase 4.

Owns:

- `Trigger` enum with `Cron`, `Event`, `Webhook`, `Manual` variants.
- Cron evaluator (uses the `cron` crate; resolution = 1 minute).
- Event-trigger subscriber: a long-running task that filters `agent_event` rows and fires.
- Webhook ingress: `POST /v1/webhooks/{trigger_id}` handlers (the route lives in `actant-server`; the handler logic lives here).
- Manual trigger via `start_workflow_run` command.
- Trigger enable/disable + audit.

Does **not** own: workflow execution (`actant-flow`).

See `agents/actant-trigger.md`.
