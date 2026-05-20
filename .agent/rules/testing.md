# ActantDB Testing and Verification Rules

- **Crate-Specific Rust Tests**: Run tests via `cargo test -p <crate_name>` rather than `--workspace` to avoid low disk space issues or cache crashes.
- **Smoke Tests**: Run smoke tests via `pnpm smoke` to verify workspace E2E. This must pass before merging.
- **Spec Verification**: Every `specs/*.md` must contain a `## Verification` section. Check with `just verify-specs`.
- **Agent Document Verification**: Every agent guidelines file `agents/actant-*.md` must contain:
  - `## Context`
  - `## Scope`
  - `## Specs to read first`
  - `## Acceptance criteria`
  - `## Do NOT`
  Check with `just verify-agents`.
