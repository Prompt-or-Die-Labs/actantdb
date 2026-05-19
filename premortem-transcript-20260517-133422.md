# ActantDB premortem — transcript

**Date:** 2026-05-17
**Frame:** Six months have passed. It is November 17, 2026. The ActantDB strategy has died. Wide developer adoption did not happen. We are looking backward to understand why it died, what warning signs appeared, and what should have been changed before execution.

---

## Context brief

**Plan.** ActantDB — a Rust agent-backend product. As of 2026-05-17 the project has 258 planning files in `/Users/home/actantDB/`: 20 specifications, 3 schema migrations defining ~80 tables, 18 accepted ADRs, 50 agent work packages, 40 Rust crate scaffolds (empty `src/lib.rs` only — no production code), 20 planning docs.

**Positioning.** "ActantDB is the operating substrate for accountable autonomous action." Phase 1 promise: `actant new my-agent && cd my-agent && actant dev` → working governed agent in 5 minutes. Deep feature set: causal event chronicle, typed commands, effect queue + workers, Guard authority calculus, context firewall, memory provenance, workflow DAGs, replay engine, ActantIndex (hybrid retrieval), MCP/A2A/AP2 protocols, OTel + OpenInference observability, full reliability primitives, hot-kernel + async-lanes performance discipline, six deployment modes.

**Audience.** Agent developers (TypeScript + Python + Swift + Rust SDKs planned).

**Success criterion.** Wide developer adoption — thousands of developers running `actant new`, a real OSS community, GitHub stars, plugin ecosystem. **Failure = nobody outside the founder uses it.** Six-month horizon. (User noted: 6 months is short for this; the real metric is "have we crossed the chasm to organic pull?")

**Timeline.** Phase 0 (spec) done. Phase 1 alpha planned 4–6 weeks. Phase 6 ~8–12 more months.

**Team.** One person (Wes). Also building Swoosh (a personal-agent product for Mac) in parallel. Plan calls for coding agents (Claude Code, Cursor, Aider) to implement the 50 work packages. No design partners. No outside funding mentioned.

**Inferred from prior context.** Wes is also actively building Swoosh; the swift-mac-agent template was scaffolded explicitly so Swoosh can use ActantDB. This is the basis for the founder-bandwidth-split failure mode below.

**Market context (verified via web search, May 2026).**
- **Mastra** — TypeScript-native. 1.0 stable Jan 2026. $13M seed. 19k GitHub stars. 300k weekly npm downloads. Production at Replit, PayPal, Sanity, Brex, SoftBank, Marsh McLennan. The de facto TS choice for agent development in 2026.
- **LangGraph** — surpassed CrewAI in stars early 2026. Enterprise default for stateful workflows. Production at Klarna, Uber, LinkedIn, BlackRock, Cisco, Elastic, JPMorgan, Replit.
- **Convex** — shipped native "Durable Workflows" + "Agent memory" components in its reactive backend (architecturally the closest analog to ActantDB).
- **OpenAI Agents SDK** — March 2025. **Google ADK** — April 2025.
- Framework adoption nearly doubled YoY: 9 % of orgs (early 2025) → 18 % (early 2026).

Source: web search 2026-05-17 — Mastra docs, Speakeasy framework comparison, Datadog State of AI Engineering, agent-framework comparisons published April 2026.

---

## Raw failure modes (Step 5)

1. Spec-to-code chasm at 40-crate scale, one human reviewer.
2. No working 5-minute demo by month 6 — the pitch is the demo, and the demo requires ~18 crates + Studio to all ship coherently.
3. Differentiating claims ("governance + replay + accountability") are enterprise buying criteria, not solo-developer pull.
4. Rust-first kills the contributor pool and adds friction the volume audience won't accept.
5. Scope expansion within this very project: the plan doubled in size across our sessions, before any code shipped.
6. Hot-kernel discipline gets violated under deadline pressure, latency budgets miss, demo feels slow.
7. Privacy/governance defaults trip developers in dev mode and they walk away calling it "enterprise-y."
8. 20 specs is a wall to outsiders — competitors win with one good README.
9. Coding-agent build approach accumulates inconsistency across crates faster than one human can reconcile it.
10. Anthropic / Cowork ecosystem entanglement reads as politically aligned to some, unendorsed to others.
11. Wes is splitting attention with Swoosh.
12. SQLite alpha → Postgres deferred to Phase 6 = no production path visible at month 6.
13. The reliability + AI-native + protocols + observability + CLI ambition vs. ship date.
14. No design partners + no developer interviews + no waitlist signal.

## Blind-spot pass (Step 6)

Added after the standard lenses:

15. **Demand risk.** "Accountable autonomous action" may pull AI-safety researchers more than agent developers. Wrong audience for the design.
16. **Adoption risk.** Schema DSL + command model + approval pattern + context firewall + effect queue + memory lifecycle + replay + policy DSL = a cliff. Try-once-and-churn.
17. **Economic risk.** Open source, no stated funding model. Cloud (the revenue path) is Phase 6.
18. **Operational risk.** Even on success, one person can't support a fast-growing OSS community.
19. **Positioning risk.** "Actant" / "ActantDB" is unfamiliar. People search for "agent backend," not "actant."
20. **Stakeholder risk.** No outside accountability (funder, co-founder, design partner) → no forcing function that says "ship."
21. **Late-to-market.** The category crystallized in late 2025 / early 2026 around Mastra (TS volume), LangGraph (enterprise), Convex (reactive backend + agent components). May 2026 with zero code is structurally late.

## Consolidation

After merging overlapping modes around shared root assumptions: **9 distinct failure modes**.

| # | Failure mode |
| --- | --- |
| F1 | Late to a crystallized market |
| F2 | TypeScript-first market vs Rust-first project |
| F3 | Spec-to-code gap via coding agents |
| F4 | No working 5-minute demo by month 6 |
| F5 | Scope keeps eating runway |
| F6 | Differentiation matches enterprise buyers, not volume developers |
| F7 | Documentation wall (250+ markdown files) |
| F8 | Founder bandwidth split with Swoosh |
| F9 | No external validation; design is a guess |

## Scored failure map (Step 7)

| # | Failure mode | Likelihood | Severity | Detectability | Confidence | Primary mitigation |
| --- | --- | --- | --- | --- | --- | --- |
| F1 | Late to crystallized market | High | High | Easy | High | Ship a wedge as a Mastra/Convex plugin, not a replacement |
| F2 | TS market vs Rust project | High | High | Easy | High | Embed core via napi-rs/WASM as `@actantdb/core`; server mode is the scale-out option, not the default |
| F3 | Spec-to-code gap | High | High | Moderate | High | Replace prose specs with a machine-checked `actant-contracts` crate that all 40 crates depend on |
| F4 | No demo by month 6 | High | Critical | Moderate | High | Invert build order: write the demo as a stub-driven end-to-end test in week 1; fill stubs to satisfy it |
| F5 | Scope eats runway | High | High | Hard | High | Hard freeze: 40 crates → 4 (core, eventlog, sdk, cli-minimal). Everything else → `future/` |
| F6 | Wrong tier of buyer | Medium | High | Hard | Medium | Lead with one unmistakable developer pull (e.g. replay) — let governance trail |
| F7 | Documentation wall | Medium | Medium | Easy | High | A 200-line getting-started.md with a runnable example, everything else linked-not-surfaced |
| F8 | Founder bandwidth split | Medium | Critical | Easy | Medium | Freeze Swoosh public users until ActantDB has 3 paying design partners |
| F9 | No external validation | High | High | Easy | High | 15 customer-discovery interviews this week before writing more code |

---

## Deep dives (Step 8)

### F1 — Late to a crystallized market (full)

**Failure story.** In May 2026, while Mastra was already running in production at Replit and Brex with 300k weekly downloads and Convex was shipping Durable Workflows as a checkbox feature on an existing reactive backend, Wes was still in `actant new my-agent` slideware. The 258 planning files — 20 specs, 18 ADRs, 50 work packages, 40 empty Rust crates with stub `lib.rs` files — were a beautiful architecture for a category that had already picked its winners four months earlier. The "deep features" list (chronicle, Guard authority, context firewall, memory provenance, ActantIndex hybrid retrieval) read like differentiation on paper, but every item mapped to something a Mastra or LangGraph user could already get by composing two libraries they'd already adopted.

By August, Wes had implemented maybe 12 of 50 work packages with coding-agent help, split between ActantDB and Swoosh. The 5-minute `actant dev` promise shipped in September as a half-working CLI against a Rust core that nobody had production-tested. The first HN post got 40 upvotes and three comments — all variations of "how is this different from Mastra + Convex?" There was no answer because the answer required believing four SDKs, six deployment modes, and a hot-kernel performance story mattered more than the fact that Klarna was already running LangGraph at scale.

By November, GitHub stars sat at 380. Zero design partners. Zero production logos. Framework adoption had doubled again to ~30 % of orgs — and every one of those orgs picked from the three names that already had verified customers in May. The category didn't reject ActantDB; it never noticed it.

**Root assumption.** Architectural depth and breadth of SDK coverage would beat first-mover distribution, production logos, and a $13M sales motion.

**Early warning signs.**
- Every prospective user asks "why not just Mastra + Convex?" and the answer requires more than one sentence.
- After 60 days of building, no developer outside Wes has run `actant dev` voluntarily.
- The work-package backlog grows faster than it burns down.

**Validation test.** Before writing any more Rust: cold-pitch 15 working agent developers with the ActantDB README. If fewer than 3 say "I'd switch from $current_tool to try this" with a concrete reason, the substrate isn't real.

**Mitigation.** Kill the four-SDK, six-deployment, full-substrate scope. Ship one opinionated wedge as a Mastra/Convex plugin — Guard authority + chronicle replay — that runs on top of the incumbents instead of replacing them. Win one capability before claiming a category.

**Kill/pivot criterion.** If by August 17, 2026 (90 days in) ActantDB has fewer than 5 non-Wes developers who have shipped an agent on it, pivot to plugin-on-Mastra or shut it down. Sunk planning cost is not a reason to keep building.

---

### F2 — TypeScript-first market vs Rust-first project (full)

**Failure story.** ActantDB launched February 2026 with a polished Rust core, a typed TS SDK, and a "single binary, run anywhere" pitch. The first week looked promising — 1.8k GitHub stars, mostly from r/rust and Hacker News. Then the curve flattened. The TS Discord that did show up kept asking the same two things: "why do I need to run a separate server?" and "can I just `npm install` this?" Meanwhile Mastra shipped two minor releases in March that closed the durability gap with a SQLite-backed memory adapter — zero infra, in-process, one import. The comparison became unwinnable: a junior TS dev could ship a Mastra agent in 20 minutes; ActantDB demanded a Rust toolchain on CI, a worker process diagram, and a port to expose.

By April the contributor data told the rest of the story. 94 % of issues filed were TS/Python SDK bugs; 89 % of merged PRs were Rust core changes from three maintainers. The SDK authors couldn't fix their own bugs because the bugs lived in the GROQ-equivalent query planner written in Rust. PRs from outside contributors stalled in review because nobody else understood the actor runtime. The "wide developer adoption" goal silently became "Rust enthusiasts who tolerate a JS shim." In May, two of the three core maintainers burned out triaging SDK issues they couldn't action, the Python SDK fell three releases behind, and a CVE in the worker auth path sat open for 11 days. By November the npm package had 400 weekly downloads against Mastra's 600k. The project wasn't killed — it was simply skipped.

**Root assumption.** TS/Python developers will accept a separate Rust server process if the SDK ergonomics are good enough.

**Early warning signs.**
- Ratio of SDK-reported issues to SDK-authored fixes exceeds 5:1 within 60 days.
- First-time TS contributors open zero PRs against the Rust core after 90 days.
- "Why not just use Mastra?" appears unanswered in Discord/issues more than 3×/week.

**Validation test.** Before GA: run a 50-person unmoderated onboarding study with TS devs who have never touched Rust. Measure time-to-first-working-agent and drop-off point. If median TTFW exceeds Mastra's by more than 2× or > 40 % abandon at "install the server," the positioning is broken.

**Mitigation.** Ship an embedded mode: compile the Rust core to a Node native addon (napi-rs) and WASM, distributed as `@actantdb/core` on npm. No separate process for the dev-loop path; server mode becomes the production scale-out option, not the default. Reframe docs around "library that grows into a server," not "server with SDKs."

**Kill/pivot criterion.** If six months post-launch weekly npm downloads are under 10 % of Mastra's AND outside-contributor PRs to the Rust core are under 5/month, pivot to a TS-native rewrite of the control plane or sunset.

---

### F3 — Spec-to-code gap via coding agents (full)

**Failure story.** Wes started June 2026 strong. The first 8 work packages — `actant-core-types`, `actant-error`, `actant-config`, and the storage trait crate — landed in two weeks. Claude Code produced compiling, tested Rust. The pattern felt validated. Wes parallelized: by August, 30+ crates had PRs open, each generated against the same 20 specs and 3 schema migrations. Locally, every crate passed `cargo test`. The trap was invisible: each agent session re-derived its own interpretation of "AccountableAction," "GovernanceContext," and the audit-log schema from the specs. `actant-policy` modeled actions as enums; `actant-runtime` modeled them as trait objects; `actant-audit` serialized a third shape. Error types forked four ways across crates because ADR-009's `thiserror` pattern was ambiguous about whether transport errors wrapped storage errors or vice versa.

The first workspace-level `cargo build` happened in early September. It failed in 312 places. Wes spent three weeks reconciling `actant-core-types` with `actant-policy`, only to discover that fixing the trait shape invalidated `actant-runtime`, which had been built against the old shape and had 4,000 lines of agent-generated glue assuming it. Each reconciliation cluster took 2–3 weeks because Wes had not personally written any of the code and had to reverse-engineer the agents' assumptions before changing them. By October, the audit schema migration didn't match what `actant-audit` wrote, integration tests for the governance loop deadlocked, and `actant new my-agent` produced a project that compiled but panicked on first action. The 5-minute demo never ran end-to-end. Mastra shipped 2.0 in October. Wes shipped a README.

**Root assumption.** A single human reviewer can hold workspace-wide semantic coherence across 40 crates implemented in parallel by stateless coding agents working from prose specs.

**Early warning signs.**
- First cross-crate integration attempt requires non-trivial type adapters rather than direct calls.
- Two agent sessions, given the same spec section, produce materially different trait signatures.
- Review queue depth exceeds 5 open PRs for more than one week.

**Validation test.** Before writing any more crates: implement crates 1–5 end-to-end with the agent, then have a fresh agent session implement crate 6 (`actant-runtime`) using only the published interfaces of 1–5. If it requires more than one round of interface revision, the model is broken.

**Mitigation.** Replace prose specs with a single machine-checked `actant-contracts` crate — every trait, error, and schema type defined once, in Rust, with `cargo check` as the source of truth. All 40 crates depend on it. Coding agents consume types, not paragraphs.

**Kill/pivot criterion.** If by August 1, 2026, `cargo build --workspace` does not succeed with at least 15 crates integrated and one end-to-end smoke test passing, abandon the 40-crate architecture and rebuild as a 3-crate monolith.

---

### F4 — No working 5-minute demo by month 6 (full)

**Failure story.** The plan looked tractable on paper: 18 crates, coding agents grinding through 50 work packages, one human steering. Months 1–3 felt productive — actant-core, actant-storage, actant-policy, actant-command, actant-schema-dsl all compiled with green tests. Crate-level work is exactly what coding agents are best at: bounded interfaces, unit tests, type checks. The dashboard looked great. Twelve crates done by month 5.

Then the integration tax came due. The kernel had to compose storage + policy + effects + subscribe into a single runtime, but each crate had been built against a slightly different mental model of the event loop, the trace context, and the cancellation semantics. actant-subscribe assumed push from storage; storage had been built query-pull. The CLI's `actant dev` needed templates + schema-dsl + codegen-project + server to hand off cleanly, but codegen emitted modules the templates didn't reference. The index↔embedders↔cache triangle had three plausible ownership models and no decision. Every integration session uncovered a fresh API mismatch that required going back into a "finished" crate. Coding agents are excellent at filling in spec'd seams; they are bad at deciding which seam is correct when two crates disagree, and the one human couldn't be in eighteen places at once.

By month 6 the Studio app was a stub, `actant dev` crashed on cold start half the time and on the second approval the other half, and the "live memory candidates" panel — the actual wow moment — had never rendered real data end-to-end. Mastra shipped two more minor versions. The homepage said "coming soon." No video. The first three blog posts ended with "this is what it will be." That phrase is what killed it.

**Root assumption.** A working demo is the sum of working crates, rather than the product of integration decisions that must be made by a human in advance.

**Early warning signs.**
- Two "finished" crates need re-opening to make a third one compile against them.
- The `actant dev` happy-path script doesn't exist as an executable test by end of month 2.
- Any blog post or README ends with future tense.

**Validation test.** By end of month 2, write a failing end-to-end test: `actant new demo && actant dev` boots, accepts one command, fires one approval, surfaces one memory candidate. Crates can be stubs. The wiring must be real. If this test can't even be made to fail meaningfully, the architecture isn't ready.

**Mitigation.** Invert the build order. Week 1: build the demo as a hardcoded shell with all 18 crates as `todo!()` stubs that satisfy a frozen integration contract. Then fill stubs. Crates serve the demo, not the other way around.

**Kill/pivot criterion.** If at month 4 `actant dev` cannot run the demo script end-to-end with stub data, cut scope to a single-binary monolith and ship that as v0.1, or stop.

---

### F5 — Scope keeps eating runway (full)

**Failure story.** The pattern was visible by mid-June 2026 but invisible to the user inside it. Sessions 1–5 each ended with a planning artifact that *felt* like progress: 258 files, 40 crate scaffolds, ADRs 0004-0020, specs 15-19, three migrations. Every addition passed the local test — yes, an event-sourced substrate needs reliability primitives; yes, AI-native means real retrieval not bolt-on vector search; yes, a CLI without a schema DSL is a toy. The user, a one-person team also building Swoosh, mistook planning velocity for product velocity. Phase 1 began in week 7 instead of week 1 because the hot-kernel discipline (ADRs 0018-0020) required re-architecting the event-loop crate that Phase 1 depended on, which required finishing the lane catalog, which required the throttle/circuit/lock primitives from spec 17 to be specified first.

By August, Phase 1's 4–6 week estimate had become 16 weeks and only the event-log crate compiled. The 14 extended primitives from migration 0002 — regret, drift, compensation, memory_conflict — were untouched because no working surface existed to exercise them. The CLI, promoted to flagship in session 3, blocked on codegen-project which blocked on schema-dsl which blocked on the templates crate which blocked on a stable core API that didn't exist. In September, Mastra 1.1 shipped agent memory + workflow replay. Convex shipped reactive agent state with a 30-line quickstart. LangGraph added durable execution. Each ship made the user add *one more* differentiator to ActantDB's plan rather than cutting one. November 17: no public release, no users, the GitHub repo is 258 markdown files and 40 empty `lib.rs` files. Dead.

**Root assumption.** The substrate must be conceptually complete before any developer touches it.

**Early warning signs.**
- Ratio of planning artifacts to compiled code crossing 10:1 (currently infinite).
- New crates being added to Phase 1's dependency graph after Phase 1 has "started."
- The phrase "we'll need this for the substrate to be coherent" appearing in session notes.

**Validation test.** Before writing one more spec: build a 200-line single-crate prototype that records events, runs one state machine, and exposes one subscription. Show it to 5 agent developers this week. If fewer than 3 say "I'd use this tomorrow," the problem is positioning, not feature surface — and no amount of substrate will fix it.

**Mitigation.** Declare a hard freeze: 40 crates → 4 (`core`, `eventlog`, `sdk`, `cli-minimal`). Move the other 36 to a `future/` directory. Phase 1 ships those 4 with one templated example in 6 weeks or the project is wrong-shaped. Migrations 0002–0003 reduced to migration 0001-v2. ADRs 0008-0020 marked "post-v0.1."

**Kill/pivot criterion.** If by July 1, 2026 (6 weeks) there is no `cargo install actant-cli && actant init && actant run` working end-to-end against the 4-crate core: kill ActantDB as a product, extract the event-ledger crate as a library, return to Swoosh.

---

### F8 — Founder bandwidth split with Swoosh (full)

**Failure story.** Wes started June 2026 with the cleanest possible setup: 258 planning docs, 50 work packages mapped, swift-mac-agent template ready so Swoosh could dogfood ActantDB on day one. The thesis was elegant — Swoosh would be the canonical proof that ActantDB worked. By month two it became the canonical reason ActantDB didn't ship. Swoosh got real Mac users in private beta around week 6, and real users meant real bugs: keychain issues, MCP handshake races, a permissions dialog that fired twice. Each bug was a four-hour context switch out of the 40-crate backend. Coding agents kept producing 4000-line PRs against ActantDB crates — the storage layer, the event bus, the GROQ-style query planner — and those PRs sat in review queues for 5–9 days because Wes was the only person who understood the architecture well enough to merge them. Agent velocity outran review bandwidth by week 8.

By month 3, Swoosh was getting screenshots on X and the dopamine loop locked in. Wes started spending mornings on Swoosh polish (it had users who said thank you) and evenings on ActantDB (which had unmerged PRs that said nothing). Month 4: a Mastra 1.1 release shipped first-class Swift bindings. Month 5: a Convex partner announcement covered the "agent state" narrative ActantDB was supposed to own. Month 6: ActantDB had 31 of 50 work packages "in progress," three crates in passable shape, no design partners, no docs site, and a README that hadn't been updated in seven weeks. Swoosh survived because it had users. ActantDB died because it had a backlog.

**Root assumption.** One founder can simultaneously be the architect-of-record for a 40-crate infrastructure product AND the product-and-support owner for a consumer Mac app, because "coding agents do the work."

**Early warning signs.**
- PR review latency on ActantDB crates exceeds 72 hours for two consecutive weeks.
- Weekly commit ratio between the two repos drifts past 70/30 in either direction for 3+ weeks.
- Wes hasn't personally written a non-trivial ActantDB commit (not just merged one) in 14 days.

**Validation test.** Run a 2-week time-boxed simulation in June: log every working hour against {ActantDB-build, ActantDB-review, Swoosh-build, Swoosh-support, shared}. If ActantDB-build + ActantDB-review is under 25 hours/week, the plan is already failing — fix the structure before writing crate #2.

**Mitigation.** Freeze Swoosh at "internal-only dogfood" until ActantDB has 3 paying design partners. No public Swoosh users, no launch, no X posts. Swoosh exists solely as the integration test for ActantDB until ActantDB has external pull.

**Kill/pivot criterion.** If by end of month 3 (Aug 17, 2026) ActantDB has fewer than 10 of 50 work packages production-merged OR zero external design partners actively integrating, kill Swoosh as a separate product and fold its team-of-one entirely onto ActantDB — or kill ActantDB and go all-in on Swoosh. **Not both.**

---

### F6 — Differentiation matches enterprise buyers, not volume developers (compact)

The architecture optimizes for governance, replay, audit, compliance, sensitivity lineage. These are buying criteria for regulated enterprises — banks, healthcare, public sector. They are not pulls for a solo developer choosing an agent backend on a Sunday afternoon. By month 6 the message lands with three CISOs at Series-C companies but produces ~zero `actant new` runs from the OSS community the success criterion targets. The plan succeeds at the wrong tier.

**Root assumption.** Accountability/governance is a wide-developer pull.
**Warning sign.** The most positive feedback is from compliance/security people, not from working agent developers.
**Mitigation.** Lead with one unmistakable developer pull (replay agent failures, or fastest local agent dev loop) — let governance be the trailing benefit, not the headline.

---

### F7 — Documentation wall (compact)

20 specs + 18 ADRs + 20 planning docs + 50 work packages = 250+ markdown files. A developer evaluating ActantDB hits this and bounces. Mastra's homepage shows `pnpm create mastra@latest` and a 30-line working example. The completeness of ActantDB docs reads as overwhelming to outsiders even though it's the source of strength inside.

**Root assumption.** Documentation depth signals quality.
**Warning sign.** Every external review includes "this is a lot to read."
**Mitigation.** A single `getting-started.md` that's 200 lines including a runnable example. Everything else linked but not surfaced.

---

### F9 — No external validation; design is a guess (compact)

Eight hours of planning produced 258 files with zero developer interviews, zero design partners, zero waitlist. The audience identity ("agent developers") is broad enough that it doesn't predict buying behavior. Six months in, the docs describe a product no specific person explicitly asked for.

**Root assumption.** A well-reasoned design generalizes to a real audience.
**Warning sign.** Cannot name 3 specific developers who would switch from $current_tool to ActantDB.
**Mitigation.** 15 customer-discovery interviews this week before writing more code.

---

## Synthesis (Step 9)

### Executive summary

- **Most likely failure:** F1 + F4 combined — the market has crystallized around Mastra (TS), LangGraph (enterprise), and Convex (reactive backend + agent memory) **before ActantDB has shipped a single line of production code**, and the 5-minute demo that could've cut through won't be ready by month 6.
- **Most dangerous failure:** F8 — bandwidth split with Swoosh, because it determines which product survives; if Wes picks wrong, both die.
- **Most fragile assumption:** That ActantDB-as-designed (40 Rust crates, 4 SDKs, Studio, six deployment modes, full reliability + AI-native + protocols + observability stack, hot-kernel discipline) can be shipped by one person in 6 months against funded incumbents with production logos.
- **Most important change:** Pick **one wedge that is winnable** and ship it as a *complement* to incumbents rather than a *replacement* — strong candidate: chronicle + replay as a Mastra/Convex plugin or as a Node-embeddable library, distributed via npm.
- **Biggest unresolved unknown:** Have you actually talked to 15 working agent developers about whether they'd pay (in attention or money) for what you're designing? Per F9, no — and the answer almost certainly reshapes the plan.

### Failure map

| # | Failure mode | Likelihood | Severity | Detectability | Confidence | Primary mitigation |
| --- | --- | --- | --- | --- | --- | --- |
| F1 | Late to a crystallized market | High | High | Easy | High | Ship a wedge as a Mastra/Convex plugin, not a replacement |
| F2 | TS market vs Rust project | High | High | Easy | High | Embed core via napi-rs/WASM; ship on npm as `@actantdb/core` |
| F3 | Spec-to-code gap via coding agents | High | High | Moderate | High | Single machine-checked `actant-contracts` crate as source of truth |
| F4 | No working 5-minute demo by month 6 | High | Critical | Moderate | High | Demo-first: end-to-end stub script in week 1, fill stubs after |
| F5 | Scope keeps eating runway | High | High | Hard | High | Hard freeze: 40 crates → 4. Everything else → `future/` |
| F6 | Wrong tier of buyer | Medium | High | Hard | Medium | Lead with a developer pull (replay) — governance trails |
| F7 | Documentation wall | Medium | Medium | Easy | High | 200-line getting-started.md; deep docs linked-not-surfaced |
| F8 | Founder bandwidth split with Swoosh | Medium | Critical | Easy | Medium | Freeze Swoosh public users until 3 ActantDB design partners |
| F9 | No external validation | High | High | Easy | High | 15 customer-discovery interviews this week |

### Most likely failure (in plain words)

ActantDB ships into a market that already chose its winners. By August 2026, prospective users keep asking "why not just Mastra + Convex?" and the team can't answer in one sentence. By October, the working demo still doesn't exist because integration of 18 interdependent crates is much harder than building any one of them. By November, GitHub stars sit in the low hundreds. Adoption never starts because the moment of organic pull required either (a) a demo that's better than what already exists, or (b) a category developers didn't already have an answer for — and neither is true.

### Most dangerous failure

F8 (founder bandwidth split). Severity is **critical** because it's the choice that determines which product survives the year. If Swoosh is the survivor, ActantDB dies as planned. If ActantDB is the survivor, the cost is Swoosh — which is the only thing forcing dogfood and might be the most valuable thing Wes has shipped to date. Trying to maintain both is the worst path: each gets less than half-attention and both stagnate. The mitigation requires a real decision (freeze one until the other has external traction), and the kill criterion is sharp (10 work packages merged + design partners by month 3, or commit to one).

### Hidden assumption

> **You can ship ActantDB-as-designed, with the team and time you have, against the competitors who already shipped, and earn wide developer adoption.**

This is the single assumption underneath F1 + F2 + F3 + F4 + F5 + F8 + F9. The architecture isn't wrong. The bet that one person, splitting attention with Swoosh, using coding agents on 40 Rust crates, into a TypeScript-volume market that has already named its winners, can hit "wide developer adoption" in 6 months — that bet is what's failing.

Test it explicitly: take the current plan to 15 working agent developers (people currently shipping agents at companies of 10–500 people, on Mastra/LangGraph/Convex/OpenAI Agents SDK). Ask one question: "What would have to be true for you to move from your current tool to ActantDB?" If fewer than 3 give a concrete answer, the substrate is wrong — and no amount of additional scope will fix it.

### Revised plan

**Change now**
- Reframe the project. ActantDB is too big to win as a category in 6 months. Pick one wedge: chronicle + replay, or governance plug-in for Mastra/Convex, or local-first private memory for Swoosh. Make the rest later.
- 40-crate scope freeze. Move 36 crates to `future/`. Phase 1 is 4 crates: `actant-core`, `actant-eventlog`, `actant-sdk-ts`, `actant-cli-minimal`. Six weeks.
- Stop writing specs. Existing spec set goes to `archive/`. The single living spec becomes the `actant-contracts` Rust crate.
- Ship the demo-first scaffold: a failing end-to-end `actant new demo && actant dev` test, with `todo!()` stubs in every crate, before any feature implementation begins.

**Test before committing**
- 15 customer-discovery interviews this week. Working agent developers. One question: "What would have to be true for you to use ActantDB?"
- A 2-week time audit: log every hour against {ActantDB build, ActantDB review, Swoosh build, Swoosh support}. If ActantDB-direct work is under 25 h/week, the structure is already broken.
- A coding-agent coherence test: implement crates 1–5 end-to-end with Claude Code; have a *fresh* agent session implement crate 6 using only their published interfaces. If it needs more than one round of interface revision, replace prose specs with a machine-checked contracts crate now.

**Monitor during execution**
- PR review latency on ActantDB. Hard threshold: 72 hours.
- Weekly commit ratio between ActantDB and Swoosh. Hard threshold: 70/30 in either direction for 3+ weeks triggers a decision.
- Ratio of "actant new && actant dev" cold-start successes to attempts. Goal: 100 % by end of month 2.
- "Why not just Mastra + Convex?" appearances in issues/Discord. If unanswered 3+ times, the positioning is broken.

**Accept as risk**
- The architectural depth (Actant Contract, hot kernel discipline, ActantIndex, reliability primitives) may never be fully delivered in v0.1. Accept that the v0.1 product is much smaller than the design; the design becomes the v2 roadmap.
- Some of the work done so far is "wasted" in the narrow sense. It's also the strongest competitive moat ActantDB has — but only if a v0.1 ships to put it in front of developers.

### Pre-launch checklist

- [ ] Talk to 15 working agent developers by end of week, with the single question above. Record responses.
- [ ] Pick one wedge by end of next week. Write a one-paragraph positioning statement: "ActantDB is the [thing] for developers who want [pain] without [trade-off]."
- [ ] Commit to a 4-crate scope freeze. Move everything else to `future/`. Get this into the repo as a visible commit.
- [ ] Run the time-audit simulation for 2 weeks before writing crate #2.
- [ ] Replace prose specs with `actant-contracts` crate before parallelizing coding agents.
- [ ] Write the demo-first failing test before any feature implementation.
- [ ] Choose: freeze Swoosh users until ActantDB has 3 design partners, or commit fully to Swoosh and re-evaluate ActantDB after. Not both.

### Kill / pivot criteria

- By **July 1, 2026** (6 weeks): if `cargo install actant-cli && actant init && actant run` does not produce a working end-to-end demo against the 4-crate core, freeze ActantDB and extract the event-ledger as a standalone library.
- By **August 1, 2026**: if `cargo build --workspace` cannot integrate 15 crates with one end-to-end smoke test passing, abandon the 40-crate architecture and rebuild as a 3-crate monolith.
- By **August 17, 2026** (90 days in): if ActantDB has fewer than **5** non-Wes developers who have actually shipped an agent on it, pivot to plugin-on-Mastra or shut down.
- If ActantDB has fewer than **10 of 50** work packages production-merged OR **zero** external design partners actively integrating by end of month 3, kill Swoosh-as-separate-product OR kill ActantDB. Not both.
- If `actant dev` cannot run the demo script end-to-end with stub data by **month 4**, cut scope to a single-binary monolith and ship that as v0.1, or stop.

---

## Sources

- Web search 2026-05-17: Mastra docs, Speakeasy framework comparison, Datadog State of AI Engineering, AgentMarketCap LangGraph-vs-Temporal guide April 2026.
- `/Users/home/actantDB/` planning corpus (258 files; 20 specs, 18 ADRs, 50 work packages, 3 migrations, 40 crate scaffolds, 20 planning docs).
