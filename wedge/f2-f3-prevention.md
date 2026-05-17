# F2 & F3 prevention — binding product constraints

The premortem named F2 (TypeScript market vs Rust project) and F3 (spec-to-code gap via coding agents) as the existential adoption + execution failures. Neither is fixable by "manage better." Both require **product and build-model constraints** that bind every other decision.

These constraints supersede anything in `/wedge/60-day-plan.md` they contradict.

## F2 fix — TypeScript-native default, Rust invisible

### Constraint

> A TypeScript developer must never need to know ActantDB is Rust-backed until they choose production scale-out.

### What this rules out (binding)

- ❌ No Rust toolchain in default install.
- ❌ No separate server in default install.
- ❌ No exposed port in default install.
- ❌ No worker diagram in the first README.
- ❌ No `curl install actantd` in the first README.
- ❌ No Docker in default install.

### What ships instead

Default install:

```bash
npm install @actantdb/core
```

```ts
import { createActant } from "@actantdb/core";

const actant = await createActant({
  mode: "embedded",
  project: "support-agent",
});
```

`@actantdb/core` ships:

- Prebuilt NAPI-RS native addon (`actant-napi`) for common platforms — Linux x64/arm64, macOS x64/arm64, Windows x64 — as `optionalDependencies` so `npm install` picks the matching one automatically.
- WASM fallback (`actant-wasm`) for any platform without a native build, plus edge / browser environments.
- Local SQLite event log under `~/.actantdb/<project>/events.sqlite`.
- In-process command engine, Guard, Chronicle. No daemon. No ports. No service.

Server mode is **scale-out only**:

```ts
const actant = await createActant({
  mode: "server",
  url: process.env.ACTANT_URL,
});
```

The API is **identical** between embedded and server mode — `actant.command(...)`, `actant.approvals.pending()`, `actant.replay.fromEvent(...)`. The choice is config, not migration.

### Onboarding test — strict gates

Run a 50-person unmoderated study with TypeScript developers who have **never** touched Rust. Compare against Mastra's `npm create mastra` baseline.

| Metric                                                | Pass threshold        |
| ----------------------------------------------------- | --------------------- |
| Median time-to-first-working-agent                    | ≤ 1.25× Mastra baseline |
| Abandonment at install                                | ≤ 10 %                |
| % who ask "why a server?"                             | ≤ 10 %                |
| % who hit a Rust toolchain error                      | 0 %                   |
| % who complete the replay demo                        | ≥ 60 %                |
| % who say "I understand why this isn't just Mastra"   | ≥ 70 %                |

If those don't pass, the product is wrong (not the marketing).

### Kill criterion

If six months post-launch:

- Weekly npm downloads of `@actantdb/core` are < 10 % of Mastra's, **AND**
- Outside-contributor PRs to the Rust core are < 5 / month

…do not keep pushing Rust-first. Pivot to:

- TS-native control plane
- Rust embedded accelerator (kept private, optional)
- Server mode only for enterprise / scale customers

## F3 fix — contract-first, agents implement only against `actant-contracts`

### Constraint

> All public types, errors, events, commands, and schemas live in ONE crate — `actant-contracts`. Coding agents may implement code; they may not invent interfaces.

### What this rules out (binding)

- ❌ No prose-spec-only contracts. The contract is Rust types + JSON schemas in `actant-contracts`, full stop.
- ❌ No 40-crate workspace before the wedge proves out. Start at 3 crates.
- ❌ No two crates with their own version of the same event, error, or trait.
- ❌ No PR merge without daily `cargo build --workspace` green.
- ❌ No new public type introduced outside `actant-contracts`.

### Starting crate count

```
crates/
├── actant-contracts     ← single source of truth (NEW)
├── actant-kernel        ← Rust implementation
├── actant-napi          ← Node bindings (bundled in @actantdb/core)
├── actant-wasm          ← WASM fallback (bundled in @actantdb/core)
└── actant-server        ← optional scale-out (later)

packages/
├── actant-core          ← @actantdb/core  (embedded TS runtime)
├── actant-types         ← @actantdb/types (generated from actant-contracts)
├── actant-policy        ← @actantdb/policy
├── actant-replay        ← @actantdb/replay
├── actant-mastra        ← @actantdb/mastra (wedge entry point)
├── actant-studio        ← @actantdb/studio (UI + `actantdb` CLI)
└── actant-convex        ← @actantdb/convex (conditional, Phase 2)
```

The 40-crate substrate vision under `/crates` is preserved as v2 roadmap with `STATUS.md` markers but is **not** the v0.1 architecture.

### The contract update protocol

Every change to a cross-package type follows exactly this path:

1. Author proposes the change in `crates/actant-contracts`.
2. `cargo run -p actant-contracts -- check-compat` — backward-incompatible changes without an explicit version bump fail.
3. `cargo run -p actant-contracts -- codegen-ts` — regenerates `packages/actant-types/src/generated/*`.
4. Commit Rust + regenerated TS in the same PR.
5. Human approval required before merge.

Coding-agent work packages MUST start with:

```
You may only use public interfaces from actant-contracts.
Do not modify actant-contracts.
If a needed interface is missing, stop and request a contract change.

Your PR is invalid unless:
  cargo check --workspace passes
  contract tests pass
  the workspace smoke test passes
```

### The workspace smoke test (mandatory from day 1)

A single test, in CI, run on every PR. It must pass before any other test counts:

```
create session
append user message
request tool call
require approval
approve tool call
complete effect
create replay checkpoint
render Studio timeline (headless)
```

If this fails, no merge. If this fails for more than 24 hours, freeze new work until it's green.

### Coherence validation test

Per `/wedge/validation-tests.md`, add an architecture-coherence test:

1. Agent implements crates 1–5 against `actant-contracts`.
2. Freeze interfaces (no contract changes).
3. **A fresh agent session** implements crate 6 using only the public interfaces of 1–5. No prose context from Wes.
4. Pass: crate 6 compiles with ≤ 1 interface clarification, workspace build + smoke test pass.
5. Fail: needs > 1 interface revision, requires hand-written cross-crate adapters, or invents alternate trait signatures.

If fail, the boundaries are too split — **collapse to a 3-crate monolith.**

### Early kill gate

By the time there are 5 crates compiling:

- `cargo build --workspace` must pass daily.
- The smoke test must pass daily.

If not, **collapse to a monolith immediately.** No discussion.

### August 1 gate (unchanged from prior plan)

If by Aug 1, 2026, `cargo build --workspace` cannot integrate 15+ crates with one smoke test passing, abandon the 40-crate architecture and rebuild as a 3-crate monolith.

## Combined F2 + F3 plan — what changes immediately

These constraints land **today (2026-05-17)**:

1. Default install path is `npm install @actantdb/core`. The first README line is npm, not curl, not cargo.
2. `actant-contracts` exists as a crate and ships with stub types before any other v0.1 implementation crate gains new public types.
3. The workspace smoke test is a failing test in CI from day one. Every PR runs it.
4. No new crate ships before passing the coherence validation test.
5. Wedge package layout is the user-facing answer; `crates/actant-kernel` + `crates/actant-napi` + `crates/actant-wasm` are the **invisible** implementation backbone.
6. The full 40-crate substrate in `/crates` stays archived as v2 roadmap and does NOT get new work without an explicit v2-roadmap issue.

## One-sentence answers

**F2:** ActantDB is not a Rust server TypeScript devs must install. It is an npm package with an embedded Rust/WASM core. Server mode is optional scale-out.

**F3:** ActantDB is not prose specs implemented by stateless agents. It is a contract-first system where every public type lives once in `actant-contracts`, every PR runs `cargo build --workspace` and an end-to-end smoke test, and any fresh agent session can implement a new crate using only the existing crates' published interfaces.

These two constraints prevent the two existential failures.
