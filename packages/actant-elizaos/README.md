# @actantdb/elizaos

Named adapter for elizaOS actions and plugin-shaped runtimes.

```ts
import { withActantElizaAction } from "@actantdb/elizaos";

export const sendReply = withActantElizaAction(
  {
    name: "SEND_REPLY",
    description: "Send a reply to the current room.",
    handler: async (runtime, message) => {
      return { ok: true, text: message.content.text };
    },
  },
  { project: "my-eliza-agent" },
);
```

The wrapper records an ActantDB run, user-message event when the first argument
looks like an elizaOS message, tool-call requested/started/completed events,
and a finished run event. It does not import elizaOS, so it can sit next to
whatever elizaOS version the host app already uses.

```ts
import { createActantElizaPlugin } from "@actantdb/elizaos";

export const actantPlugin = createActantElizaPlugin({
  project: "my-eliza-agent",
  actions: [sendReply],
});
```
