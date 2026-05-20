# Runtime Guidance

## Supported Today

| Runtime | Use | Status |
| --- | --- | --- |
| Node 24+ | Embedded `@actantdb/core`, adapters, Studio CLI | First-class. `node:sqlite` is unflagged. |
| Node 22.5+ | Embedded `@actantdb/core`, adapters, Studio CLI | Supported when `node:sqlite` is available in the runtime. |
| Bun 1.3+ | Embedded `@actantdb/core`, Bun workspace scripts, and generated `create-actantdb --runtime bun` apps | Supported through `bun:sqlite`; covered by `bun run ci:bun` and `pnpm smoke:bun-create`. |
| Python 3.9+ | HTTP SDK, async facade, LangChain/CrewAI/AutoGen helpers | Supported through `sdks/python`; no embedded ledger. |
| Swift / Rust | HTTP SDKs and Rust crates | Supported in tree; see `sdks/` and `crates/`. |
| Docker / Linux server | `actantdb-server` over HTTP/WebSocket | Supported with SQLite-backed server storage. |

## Explicit Boundaries

| Runtime | Guidance |
| --- | --- |
| Deno | Use HTTP against `actantdb-server`; the embedded Node package is not a Deno package. |
| Cloudflare Workers / Vercel Edge / browser | Use HTTP from a server-side proxy or worker-safe client. Embedded storage is not supported because there is no `node:sqlite` filesystem path. |
| Postgres | `PgStorage` and `Engine::postgres` support storage + command-engine use from Rust. `actantdb-server` is still SQLite-only and refuses `ACTANTDB_DATABASE_URL`. |

## Rule Of Thumb

Use embedded `@actantdb/core` only where Node can open `node:sqlite` or Bun can
open `bun:sqlite` on a real filesystem. Everywhere else, run `actantdb-server`
and connect through the HTTP SDK for that language.

## Maintainer Toolchains

`pnpm` remains the canonical publish and contributor path. Bun is a checked
second lane: `bun install --frozen-lockfile`, `bun run build:bun`,
`bun run test:bun`, and `bun run smoke:bun-create:bun` exercise the same
workspace with Bun's installer, script runner, packer, and runtime while keeping
Vitest as the TypeScript test runner.
