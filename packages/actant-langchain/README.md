# @actantdb/langchain

LangChain JS callback handler for ActantDB. Plugs into any LangChain
runnable, chain, agent, or chat model via the standard
`BaseCallbackHandler` interface, and records every LLM, tool, and chain
event to the ActantDB ledger.

You keep LangChain. You add Actant.

## Install

```bash
npm install @actantdb/langchain @langchain/core
```

## Use

```ts
import { ActantCallbackHandler } from "@actantdb/langchain";
import { ChatAnthropic } from "@langchain/anthropic";

const handler = new ActantCallbackHandler({
  project: "my-app",
  storeDir: "./.actantdb",
});

const chat = new ChatAnthropic({
  callbacks: [handler],
});

await chat.invoke("hello, world");
```

Or attach it per-invoke:

```ts
await chain.invoke(input, { callbacks: [handler] });
```

## What gets recorded

| LangChain callback     | ActantDB event              |
| ---------------------- | --------------------------- |
| `handleChainStart`     | `agent_run_started`         |
| `handleLLMStart`       | (frame opened)              |
| `handleChatModelStart` | (frame opened)              |
| `handleLLMEnd`         | `model_call`                |
| `handleLLMError`       | `model_call` (ERROR summary)|
| `handleToolStart`      | `tool_call_requested` + `tool_call_started` |
| `handleToolEnd`        | `tool_call_completed` (ok)  |
| `handleToolError`      | `tool_call_completed` (error) |
| `handleChainEnd`       | `agent_run_finished` (ok)   |
| `handleChainError`     | `agent_run_finished` (error)|

Token usage from `llmOutput.tokenUsage` (legacy) and
`message.usage_metadata` (newer) is extracted into the `model_call` event
when present.

## Migration

```diff
+ import { ActantCallbackHandler } from "@actantdb/langchain";
+ const handler = new ActantCallbackHandler({ project: "my-app", storeDir: "./.actantdb" });

  const chat = new ChatAnthropic({
+   callbacks: [handler],
  });
```

## License

Apache 2.0.
