# Anti-scope rules

Binding through day 90. These are the things we do **not** build before the wedge proves out (Gate 3, 2026-08-17).

## Crates and packages we don't ship

- ❌ A new database engine
- ❌ A full durable workflow runtime
- ❌ A vector database
- ❌ A protocol gateway (MCP / A2A / AP2 server)
- ❌ Four-language SDK coverage (Python / Swift / Rust beyond the TS package + minimal core)
- ❌ A marketplace
- ❌ Multi-tenant cloud
- ❌ Enterprise compliance packs
- ❌ Multi-region HA
- ❌ Multivector / late-interaction retrieval
- ❌ GraphRAG
- ❌ Registry / governance evidence rollups
- ❌ A Rust-based hot kernel
- ❌ A schema DSL
- ❌ Project-scaffolding generators beyond a single template

If any of these starts to feel necessary, that's the cue to **narrow** the wedge, not expand it.

## Things we don't say in public

- ❌ "ActantDB is the operating substrate for accountable autonomous action."
- ❌ "Realtime database + event ledger + state machine + permission system + effect queue + memory provenance + context firewall + workflow runtime + replay engine."
- ❌ "Six deployment modes."
- ❌ "Hot kernel + async lanes."
- ❌ "Compatible with OpenTelemetry GenAI + OpenInference." (true, but not the headline)
- ❌ Anything that takes more than one sentence to explain.

What we say instead is in [`positioning.md`](./positioning.md).

## Rules that bind every sprint

1. **No more planning files after this pivot.** The plan is the pivot doc + the wedge dir. New docs only if they unblock external installs.
2. **No empty stubs.** Every file checked in has a use site in the demo or a failing test that needs it.
3. **No more than 5 active work packages.** When a 6th is needed, finish or drop one first.
4. **First artifact is `@actant/mastra`, not Actant core.** The wrapper exists before the library exists, even if the wrapper duplicates work briefly.
5. **CLI supports only `studio`, `replay`, `approvals`.** Other subcommands wait.
6. **Every feature must appear in the killer demo.** If it's not visible in the demo, defer it.
7. **Every sprint ends with an external install attempt.** No sprint is "internal cleanup only."
8. **No full backend until 5 external developers ship/stage Actant.** Cloud, hosted, multi-tenant — all post-Gate-3.
9. **No Swoosh public users until Actant has 3 design partners.** Swoosh stays internal-only.
10. **Kill/pivot on schedule when validation fails.** Gates are not negotiable.

## The "we'll need this for coherence" trap

The thing that ate the planning corpus across May 2026 was the line of reasoning:

> "We'll need this for the substrate to be coherent."

Banned. If a feature is not required by the killer demo or by an external developer's install, it is not required for v0.1. Coherence is what v2 buys. v0.1 buys validation.

## What "yes" looks like

A new work package is allowed if **all** of these are true:

- It is named by a design partner pain (with a quote in the issue).
- It is required for the killer demo OR for an external install to succeed.
- It can ship in ≤ 2 weeks.
- It does not duplicate something Mastra / Convex / LangSmith already does well.
- Wes can review the entire output personally within 72 hours.

If a work package fails any of these, it goes to `wedge/parking-lot.md` (to be created as needed) and re-evaluated post-Gate-3.

## The substrate is not gone

Every concept in the v2 substrate roadmap — chronicle, Guard, commands, effects, capsules, intent, observation, replay, hot kernel, six deployment modes — earned its place in the design. They become the v2 roadmap when v0.1 earns the right to grow.

The anti-scope rules don't disagree with that vision. They prevent it from killing v0.1.

## How we enforce this

- The `/specs`, `/crates`, `/planning`, `/agents`, `/migrations`, `/templates`, `/examples` directories carry `STATUS.md` markers stating they are v2 roadmap and not active.
- Any commit that adds a file under those directories must reference an open issue tagged `v2-roadmap`.
- Any work-package-style markdown file added outside `/wedge/`, `/packages/`, or `/archive/` (post-pivot) needs an explicit PIVOT exception in the commit message.
- Reviewer prompt for every PR: "Does this serve an external install or the killer demo? If not, why is it open?"

When the wedge passes Gate 3, this file is rewritten. Until then, it's the constitution.
