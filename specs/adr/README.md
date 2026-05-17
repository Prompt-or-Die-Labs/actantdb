# Architecture Decision Records (ADRs)

This directory holds ADRs that capture important architectural decisions in ActantDB. Use `_template.md` to start a new ADR.

## Index

| ID       | Title                                                       | Status   |
| -------- | ----------------------------------------------------------- | -------- |
| ADR-0001 | Commands as the only mutation path                          | Accepted |
| ADR-0002 | Side effects live outside the database transaction          | Accepted |
| ADR-0003 | Model context is a manifest, not an implicit prompt         | Accepted |
| ADR-0004 | Intent is a separate layer from action                      | Accepted |
| ADR-0005 | Data capsules — policy travels with derivations             | Accepted |
| ADR-0006 | Regret hooks — failures become evals automatically          | Accepted |
| ADR-0007 | Trust is calibrated by behavior                             | Accepted |
| ADR-0008 | The CLI is a first-class product surface                    | Accepted |
| ADR-0009 | The `.actant` schema DSL                                    | Accepted |
| ADR-0012 | Hybrid retrieval (dense + sparse) is the default            | Accepted |
| ADR-0013 | Reranking is part of the default stack                      | Accepted |
| ADR-0014 | Local-first embedders by default                            | Accepted |
| ADR-0015 | Observability follows OpenTelemetry GenAI + OpenInference   | Accepted |
| ADR-0016 | Reliability primitives are built into ActantDB              | Accepted |
| ADR-0017 | Idempotency is required for every command and effect        | Accepted |
| ADR-0018 | Hot kernel + async lanes                                    | Accepted |
| ADR-0019 | Progressive enrichment                                      | Accepted |
| ADR-0020 | Deployment modes                                            | Accepted |

ADR-0010 (Postgres vs SQLite-per-workspace) and ADR-0011 (sync conflict resolution) remain reserved for Phase 6 entry, per `/planning/phase-6-plan.md`.

## When to write an ADR

Write an ADR when a decision:

- Affects more than one crate or spec file.
- Closes off other options that a reasonable engineer might have chosen.
- Will be re-litigated if it isn't recorded.

Examples that should NOT be ADRs: bug fixes, performance tweaks within a crate, naming inside a single module.

## Lifecycle

```
proposed → accepted → superseded
                    → rejected
```

A superseded ADR is kept; the superseding ADR links back.
