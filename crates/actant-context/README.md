# actant-context

The Context Engine — model-context firewall and manifest builder.

Owns:

- `ContextBuilder::build(request) -> ContextBuild` running the six-stage pipeline (gather → score → firewall → redact → truncate → emit) per `specs/06-context-and-memory.md` §2.
- Pluggable scorer trait (Phase 1 default: weighted recency + simple keyword overlap; semantic similarity arrives in Phase 3 when the semantic index lands).
- Redaction passes: secret patterns, basic PII.
- Visibility/sensitivity-vs-route firewall logic.
- Token-budget truncation that respects pinned items.

Phase 1 ships a minimal pipeline used by the alpha demo's `build_context` command. Phase 3 deepens it (semantic scoring, embedding refs, advanced redaction).

See `agents/actant-context.md` for the work package.
