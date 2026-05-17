# Contributing to ActantDB

Thanks for your interest. ActantDB is in **Phase 0 (specification)**. The code surface does not exist yet — contributions during this phase are to the specs in `specs/`.

## Phase 0 contribution flow

1. **Open an issue first** for any non-trivial change. Use the issue template under `.github/ISSUE_TEMPLATE/`.
2. **One concern per PR.** A PR that touches `02-data-model.sql` should not also rewrite `05-security-model.md` unless the change is genuinely linked.
3. **Keep specs internally consistent.** Every command in `03-command-spec.md` must reference tables that exist in `02-data-model.sql` and emit events listed in `01-architecture.md`. The verification checklist at the bottom of each spec file lists the cross-references.
4. **Prefer explicit invariants.** Specs should state what cannot happen, not just what happens.
5. **Cite prior art.** If a design choice is borrowed from SpacetimeDB, Temporal, Datomic, EventStore, OpenPolicyAgent, NIST AI RMF, etc., link the source. ActantDB is clean-room but not invented in a vacuum.

## What we want in Phase 0

- Schema changes that close gaps in the data model
- New invariants for the security or privacy model
- Replay correctness arguments
- Threat-model entries
- Worked examples for the alpha demo
- Edge cases for the effect protocol (idempotency, retries, partial failure)

## What we do not want in Phase 0

- Code that anticipates Phase 1+
- Vendor-specific bindings (Postgres-only schema, etc.)
- New product surface area beyond the four products listed in the README
- Aesthetic-only formatting churn

## Style

- Markdown: 100-column soft wrap. ATX headers (`#`). Fenced code blocks with language hints.
- SQL: `snake_case`, `TEXT` for IDs, `TEXT NOT NULL` for required strings, ISO-8601 strings for timestamps until we move off SQLite alpha.
- Names: `actant_*` for crate names, `Actant*` for types in code, lowercase `actantdb` for the product when written as one word.

## Code of conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Be respectful. Argue about designs, not people.

## Licensing of contributions

By submitting a contribution you agree it is licensed under Apache 2.0 (the project license). See [LICENSE](LICENSE).
