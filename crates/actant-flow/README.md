# actant-flow

The Flow Engine — durable workflow DAGs.

Owns:

- Workflow definition parser (Phase 4 picks the format; Phase 1 has the trait + types only).
- Workflow run scheduler that progresses `workflow_run.current_node_ids`.
- Node handlers per `specs/07-workflows-and-replay.md` §2.
- Retry / timeout / approval-gate integration via `actant-effects` and `actant-policy`.
- Trigger registry (cron / event / webhook / manual).

Phase 1 scope: types + trait + a no-op run loop. Phase 4 is where this fills in.

See `agents/actant-flow.md` for the work package.
