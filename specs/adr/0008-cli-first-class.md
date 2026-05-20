# ADR-0008: The CLI is a first-class product surface

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

ActantDB has three plausible developer surfaces:

1. **SDK-first.** Developers learn the system through library docs and write code against the language SDK.
2. **CLI-as-scaffolder.** A small `actantdb` command exists for `start`, `migrate`, `status` — adequate for bootstrapping, then the developer lives in the SDK.
3. **CLI-as-product.** The CLI is the primary interface — it scaffolds, runs, observes, replays, generates, and deploys.

The first two options have shipped before. Successful agent-tooling competitors (Rails, Vercel, Temporal, Prisma, Docker Compose, LangSmith) consistently win developer mindshare with a strong CLI that exposes the full system surface. The bar is high: `actant new` → `actant dev` → working agent in under five minutes.

The risk of not committing to CLI-as-product: the developer's first hour is spent wiring memory, tools, approvals, traces, and storage by hand. ActantDB's actual value (governance, replay, the Actant Contract) is invisible during that hour, and developers churn before they see it.

The cost of committing: a much larger CLI surface area, four supporting crates (`actant-cli`, `actant-templates`, `actant-schema-dsl`, `actant-codegen-project`), per-template + per-example maintenance.

## Decision

ActantDB ships a **first-class CLI** as the primary developer surface. The CLI is responsible for:

1. **Scaffolding.** `actant new` + 10 templates by Phase 6, growing per-phase.
2. **Local dev loop.** `actant dev` boots the node, workers, Studio, and an example agent in one command.
3. **Observability.** `actant approval list`, `actant memory trace`, `actant context show`, `actant replay run`, `actant doctor`.
4. **Code generation.** `actant generate command|effect|worker|agent|workflow`.
5. **Schema management.** `actant schema validate|apply|diff|migrate` over the `.actant` DSL (ADR-0009).
6. **CI.** Every command supports `--json --quiet --yes --dry-run`; failures are non-zero exit codes.
7. **Deployment.** `actant deploy local|scaffold docker-compose|scaffold k8s|cloud`.

The CLI is **not** a wrapper around the SDK — it composes the same crates as the server, can run in-process or against a remote node, and embeds Studio.

The Phase 1 minimum is in `/planning/cli-design.md` §"CLI v0.1 minimum". Subsequent phases add commands per the staging table in that doc. The CLI must satisfy the **5-minute test**: from `cargo install` to a working approval flow in under 5 minutes wall-clock.

## Consequences

### Positive

- The first hour of an ActantDB developer's life is spent *using* the system, not building it.
- ActantDB's unique value (intent, approval, context manifest, replay) is visible immediately because the CLI surfaces every concept.
- Templates encode best practices. New developers don't have to rediscover the right shape of a coding agent.
- CI integration is structurally aligned with how developers run things locally.
- The CLI is a marketing surface: `actant examples run coding-agent` is the canonical homepage demo.

### Negative

- Four extra crates (`actant-cli`, `actant-templates`, `actant-schema-dsl`, `actant-codegen-project`) plus a `/templates/` and `/examples/` tree to maintain.
- Help text and output strings become a documentation surface. Drift between docs and CLI output is a real risk; mitigated by snapshot tests of help output.
- Templates can rot. Mitigated by CI: every template is scaffolded + booted in CI every PR.
- Commits to a particular shape early — but the shape is reversible (the CLI composes the same crates the server uses).

### Neutral / open

- Whether `actantd` (daemon mode) ships in Phase 4 or stays a long-tail Phase 6 item is unresolved.
- Whether `actant chat` becomes a full TUI in Phase 6 is unresolved.

## Alternatives considered

- **SDK-first.** Rejected — proven to lose to CLI-first competitors in developer traction.
- **CLI-as-scaffolder only.** Rejected — leaves observability and replay in the SDK, where they're invisible to developers running ad-hoc workflows.
- **Studio-as-product.** Studio is already first-class for *operating* an ActantDB system, but most developers never open Studio during their first hour. The CLI complements Studio; it does not replace it.

## References

- `/planning/cli-design.md` — full design.
- `/planning/cli-templates.md`, `/planning/cli-examples.md` — content catalogs.
- `/agents/actant-cli.md`, `/agents/actant-templates.md`, `/agents/actant-schema-dsl.md`, `/agents/actant-codegen-project.md`.
- Prior art: Rails CLI, Vercel CLI, Temporal CLI, Prisma CLI, Docker Compose.
