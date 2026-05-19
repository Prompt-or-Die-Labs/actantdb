# Issues to file — DX gaps from 2026-05-18 trial

Four product gaps surfaced in the trial-2 run of `/tmp/actantdb-real-project` against `@actantdb/*@0.0.4`. Drafted but not filed (deferred for explicit OK). Paste each into `gh issue create --title "..." --body @-` or the GitHub UI.

---

## 1. policy DSL: support numeric comparators (amount > N)

### What I tried
Built a real support agent against `@actantdb/mastra@0.0.4` that wanted to require approval for any refund > $100.

### What broke
The Policy DSL only supports `pattern` (regex over serialized arg JSON) and per-tool default-risk. There's no way to express **"this numeric field is over N"** as an approval trigger.

### Workaround
Currently you have to either:
- pre-process args in the agent itself, or
- use a regex that brittle-matches numbers in the serialized JSON.

### Proposed fix
Extend the policy DSL's deny + require_approval branches with a small JSON-path + comparator vocabulary:

```ts
{
  tool: "issue_refund",
  path: "$.amount",      // limited jsonpath, like actant-eval supports
  op: "gt",              // gt | gte | lt | lte | eq | ne
  value: 100,
  decision: "require_approval",
  reason: "refunds over $100 need a human",
}
```

The same path/op/value triple is already implemented in `actant-eval`'s success-criteria DSL, so most of the work is bridging the two.

### Source for the trial
`/tmp/actantdb-real-project/agent.mjs` — happy to attach if useful.

---

## 2. replay: policy override doesn't propagate through runFromEvent

### What I tried
Replay a captured run with a stricter policy to demonstrate the "what if Guard had been tougher" workflow the README pitches.

```js
const replay = runFromEvent({
  ledger,
  eventId,
  // overrides: ...      ← only memory-related overrides documented
  // policy: tighten(...) ← exported but the README example never wires it through
});
const diff = diffReplayAgainstOriginal(ledger, replay);
```

### What broke
Even with `tighten(demoPolicy, { deny: [...] })` passed, the diff comes back with every entry tagged `identical`. The override doesn't change verdicts during replay.

### Expected
The README's "hero diff" section explicitly shows `require_approval (constrain) → allow` flipping under a stricter policy. That capability needs a tested path through `runFromEvent` + `diffReplayAgainstOriginal`.

### Repro
`/tmp/actantdb-real-project/replay-override.mjs`.

---

## 3. docs: per-framework prerequisites for @actantdb/mastra

### What I tried
README says `@actantdb/mastra` works with Mastra, LangGraph, OpenAI Agents SDK, or hand-rolled agents. Tried to build a hand-rolled one.

### What I hit
- The synthetic `model_call` event carries `"model":"user-provided"` + `"role":"planner"` for agents without their own model-call surface. A fresh user has no idea what those mean or how to override.
- For real Mastra usage, you need `npm install @mastra/core` separately. For LangGraph: `@langchain/langgraph`. For OpenAI Agents: `@openai/agents`. None of this is in the README.

### Fix
One short subsection per supported framework: how to install, what shape the agent must expose, what the wrapper does in each case. Could live in `packages/actant-mastra/README.md` or in the top-level README's "Install" section.

### Bonus
If we're going to claim cross-framework support, add at least one integration test per framework to keep the claim honest.

---

## 4. core: in-memory ledger mode for testing

### What I want
```js
const ledger = openLedger({ project: "test", inMemory: true });
```

Useful for:
- Unit tests that don't want to touch the filesystem.
- Sharing one in-memory ledger between an agent under test and Studio in the same process.
- CI environments where the default `~/.actantdb` path doesn't exist or isn't writable.

### Current state
`openLedger` requires a path. Tests work around this by writing to `tempfile` directories, which is fine until you want Studio to read the same in-memory store.

### Implementation
`node:sqlite` supports `:memory:`. The `Ledger` constructor would need to:
- Accept `inMemory: true` and skip the `mkdirSync` + path-derivation.
- Pass `:memory:` to `new DatabaseSync(":memory:")`.
- The same in-process `Ledger` instance is safe to share; cross-process in-memory sharing isn't.
