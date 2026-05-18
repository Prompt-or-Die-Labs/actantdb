# Anti-scope (historical)

The original anti-scope list (binding through day 90) forbade building the substrate before the wedge proved out. The substrate was built anyway, in parallel, in the same session. The list is preserved below as historical record.

The rules that *survived* the substrate build and are still useful are kept in [`f2-f3-prevention.md`](./f2-f3-prevention.md):

- TS-native default install path (`npm install`, no Rust toolchain visible).
- Single source of truth for public types (`crates/actant-contracts` → `packages/actant-types`).
- Workspace smoke test on every PR (`pnpm smoke`).

The rules that were superseded by reality are in [`/CHANGELOG.md`](../CHANGELOG.md) — every item below ended up implemented or named as a deferred gap.

---

## Original list (superseded by reality)

What this document said the project should NOT build before Gate 3:

```
❌ A new database engine                — built (actant-storage, SQLite + Postgres)
❌ A full durable workflow runtime      — built (actant-flow + actant-trigger)
❌ A vector database                    — built (actant-embed + actant-index)
❌ A protocol gateway (MCP/A2A/AP2)     — built (actant-protocol + actant-worker-mcp)
❌ Multi-language SDK coverage          — TS + Python; Swift deferred
❌ Multi-tenant cloud                   — built (actant-tenant + actant-auth)
❌ Enterprise compliance packs          — built (actant-audit-export + retention)
❌ Multi-region HA                      — deferred (named in CHANGELOG)
❌ Multivector / late-interaction       — deferred (named)
❌ GraphRAG                             — deferred (named)
❌ Registry / evidence rollups          — partial (audit-export covers most)
❌ A Rust-based hot kernel              — built (actant-kernel)
❌ A schema DSL                         — built (actant-schema-dsl)
❌ Project-scaffolding generators       — built (actant-templates + actant-codegen-project)
```

## What replaced this document

[`/PIVOT.md`](../PIVOT.md) — the current state. [`/GATES.md`](../GATES.md) — Gate 1/2/3 punch list. [`/RELEASE_CHECKLIST.md`](../RELEASE_CHECKLIST.md) — the precise 5 steps to close Gates 2 + 3.

The premortem's actual prevention strategy turned out to be: build everything, ship the wedge first, install it on real developers' agents. That's what [`/wedge/validation-tests.md`](./validation-tests.md) + [`/RELEASE_CHECKLIST.md`](../RELEASE_CHECKLIST.md) cover now.
