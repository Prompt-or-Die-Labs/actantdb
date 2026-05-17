# Kill / pivot criteria

Three hard gates. Each names a date, a threshold, and an action on miss. The actions are not advisory; they are pre-committed.

## Gate 1 — Wedge MVP

**Date:** 2026-06-30 (day 44 from this pivot)

**Threshold (must all be true):**

- `@actant/mastra` wraps a Mastra agent without breaking it.
- `actant studio` opens with a timeline showing model call → tool call → context manifest → approval → effect result.
- Tool-call approval works: approve / deny / constrain.
- Replay checkpoint can rerun the model call from a chosen event.
- **3 non-Wes developers have run it on a real agent.**

**Action on miss.** Stop platform work. The substrate vision in `/specs` is suspended for the rest of the year. Resources go entirely to fixing whichever of the five threshold items failed, or to repositioning if the failure is "no developer was interested enough to try."

## Gate 2 — External adoption

**Date:** 2026-07-31 (day 75)

**Threshold (must all be true):**

- **10** non-Wes developers have installed `@actant/mastra`.
- **5** used it on real projects (not toy scripts).
- **3** kept it installed past one week.
- **2** design partners are giving weekly feedback.

**Action on miss.** Narrow the wedge further. Likely pivots:

- Drop the runtime authority gate; keep only the flight recorder (record + replay).
- Or drop replay; keep only the authority gate.
- Or change frameworks — if Mastra users aren't pulling, try `@actant/openai-agents` or `@actant/langgraph` and revisit the framework choice.

Whichever narrowing happens, document it in `wedge/positioning.md` and rerun the cold-README test.

## Gate 3 — Shipped or staged

**Date:** 2026-08-17 (day 92, 90 days from this pivot)

**Threshold (must all be true):**

- **5** non-Wes developers have shipped or staged an agent with Actant.
- **2** public example repos exist from real users (not Wes).
- **1** named design partner or production logo.

**Action on miss.** Binary choice:

- **Pivot to plugin-only.** Actant becomes a paid plugin / OSS library on top of one specific framework. The substrate vision in `/specs` is shelved indefinitely.
- **Shut down ActantDB the project.** Extract the event-ledger or replay code as a standalone library; redirect all of Wes's attention to Swoosh.

**Not allowed:** "Let's try another month." If the gate doesn't pass, the next step is one of the two above.

## Swoosh-vs-Actant gate (cross-cutting)

**Date:** 2026-08-17 (concurrent with Gate 3)

**Threshold:**

- Actant has **at least 10** of its planned wedge work items production-merged.
- Actant has **at least 1** external design partner actively integrating.

**Action on miss.** Choose one and only one product:

- Kill Swoosh-as-separate-product. Fold its codebase into Actant as an internal example. Wes is full-time on Actant.
- OR kill Actant-as-separate-product. Extract the event-ledger as a library inside Swoosh. Wes is full-time on Swoosh.

**Not both.** This is the founder-bandwidth-split kill described in `premortem-transcript-20260517-133422.md` §F8.

## Time-audit early warning (week 2)

Before any of the three product gates, an early warning. Run a 2-week time audit starting 2026-05-17.

**Threshold.** Actant-direct work (build + review, excluding planning) is ≥ 25 hours/week in both weeks.

**Action on miss.** The structure is broken before crate #2 is written. Fix the structure (freeze Swoosh user-facing work, hire / find a collaborator, or admit Actant can't have 60-day attention) before continuing.

## What this does NOT allow

- "Soft" extensions. Each gate has a specific date and threshold. Missing a gate by 30 % is a miss, not "almost passing."
- Adding new dimensions to the gate after the date is set. The thresholds above are the final ones for this wedge cycle.
- New scope before the gate passes. Even if the gate looks like it will pass next week, no new packages, no new specs, no Convex wrapper, no Studio dashboards beyond the demo timeline.

The gates exist so the failure described in the premortem can't happen by accident. If we miss a gate, the predicted failure is happening. The action on miss is how we don't waste another quarter.
