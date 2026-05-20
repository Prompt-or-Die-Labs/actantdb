# actantdb

The all-in-one ActantDB import. One package, every primitive, zero boilerplate.

```bash
npm install @actantdb/all
```

```js
import { openLedger, evaluate, withActant } from "actantdb";

const ledger = openLedger("my-project");
const wrapped = withActant(myAgent, { ledger });
```

That's it. Every public symbol from `@actantdb/core`, `@actantdb/policy`,
`@actantdb/mastra`, `@actantdb/replay`, `@actantdb/sdk`, `@actantdb/elizaos`,
and `@actantdb/types` is re-exported.

## Just want one piece?

Install only what you need. Each individual package is leaner:

| Need | Install | Size |
| --- | --- | --- |
| Just the embedded ledger | `npm i @actantdb/core` | ~150 KB |
| Just Guard + policy DSL | `npm i @actantdb/policy` | ~40 KB |
| Just the agent wrapper | `npm i @actantdb/mastra` | ~80 KB |
| Just the elizaOS adapter | `npm i @actantdb/elizaos` | ~40 KB |
| Just replay + diff | `npm i @actantdb/replay` | ~60 KB |
| Just the HTTP/WS client (server mode) | `npm i @actantdb/sdk` | ~70 KB |
| Everything | `npm i @actantdb/all` | bundles the above |

API is identical whether you use the umbrella or the individual packages —
this package is *only* re-exports.
