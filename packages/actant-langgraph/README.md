# @actantdb/langgraph

LangGraph package name for actantdb's existing `withActant` wrapper.

## Install

```bash
npm install @actantdb/langgraph @langchain/langgraph
```

## Use

```ts
import { withActant } from "@actantdb/langgraph";

const wrapped = withActant(graph, {
  project: "router-agent",
  storeDir: "./.actantdb",
});
```

The package is intentionally thin. It reuses the same duck-typed tool wrapper
as `@actantdb/mastra`: if your graph exposes
`tools: Record<string, { execute(args) }>` or your nodes call that registry,
Guard and the ledger see the same typed events.

`withLangGraph` is exported as an alias for teams that prefer a framework-named
entrypoint.

## License

Apache 2.0.
