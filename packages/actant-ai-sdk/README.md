# @actantdb/ai-sdk

Vercel AI SDK adapter for ActantDB. Wrap `streamText` / `generateText` /
`generateObject` so every model call and every tool call lands as typed
events in the ledger — with full Guard + approval enforcement.

You keep the AI SDK. You add Actant.

## Install

```bash
npm install @actantdb/ai-sdk ai
```

## Use

```ts
import { wrapAiSdk } from "@actantdb/ai-sdk";
import { openai } from "@ai-sdk/openai";

const wrapped = wrapAiSdk({ project: "my-app", storeDir: "./.actantdb" });

const result = await wrapped.generateText({
  model: openai("gpt-4o"),
  messages: [{ role: "user", content: "Clean the build dir." }],
  tools: {
    "shell.run": {
      description: "Run a shell command",
      execute: async ({ command }) => runShell(command),
    },
  },
});
```

`wrapped.generateText` / `streamText` / `generateObject` forward to the
upstream `ai` package after wrapping every `tools[*].execute` with the same
Guard + approval logic as `@actantdb/mastra`.

## What gets recorded

| Event                  | What it records                                     |
| ---------------------- | --------------------------------------------------- |
| `agent_run_started`    | run id + source metadata                            |
| `model_call`           | model id, role, prompt hash, summary                |
| `tool_call_requested`  | tool name + args (with secret redaction) + risk     |
| `guard_verdict`        | allow / constrain / require_approval / block / halt |
| `approval_required`    | request payload + reason                            |
| `approval_decision`    | approver + scope + decision                         |
| `tool_call_started`    | sealed args after Guard                             |
| `tool_call_completed`  | status + result + duration                          |
| `agent_run_finished`   | success/failure summary                             |

## Migration

```diff
- import { generateText } from "ai";
+ import { wrapAiSdk } from "@actantdb/ai-sdk";
+ const wrapped = wrapAiSdk({ project: "my-app", storeDir: "./.actantdb" });

- const result = await generateText({
+ const result = await wrapped.generateText({
    model,
    messages,
    tools,
  });
```

## License

Apache 2.0.
