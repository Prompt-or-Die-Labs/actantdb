# ADR-0005: Data capsules — policy travels with derivations

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Privacy enforcement at ingestion is necessary but insufficient. A piece of data marked `sensitivity=high` at the source can lose its label as it is summarized, embedded, paraphrased, cross-referenced, or used as context for another model call. The standard pattern — "tag the row, hope downstream code respects the tag" — is unenforceable at scale.

`/specs/13-actant-contract.md` §8 names this gap: sensitivity must travel through transformations.

A worked example:

- A private memory says "User had a colonoscopy in March." `sensitivity=high`.
- An agent summarizes it as "User is health-conscious in spring." Looks `sensitivity=low` to a naïve labeller.
- The summary is offered to a cloud model.
- We have leaked health context.

Tag-at-source cannot stop this.

## Decision

Every piece of governable content can be attached to a **data capsule**. A capsule is a row carrying policy: sensitivity, visibility, sync_policy, retention_policy, redaction_policy, cloud_model_allowed, memory_allowed, optionally an `upgrades_to_sensitivity` directive.

Anything derived from one or more capsule-bound objects inherits the strictest policy among its sources. Inheritance happens at the moment of derivation, in the context engine, the memory extractor, the artifact recorder — wherever a new row is born from old rows.

`capsule_membership` rows record which objects belong to which capsule. The context firewall consults capsule policy alongside per-row sensitivity and per-route visibility.

## Consequences

### Positive

- Privacy enforcement becomes structural, not vigilant. A derivation cannot accidentally drop a label.
- Sensitivity upgrades (e.g. low + personal-identifier = medium) live in capsule rules and are auditable.
- Selective sync ("local_only", "metadata_only", "team_sync") works at the capsule level, which is the right level — content that came from a "local_only" capsule cannot sync regardless of how many transformations it passed through.
- Replay can show the capsule lineage of any context item, so a leak is debuggable.

### Negative

- Every new derivation needs to resolve its source capsules. This adds a join. We accept the cost.
- Strictest-wins composition can produce conservative outcomes that surprise users. Mitigated by visible-in-Studio capsule provenance and an explicit `weaken_capsule` command (admin-only, audited) for edge cases.
- Migration of legacy content into capsules is a one-time effort per workspace.

### Neutral / open

- Phase 3 ships per-row capsule attachment plus the strictest-wins resolution. Phase 4 adds workspace rule tables that drive sensitivity upgrades from combinations.
- Whether capsules can be nested ("a capsule of capsules") is deferred. The current schema is flat.

## Alternatives considered

- **Tag-only.** The status quo. Rejected — proven to fail at scale.
- **Differential privacy on every derivation.** Mathematically appealing, operationally infeasible for the variety of derivations ActantDB performs.
- **Capability-style flow typing in the runtime.** Compelling for a research system; too rigid for a backend that hosts third-party tools and models with unknown internals. We achieve the goal by guarding the row layer instead of the call layer.

## References

- `/specs/13-actant-contract.md` §8 (sensitivity travels)
- `/specs/14-extended-primitives.md` §3 (capsule schema)
- `/specs/06-context-and-memory.md` §3 (context firewall — capsule consultation extends this)
- `/specs/05-security-model.md` §3, §4 (sensitivity, visibility)
