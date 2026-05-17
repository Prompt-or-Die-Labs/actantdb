# planning/ — phase plans and cross-cutting design

The work in this directory closes every gap between the Phase 0 / 0.5 specs and the point at which coding agents start writing Rust. It contains:

- **Phase plans** (`phase-2-plan.md` … `phase-6-plan.md`) — what each phase ships, which crates change, which decision-gate criteria must pass.
- **Cross-cutting design** — `worker-fleet.md`, `studio-design.md`, `sdk-ts.md`, `sdk-python.md`, `sdk-swift.md`, `sdk-rust.md`.
- **Operating playbooks** — `test-strategy.md`, `eval-catalog.md`, `deployment-playbook.md`.

The specs in `/specs/` remain the source of truth. These docs *plan* implementations against the specs; they do not redefine the architecture.

## Order of consumption

1. Read `/specs/00-overview.md` → `13-actant-contract.md`.
2. Read `/agents/README.md` (the build order for Phase 1).
3. For each subsequent phase, read `phase-N-plan.md`, then the agent work packages it references.
4. Cross-cutting docs (`worker-fleet`, `studio-design`, `sdk-*`) apply across phases; the phase plans link to them.

## What this directory does not contain

- Rust source (`crates/`).
- SDK source (`sdks/`).
- Studio source (`studio/`).
- Migration source (`migrations/`).

Those four locations are where coding agents will write the implementation. Everything in `planning/` is a brief.
