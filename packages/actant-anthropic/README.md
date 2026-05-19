# @actantdb/anthropic

Drop-in replacement for [`@anthropic-ai/sdk`](https://www.npmjs.com/package/@anthropic-ai/sdk).
Same constructor, same `client.messages.create({...})` shape. Every call also
lands as a typed `model_call` event in the ActantDB ledger.

You keep Anthropic. You add Actant.

## Install

```bash
npm install @actantdb/anthropic @anthropic-ai/sdk
```

## Use

```ts
import Anthropic from "@actantdb/anthropic";

const client = new Anthropic({
  apiKey: process.env.ANTHROPIC_API_KEY,
  actant: { project: "my-app", storeDir: "./.actantdb" },
});

const msg = await client.messages.create({
  model: "claude-sonnet-4-5",
  max_tokens: 256,
  messages: [{ role: "user", content: "Hello, world." }],
});
```

`msg` is byte-for-byte the upstream response. A `model_call` event is also
recorded. Omit the `actant` field for a transparent passthrough.

## What gets recorded

| Event        | What it records                                              |
| ------------ | ------------------------------------------------------------ |
| `model_call` | `model`, `role=generator`, `prompt_hash`, `summary`, `tokens_in`, `tokens_out` |

The wrapper opens an ad-hoc run per `.create()` call unless you supply
`actant.run` (a `RunContext` from `@actantdb/core` / `@actantdb/mastra`),
in which case events land on that run.

## Migration

```diff
- import Anthropic from "@anthropic-ai/sdk";
+ import Anthropic from "@actantdb/anthropic";

  const client = new Anthropic({
    apiKey: process.env.ANTHROPIC_API_KEY,
+   actant: { project: "my-app", storeDir: "./.actantdb" },
  });
```

All other properties (`client.beta`, `client.completions`, error classes,
etc.) are forwarded verbatim via `Proxy`.

## License

Apache 2.0.
