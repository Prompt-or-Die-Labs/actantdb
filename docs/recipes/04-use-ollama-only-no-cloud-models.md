# 04 — Use Ollama only — no cloud models

The whole point of an embedded ledger is that nothing leaves the machine.
Pair that with a local model runner (Ollama, llama.cpp, LM Studio) and
the entire agent loop runs on your laptop.

## Sanity check first

ActantDB does not call models on your behalf. The wrapper observes what
*your* code sends. So "no cloud" comes down to:

1. Use a local model runner.
2. Tell your agent framework about it.
3. Wire a policy `sensitivity_ceiling` so the firewall blocks any
   accidental dispatch to a cloud model in the future.

## Local Ollama

Install: `brew install ollama` (or [ollama.com/download](https://ollama.com/download)).
Pull a model: `ollama pull llama3.2:8b`.

```js
// agent.mjs
import { withActant } from "@actantdb/mastra";

async function ollamaCall(prompt) {
  const r = await fetch("http://127.0.0.1:11434/api/generate", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ model: "llama3.2:8b", prompt, stream: false }),
  });
  if (!r.ok) throw new Error(`ollama: ${r.status}`);
  const body = await r.json();
  return body.response;
}

const agent = {
  name: "local-agent",
  tools: {},
  generate: async ({ message }) => ollamaCall(message),
};

const wrapped = withActant(agent, { project: "ollama-only" });
const ctx = wrapped.startRun();
ctx.recordUserMessage("Summarize this file in one sentence.");
const answer = await agent.generate({ message: "Summarize this file in one sentence." });
ctx.recordModelCall({
  model: "ollama:llama3.2:8b",
  role: "generator",
  prompt_hash: "h",
  summary: answer.slice(0, 80),
});
ctx.finish({ ok: true });
wrapped.actant.close();
```

## Block cloud dispatch with a policy ceiling

Use `sensitivity_ceiling` to refuse to send anything labeled `secret`
through any tool. This is enforced at Guard time, before the tool call
fires:

```js
import { withActant } from "@actantdb/mastra";

const policy = {
  sensitivity_ceiling: "low",     // anything > low is blocked
  tools: [],
  deny: [
    {
      tool: "openai_completion",
      pattern: ".*",
      reason: "no cloud completions on this project",
    },
    {
      tool: "anthropic_completion",
      pattern: ".*",
      reason: "no cloud completions on this project",
    },
  ],
};

const wrapped = withActant(agent, { project: "ollama-only", policy });
```

Any attempt to invoke a cloud model now writes `guard_verdict
{decision: "block"}` to the ledger and does not call the tool.

## Confirm nothing leaks

Run the agent, then query the ledger for outbound calls. If any
unexpected tool was attempted, it's right there in the audit trail:

```js
const blocked = wrapped.actant.ledger
  .query({ kind: "guard_verdict" })
  .filter((e) => e.payload.decision === "block");

if (blocked.length) {
  console.log(`${blocked.length} cloud calls blocked:`);
  for (const b of blocked) console.log(" -", b.payload.reason);
}
```

## Open Studio offline

Studio is a local Node process. No network access required:

```bash
ACTANTDB_STORE_DIR=./.actantdb npx actantdb studio --project ollama-only
```

## See also

- [Recipe 01](./01-add-approval-to-a-tool.md) — gate any tool you don't trust the model with.
- [Recipe 06](./06-export-to-bigquery.md) — if you change your mind later and *do* want analytics.
