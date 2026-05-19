# @actantdb/openai-agents

Adapter for OpenAI's [`@openai/agents`](https://www.npmjs.com/package/@openai/agents)
SDK. Wrap any `Agent` instance and capture every model call, tool call,
approval, and effect to the ActantDB ledger.

You keep `@openai/agents`. You add Actant.

## Install

```bash
npm install @actantdb/openai-agents @openai/agents
```

## Use

```ts
import { Agent, tool } from "@openai/agents";
import { withActantAgent } from "@actantdb/openai-agents";

const myAgent = new Agent({
  model: "gpt-4o",
  instructions: "You clean test artifacts.",
  tools: [
    tool({
      name: "shell.run",
      execute: async ({ command }) => runShell(command),
    }),
  ],
});

const wrapped = withActantAgent(myAgent, {
  project: "support-bot",
  storeDir: "./.actantdb",
  autoApprove: true,
});

const { result, runId } = await wrapped.run({ input: "clean it up" });
```

## What gets recorded

| Event                  | What it records                                     |
| ---------------------- | --------------------------------------------------- |
| `agent_run_started`    | run id, agent name, input                           |
| `user_message_received`| `message` if you pass `{ message }`                 |
| `model_call`           | planner model + summary                             |
| `tool_call_requested`  | tool name + args + risk                             |
| `guard_verdict`        | allow / constrain / require_approval / block / halt |
| `approval_required`    | request payload + reason                            |
| `approval_decision`    | approver + scope + decision                         |
| `tool_call_started`    | sealed args                                         |
| `tool_call_completed`  | status + result + duration                          |
| `agent_run_finished`   | success/failure summary                             |

## Migration

```diff
  import { Agent } from "@openai/agents";
+ import { withActantAgent } from "@actantdb/openai-agents";

  const myAgent = new Agent({ model: "gpt-4o", tools: [...] });
+ const wrapped = withActantAgent(myAgent, {
+   project: "support-bot",
+   storeDir: "./.actantdb",
+ });

- const result = await myAgent.run("hi");
+ const { result } = await wrapped.run({ input: "hi" });
```

## Caveats

The `@openai/agents` API surface is still evolving. The wrapper detects
both `tool.invoke()` and `tool.execute()` and wraps whichever the
upstream uses. If you hit a tool shape we don't recognise, please open
an issue with a reproduction.

## License

Apache 2.0.
