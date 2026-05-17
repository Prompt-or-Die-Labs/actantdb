# Distribution plan

Obscurity is the default outcome. The plan: go where the users already are.

## Phase 1 — Mastra ecosystem first

Mastra is the highest-volume TS agent ecosystem in May 2026. We ship the Mastra wrapper first and pitch directly to that audience.

Deliverables by day 30:

- `@actant/mastra` on npm (alpha / pre-release tag).
- One example repo: `actant-mastra-test-cleanup-demo` matching [`killer-demo.md`](./killer-demo.md).
- One tutorial: "Add replay + approvals to your Mastra agent in 10 minutes."
- One Mastra-Studio-style screenshot of Actant Studio's timeline.
- One 90-second screen recording of the killer demo.

Channels:

- Mastra Discord — share the tutorial + screenshot in `#showcase` (after the demo passes the validation tests).
- Mastra issues / discussions — open issues offering to add Actant integration examples to Mastra-template repos that face approval / replay pain.
- Reach out directly to authors of Mastra agents we find on GitHub; ask if they'd try Actant on their agent.

**Do not ship to HN yet.** Premature HN with an unproven wedge is a category-defining mistake.

## Phase 2 — Convex ecosystem second

Only after the Mastra wedge has hit the Gate-1 threshold (3 non-Wes developers running it on real agents).

Deliverables when Phase 2 starts:

- `@actant/convex` on npm.
- One example repo: a Convex agent workflow with Actant replay + approvals wired in.
- One blog post: "Why my Convex agent deleted the wrong record, and how I replayed the failure path."

Channels:

- Convex Discord — same `#showcase` pattern.
- Convex docs — propose a community section / template that demonstrates Actant on a Convex agent.
- The Stack (Convex blog) — if they're open to guest posts, propose one.

## Phase 3 — Cross-framework

Only after Phase 1 + Phase 2 are stable.

Order:

1. `@actant/langgraph` — captures LangGraph node executions.
2. `@actant/openai-agents` — wraps OpenAI Agents SDK tool calls.
3. `@actant/mcp` — captures MCP tool invocations + resources, regardless of which framework called them.

The MCP wrapper is strategic: it covers any framework that uses MCP tools, which is most of them.

## Phase 4 — Direct outreach

Independent of phase, run this loop continuously:

- 5 direct conversations per week with agent developers.
- Offer to wire Actant into their agent in a 30-minute call.
- Each call produces either a design-partner relationship or a documented "won't try" reason.

The cumulative "won't try" reasons feed back into [`positioning.md`](./positioning.md) and the README.

## What we don't do

- **No paid ads.** Not until we have $0 revenue and ≥10 free design partners.
- **No conference talks before product is shippable.** A talk without a working demo is a category-defining mistake.
- **No Twitter / X campaign.** A single thread per design partner story when it lands, no more.
- **No "we will partner with…" announcements.** Real logo or no announcement.
- **No subreddit-blasting.** Targeted Discord / GitHub presence only.

## Channels to watch (not push)

- `r/LocalLLaMA`, `r/MachineLearning`, `r/LangChain` — listen for "trace doesn't tell me why" pain.
- HN agent-tool threads — when someone complains about reproducing an agent failure, that's our pull cue.
- Mastra, Convex, LangGraph, OpenAI SDK release notes — track when our integration target adds something that affects our wrapper.

## What success looks like in distribution terms

Not "thousands of stars."

```
Day 60: 5 completed external replays
Day 90: 15 completed external replays
Day 120 (if Gate 3 passed): 50 completed external replays + 5 named design partners
```

The funnel:

```
README seen          → 100s
Install attempted    → 10s
Install succeeded    → ~7 per 10
Real run captured    → ~5 per 10 of installs
First replay clicked → ~3 per 10 of installs
Used past one week   → ~3 per 10 of installs
Design partner       → 2–3 cumulative by day 75
Shipped or staged    → 5 cumulative by day 90
```

If the ratios drop more than 2× below these, the funnel is broken — usually at install or first-replay. Fix that before pushing more distribution.

## What happens if the Mastra ecosystem doesn't pull

- Switch the wedge target to OpenAI Agents SDK or LangGraph. The wedge content (Guard + Chronicle Replay) is framework-agnostic; the wrapper is what changes.
- If three framework targets in a row don't pull, the wedge is wrong, not the framework. Reposition (re-run the cold README test) before another wrapper.
