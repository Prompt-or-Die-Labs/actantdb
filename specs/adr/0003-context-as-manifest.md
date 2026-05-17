# ADR-0003: Model context is a manifest, not an implicit prompt

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Most agent systems build a model prompt as a transient string — concatenating system instructions, recent messages, retrieved memories, and tool outputs. The string is sent to the model and discarded. If something goes wrong, there is no inspectable record of what the model saw.

ActantDB needs to answer, definitively:

- Did the model see this private file?
- Did the cloud model receive sensitive memory?
- Why did the agent assume X — what memory was in its prompt?
- Replay: run this prompt again with a stricter visibility policy. Did anything leak?

A transient prompt cannot answer these questions. We need an inspectable, replayable record of what the model saw — and the rules that admitted or excluded each item.

## Decision

Every model call has exactly one `context_build` row plus N `context_item` rows that together constitute the **context manifest**. The manifest records:

- The candidates considered (included AND excluded).
- The reason each item was included or blocked (sensitivity, visibility, budget, residency, etc.).
- The rank score and the token count for included items.
- The redaction summary for the final prompt.
- A SHA-256 of the final canonical prompt so replay can verify exact reproduction.

The pipeline (`/specs/06-context-and-memory.md` §2) is: gather → score → firewall → redact → truncate → emit.

`/specs/05-security-model.md` §2 invariants 5, 6, and 7 depend on this:

- No model call without a context manifest (5).
- No memory without provenance — and the manifest is the join point from memory to model call (6).
- No cloud context without visibility policy — the firewall stage is where this lives (7).

## Consequences

### Positive

- "Did the model see X?" is a database query.
- Replay can reconstruct the exact prompt (`final_prompt_hash`).
- The firewall blocks data leaks structurally; policy changes can be tested with `mode=policy` replay.
- Memory provenance ties cleanly into model calls via `memory_use` rows that reference the context_build.

### Negative

- More rows per model call. Mitigated: context items are typically O(20–200), each small.
- Building a manifest takes time. Mitigated: the pipeline is in-process, and the score/firewall steps are cheap for Phase 1 (semantic similarity arrives in Phase 3 once embeddings ship).

### Neutral / open

- The scorer is pluggable. Workspaces can supply custom rankers in Phase 2+. Phase 1 ships a simple default.
- Redaction is loss-y; the un-redacted source is preserved in its source table (memory, message, file) but not in the manifest.

## Alternatives considered

- **Log the final prompt only.** Rejected — answers "what was sent" but not "what was excluded." Audits and policy replays need both.
- **Embed manifest in the model call row.** Rejected — context items have their own filtering and visibility needs and benefit from being first-class rows.
- **Defer the firewall to a sidecar / proxy.** Rejected — the firewall is a property of *context building*, not of the wire. Moving it elsewhere reintroduces the leak path during build.

## References

- `/specs/06-context-and-memory.md`
- `/specs/05-security-model.md` §2 invariants 5–7, §3 (sensitivity), §4 (visibility)
- `/specs/07-workflows-and-replay.md` §6 — replay modes that depend on the manifest
