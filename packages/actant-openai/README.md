# @actantdb/openai

Drop-in replacement for the [`openai`](https://www.npmjs.com/package/openai)
package. Same constructor, same `client.chat.completions.create({...})` and
`client.responses.create({...})` shape. Every call also lands as a typed
`model_call` event in the ActantDB ledger.

You keep OpenAI. You add Actant.

## Install

```bash
npm install @actantdb/openai openai
```

## Use

```ts
import OpenAI from "@actantdb/openai";

const client = new OpenAI({
  apiKey: process.env.OPENAI_API_KEY,
  actant: { project: "my-app", storeDir: "./.actantdb" },
});

const completion = await client.chat.completions.create({
  model: "gpt-4o",
  messages: [{ role: "user", content: "Hello, world." }],
});
```

`completion` is byte-for-byte the upstream response. A `model_call` event is
also recorded. Omit the `actant` field for a transparent passthrough.

## What gets recorded

| Event        | What it records                                              |
| ------------ | ------------------------------------------------------------ |
| `model_call` | `model`, `role=generator`, `prompt_hash`, `summary`, `tokens_in`, `tokens_out` |

Both `chat.completions.create` and `responses.create` are wrapped. Other
properties (`client.beta`, `client.embeddings`, error classes, etc.) are
forwarded verbatim via `Proxy`.

## Migration

```diff
- import OpenAI from "openai";
+ import OpenAI from "@actantdb/openai";

  const client = new OpenAI({
    apiKey: process.env.OPENAI_API_KEY,
+   actant: { project: "my-app", storeDir: "./.actantdb" },
  });
```

## License

Apache 2.0.
