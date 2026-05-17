# Positioning

## The one-sentence answer

> **Actant lets you see, approve, and replay why a production agent took an action — without replacing Mastra, Convex, or your existing stack.**

If it takes more than one sentence to explain Actant to a working agent developer, the wedge is wrong.

## The HN objection answer

When someone asks "How is this different from Mastra + Convex?":

> Mastra builds and runs the agent. Convex gives you durable reactive state. Actant gives you the agent **flight recorder**: authority decisions, context manifests, approval evidence, and replay from any model/tool decision. It plugs into Mastra and Convex; it does not replace them.

That's the entire answer. If we find ourselves writing more, we've lost the wedge.

## The pain we name

```
Your agent just called a tool. Do you know why?
What did the model see?
Who approved this exact action?
What memory or context influenced it?
Can you replay this from before the bad step?
```

Most teams cannot answer these without a meeting. Actant turns each one into a click.

## What we DON'T compete on (first)

These categories are crowded; we lose the framing battle if we enter them as competitors:

- Agent framework — Mastra, LangGraph, CrewAI, OpenAI Agents SDK
- Durable backend — Convex, DBOS, Temporal, Inngest
- Vector database — Qdrant, pgvector, LanceDB, Chroma
- Observability — LangSmith, Braintrust, Arize, Langfuse, Phoenix
- CLI scaffolding — Mastra create, Convex init

We integrate with all of these. We do not replace any of them.

## What we DO compete on

```
agent runtime accountability
```

A single category. Sub-claims:

- **Authority** — runtime gate on tool calls with named verdicts (allow / constrain / require approval / block / halt) and audit evidence.
- **Replay** — causal record + counterfactual rerun. Not a trace. A re-executable timeline.
- **Context manifest** — the inspectable record of what the model saw, why each item was included or blocked, and whether anything left local-only context.

That set is small enough to defend and concrete enough to demo.

## The positioning map

| Category           | Winner / Incumbent                       | Actant position                                                |
| ------------------ | ---------------------------------------- | -------------------------------------------------------------- |
| Agent framework    | Mastra, LangGraph, CrewAI, OpenAI SDK    | Integrate, do not replace                                      |
| Durable backend    | Convex, DBOS, Temporal, Inngest          | Integrate, do not replace                                      |
| Observability      | LangSmith, Braintrust, Arize, Langfuse   | Add causal replay + context + authority. Export traces.        |
| Governance         | OpenBox-class runtime governance         | Differentiate with **replay** + **context manifests**          |
| Vector / RAG       | Qdrant, pgvector, LanceDB, Convex RAG    | Use / adapt; do not lead with it                               |
| Developer platform | Mastra / Convex / Vercel-style DX        | Piggyback first; own platform later                            |

The first category Actant claims is **agent runtime accountability**. Not "agentic database." Not "AI-native backend." Not "agent substrate."

## The runtime governance threat

OpenBox-class wrappers ship one-line runtime governance: score every tool call, return allow / constrain / approve / block / halt. If Actant is *only* a governance wrapper, OpenBox can match us with a release.

**The differentiator is replay + context manifests.** Governance verdicts without replay are stop-gap. Replay without governance is forensics-only. Actant ships both, joined.

## What changes after the wedge

If `@actant/mastra` lands and we get the metric (5 completed external replays by day 60, 15 by day 90), positioning evolves:

- Add `@actant/convex`, `@actant/langgraph`, `@actant/openai-agents` — same wedge, broader integrations.
- Add `@actant/mcp` — record MCP tool calls + resources.
- Then, only then, revisit the deeper substrate vision in `/specs`.

The substrate is the eventual product. The wedge is how we earn the right to build it.
