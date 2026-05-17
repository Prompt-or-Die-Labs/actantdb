# agents/ — work packages for coding agents

This directory contains **work packages**: self-contained prompts that hand off a buildable unit of ActantDB to a coding agent (Claude Code, Cursor, Aider, etc.). Each file:

1. Tells the agent **why** the unit exists and where it sits in the architecture.
2. Lists the exact **specs to read first** from `/specs/`.
3. Names the **scope** (commands, types, tests) the agent must produce.
4. Defines binary **acceptance criteria**.
5. Lists **do-not-do** constraints to keep the agent in its lane.

This pattern lets a coding agent build a crate in isolation while remaining faithful to the spec set. Each work package is intentionally narrow — one crate per file, with named cross-crate interfaces.

## Order of work (Phase 1)

Build the crates in dependency order. Each step's work package assumes the previous step's crates compile and pass tests.

```
1.  actant-core
2.  actant-storage
3.  actant-policy           ┐
4.  actant-context          │ can be done in parallel
5.  actant-memory           │ once actant-core + actant-storage land
6.  actant-effects          ┘
7.  actant-command          (depends on all of the above)
8.  actant-subscribe
9.  actant-replay           (Phase 1: checkpoint-write only)
10. actant-flow             (Phase 1: types + traits only)
11. actant-server
12. actant-cli
13. actant-sdk-codegen      (last; reads server metadata)
```

Phase 1's **decision gate** (per `/specs/11-roadmap.md`) is the coding-agent demo from `/specs/10-alpha-demo.md` running end-to-end.

## Phase 2-6 work packages

The order, with one phase-extension package + the new-crate packages per phase. The phase plans in `/planning/` carry the detailed sequencing.

### CLI-supporting crates (Phase 1, alongside actant-cli)
```
12a. actant-templates       (bundled project templates + render engine)
12b. actant-schema-dsl      (.actant DSL parser + compilers)
12c. actant-codegen-project (actant generate command|effect|worker|agent|workflow)
```

These three sit alongside the existing `actant-cli` work package and are needed for the Phase 1 minimum CLI (per `/planning/cli-design.md`).

### AI-native + reliability crates (Phase 1+, after the core crates compile)
```
A1. actant-trace            (OTel + OpenInference; cross-cutting, lands early)
A2. actant-embedders        (provider registry; FastEmbed default)
A3. actant-index            (hybrid retrieval + traces + reranker dispatch + context packer)
A4. actant-prompts          (prompt + tool-schema registry)
A5. actant-models           (model registry + routing)
A6. actant-cache            (sensitivity-aware caches)
A7. actant-protocol         (MCP first; A2A/AP2 in Phase 4/6)

R1. actant-throttle         (multi-axis rate limits)
R2. actant-circuit          (per-dependency breakers)
R3. actant-lock             (lease-bounded resource locks)
R4. actant-ingress          (HMAC webhooks + email + calendar + fs + MCP/A2A)
```

These can be built in parallel groups: `A1` first (everything needs traces), then `A2 → A3` in series, and `A4..A7` + `R1..R4` in parallel.

### Hot-path coordinator (Phase 1, lands with the kernel discipline)
```
K1. actant-kernel           (dispatch table + capability tokens + hot projection L0 + admission control)
```

`actant-kernel` is the only crate that runs synchronously in the command path. It composes `actant-command`, `actant-policy`, `actant-storage`, `actant-effects`, and `actant-subscribe` under the discipline in ADR-0018. Build order: after `actant-command` + `actant-policy` + `actant-storage` + `actant-effects` + `actant-subscribe` compile, before `actant-server` adds HTTP / WebSocket. Coding agents implementing it should be familiar with `/specs/19-performance-architecture.md` + `/planning/performance-budgets.md` + `/planning/lane-catalog.md` first.

### Phase 2 (workers + extended primitives)
```
14. actant-worker-protocol
15. actant-worker-shell  ┐
16. actant-worker-file   │  in parallel
17. actant-worker-model  ┘
18. actant-worker-mcp
19. phase-2-extensions   (cross-cutting work in existing crates, including new CLI subcommands)
```

### Phase 3 (context + memory + capsules + trust)
```
20. actant-embed
21. actant-capsule
22. actant-trust
23. phase-3-extensions
```

### Phase 4 (workflows + triggers + regret/eval)
```
24. actant-trigger
25. actant-eval
26. phase-4-extensions   (includes actant-flow executor)
```

### Phase 5 (replay)
```
27. phase-5-extensions   (replay loops in actant-replay)
```

### Phase 6 (cloud / team)
```
28. actant-sync
29. actant-audit-export
30. phase-6-extensions
31. studio               (full dashboard)
32. sdk-ts               (in Phase 1 if Studio depends; otherwise Phase 6)
33. sdk-python
34. sdk-swift
35. sdk-rust
```

## How to run an agent

A canonical invocation in Claude Code:

```
Please implement the work package at agents/actant-core.md.
Read it fully, then read every spec it references, then implement the crate
under crates/actant-core/. Stop when every acceptance criterion is satisfied
and `cargo test -p actant-core` is green. Do not modify other crates.
```

Cursor and Aider follow the same pattern. The work package is the contract;
the spec set is the source of truth.

## Spec authority

The work packages **must not contradict** the spec set. If a work package
appears to conflict with a spec file, the spec wins; open an issue and update
the work package.

## Adding a new work package

Use `_template.md` in this directory as the starting point. Every work
package MUST contain these sections (enforced by CI):

- `## Context`
- `## Scope`
- `## Specs to read first`
- `## Acceptance criteria`
- `## Do NOT`
