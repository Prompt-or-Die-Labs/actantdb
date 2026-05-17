# actant-replay

The Replay Engine — checkpoints, replay runs, replay diffs.

Owns:

- `CheckpointWriter::create(event_id, ...)` producing `replay_checkpoint` + four snapshot artifacts (state, model_route, permission, memory) per `specs/07-workflows-and-replay.md` §7.
- The replay event loop covering all seven modes (`recorded`, `experimental`, `policy`, `model`, `memory`, `tool`, `local_only`).
- `replay_diff` row producer.
- Replay-scoped synthetic event storage (Phase 5 keeps these in artifacts; Phase 6 considers a dedicated table).

Phase 1 scope: checkpoint creation only. Phase 5 fills in the replay loops.

See `agents/actant-replay.md` for the work package.
