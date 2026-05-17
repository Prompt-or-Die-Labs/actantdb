# PIVOT — 2026-05-17

## Decision

ActantDB the substrate is paused. **Actant ships as a plugin** — the missing flight recorder and authority gate for AI agents — on top of Mastra and Convex, not as a replacement backend.

The premortem ran on 2026-05-17 and surfaced the failure path clearly (`premortem-report-20260517-133422.html`, `premortem-transcript-20260517-133422.md`). The market crystallized before we shipped. Architectural depth doesn't beat distribution. So we change the launch surface.

## One-sentence answer

> **Actant lets you see, approve, and replay why a production agent took an action — without replacing Mastra, Convex, or your existing stack.**

## What's active now

```
/wedge/         — the 60-day plan, killer demo, validation tests, kill criteria,
                  positioning, distribution plan, anti-scope rules
/packages/      — npm-first scaffolds:
    @actantdb/mastra    wedge entry point (wrapper plugin)
    @actantdb/core      embedded TS runtime (NAPI-RS native + WASM fallback)
    @actantdb/types     generated from crates/actant-contracts — single source of truth
    @actantdb/policy    Guard verdict builders
    @actantdb/replay    replay engine
    @actantdb/studio    UI + `actantdb` CLI (the demo surface)
    @actantdb/convex    optional, only if a design partner uses Convex

/crates/        — internal Rust (invisible to TS developers):
    actant-contracts    single source of truth for all public types
    actant-kernel       fast Rust implementation
    actant-napi         Node bindings bundled into @actantdb/core
    actant-wasm         WASM fallback bundled into @actantdb/core
    actant-server       optional scale-out (later)
    (the v2 substrate crates remain frozen as v2 roadmap)
README.md       — rewritten around pain, not architecture
```

## What's frozen as v2 roadmap

The 258-file substrate plan is **not** deleted. It is preserved as v2 roadmap material. The Actant Contract framing, the 18 ADRs, the Guard + Chronicle + Replay specs, and the deep design vocabulary remain authoritative for v2. They are just not the v0.1 product.

```
/specs/         — STATUS: v2 substrate roadmap (frozen)
/crates/        — STATUS: v2 substrate scaffolds (frozen; 40 empty crates)
/planning/      — STATUS: v2 phase plans (frozen)
/agents/        — STATUS: v2 coding-agent work packages (frozen)
/migrations/    — STATUS: v2 schema (frozen)
/templates/     — STATUS: v2 CLI templates (frozen)
/examples/      — STATUS: v2 examples (frozen)
```

Every v1 top-level directory has a `STATUS.md` marker explaining the freeze. The freeze is honored: no agent should pick up a v1 work package until the wedge proves out.

## The wedge

Two features, period.

1. **Guard Authority** — runtime authority gate: allow / constrain / require approval / block / halt on every tool call, with policy snapshot and audit evidence.
2. **Chronicle Replay** — event timeline (model call, tool call, context manifest, approval, effect result) + replay from any decision point + replay diff under alternate policy/memory/context.

Nothing else ships before the wedge has external users.

## Hard validation gates

| Gate | Date | Threshold | Action on miss |
| --- | --- | --- | --- |
| Wedge MVP green | Jun 30, 2026 | `@actantdb/mastra` wraps a Mastra agent, captures tool calls + context manifest, supports approval, opens Studio with timeline + replay | Stop platform work |
| External adoption | Jul 31, 2026 | 10 non-Wes developers installed; 5 used on real projects; 3 kept past one week; 2 weekly-feedback design partners | Narrow the wedge further |
| Shipped/staged | Aug 17, 2026 | 5 non-Wes devs shipped or staged with Actant; 2 public examples; 1 named design partner | Pivot to plugin-only or shut down |

The one metric for 60 days: **completed external replays.** Target 5 by day 60, 15 by day 90. Stars do not matter yet.

## Anti-scope rules (binding)

1. No more planning files after this pivot.
2. No empty stubs without a use site in the demo.
3. No more than 5 active work packages at any time.
4. First artifact is `@actantdb/mastra`, not ActantDB core.
5. CLI supports only: `studio`, `replay`, `approvals`.
6. Every feature must appear in the killer demo.
7. Every sprint ends with an external install attempt.
8. No full backend until 5 external developers ship/stage Actant.
9. No Swoosh public users until Actant wedge has 3 design partners.
10. Kill/pivot on schedule when validation fails.

## What this means for Swoosh

Swoosh is the dogfood, not the launch. It stays at internal-only beta until Actant has 3 design partners. The swift-mac-agent template is preserved in v2 but is not Phase 1 work.

If the wedge fails by Aug 17, the choice is binary: kill Swoosh-as-separate-product and refocus, or kill Actant-as-separate-product and go all-in on Swoosh. **Not both.**

## Why this works

- It bypasses the "why not Mastra + Convex?" objection by saying "use them; add Actant for the missing layer."
- It bypasses the Rust-vs-TS market mismatch by shipping a TypeScript package via npm install.
- It bypasses the spec-to-code gap by scoping to 3 npm packages instead of 40 Rust crates.
- It bypasses the 6-month-to-wide-adoption fiction by setting kill criteria at 6, 10, and 13 weeks.
- It bypasses the documentation-wall problem by leading with a single screenshot and a 5-line wrapper snippet.

## What this does NOT do

- It does not abandon the v2 vision. The substrate is the natural growth of the wedge if the wedge earns its right to exist.
- It does not orphan the planning corpus. The 258 files become the v2 roadmap that v0.1 grows into.
- It does not commit to TypeScript forever. The TS plugin is the entry point. Rust-core re-emerges when needed.

## What to read next

1. `/README.md` — the new homepage framing.
2. `/wedge/README.md` — the wedge overview.
3. `/wedge/60-day-plan.md` — the day-by-day plan.
4. `/wedge/killer-demo.md` — the demo that defines the wedge.
5. `/wedge/kill-criteria.md` — the gates that prevent the substrate-by-stealth trap.
6. `/wedge/f2-f3-prevention.md` — binding product constraints (TS-native default, contract-first build). Supersedes anything in the plan it contradicts.

The premortem stays as reference at the repo root (`premortem-report-*.html`, `premortem-transcript-*.md`).
