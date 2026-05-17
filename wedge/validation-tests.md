# Validation tests

Three tests, three pass/fail conditions. Run them in order. Don't lie about results.

## 1. Cold README test

**Method.** Send only the [`/README.md`](../README.md) to 15 working agent developers — people currently shipping agents on Mastra, LangGraph, Convex, or OpenAI Agents SDK. No call. No explanation. Just the README.

**Pass.**

- At least **5** ask for install instructions.
- At least **3** say they would add it to an existing agent **this week**.
- At least **2** name a current pain it would solve.

**Fail.**

- "Interesting architecture."
- "How is this different from Mastra + Convex?"
- "We already use LangSmith / Mastra / Convex."
- "Maybe useful later."

**"Interesting architecture" is a fail.** If the most common reply is intellectual curiosity rather than install intent, the README is wrong (or the wedge is wrong; iterate the README first).

**Cadence.** Re-run weekly with a fresh batch of 5 until pass conditions hold. Edit the README between batches. Don't edit the product yet — the README is what's being tested.

## 2. Install test

**Method.** Give 10 developers a 10-minute install script: `npm install @actant/mastra`, wrap one agent, run `actant studio`, capture one run.

**Pass.**

- **7 of 10** install successfully in under 10 minutes.
- **5 of 10** capture a real agent run end-to-end.
- **3 of 10** produce a replay or approval trace they can show someone.

**Fail.**

- People need explanation from Wes to get past the install.
- The trace renders but is not obviously useful.
- The plugin breaks their existing agent.
- The replay is not compelling — "okay, so what?"

**Iteration rule.** Every failed install produces exactly one ticket. The ticket must be either fixed before the next install call OR documented in a "known limitation" list visible from the README. No silent failures.

## 3. Switch test

**Method.** After a developer has installed Actant and used it for a week, ask one question:

> "What would make you remove this after one week?"

**Pass.** Answers cluster around:

- Bugs in capture / Studio / replay.
- Performance — captures slow down the agent.
- Missing integration — "I moved to Convex and you don't support it yet."
- Missing tool kind — "I use a custom MCP tool you don't capture."

These are addressable failure modes. They mean the developer wants Actant to work and has specific reasons it currently doesn't.

**Fail.** Answers cluster around:

- "I don't know why I need it."
- "Looks cool but I never opened Studio."
- "It works, but my team already has tracing."

These are positioning failures, not product failures. No amount of bug-fixing will save Actant from "looks cool but I never opened Studio." That's the cue to narrow the wedge further or reposition.

## 4. The HN test

**Method.** When the project goes public, watch for the inevitable "how is this different from Mastra + Convex?" comment. Reply with the [`positioning.md`](./positioning.md) HN objection answer.

**Pass.** The thread continues with people asking install questions, sharing use cases, or pushing back on specifics. Net: more curiosity than dismissal.

**Fail.** The thread converges on "this is just a wrapper around things that already exist." Or: silence.

**Rule.** Don't post to HN until tests 1–3 have all passed at least once. A premature HN launch with a fail-pattern README is a category-defining mistake.

## 5. The demo test

**Method.** After the killer demo is recorded ([`killer-demo.md`](./killer-demo.md)), show the 90-second video to 10 working agent developers. Then ask:

> "What did Actant do that you couldn't do without it?"

**Pass.** They name **at least one** of:

- "I could replay the run without that bad memory."
- "Guard intercepted the destructive command and let me approve a safer variant."
- "I could see exactly what the model saw before it proposed the command."

**Fail.** Best answer is "it looked nice" or "it had a timeline."

## When the wedge is real

When the cold README test passes (≥5 install requests, ≥3 this-week intent, ≥2 named pains), the install test passes (≥7/10 successful installs), and the switch test answers are addressable failures rather than positioning failures — the wedge has earned the right to grow.

Until then, **no scope additions**. See [`anti-scope.md`](./anti-scope.md).
