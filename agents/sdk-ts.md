# Work package: `sdks/ts` — `@actantdb/client`

## Context

The first SDK ships with Phase 1 because Studio depends on it. Implement what's in `/planning/sdk-ts.md`.

## Specs to read first

- `/specs/09-sdk-design.md` §1–§8.
- `/planning/sdk-ts.md`.
- `/specs/08-api-spec.md` for the wire shape.

## Scope

### Layout

```
sdks/ts/
├── package.json                (ESM only)
├── tsconfig.json               (strict)
├── tsup.config.ts              (build config)
├── README.md
├── src/
│   ├── index.ts
│   ├── client.ts               (ActantClient)
│   ├── transport/
│   │   ├── http.ts
│   │   └── ws.ts
│   ├── subscribe.ts            (async iterator)
│   ├── errors.ts               (ActantCommandError, ActantTransportError)
│   ├── auth.ts
│   ├── idempotency.ts
│   └── generated/              (codegen output; never hand-edit)
│       ├── commands.ts
│       └── tables.ts
└── tests/
```

### Generated surface

Each command becomes a method on `client.command`:

```ts
client.command.createSession(input: CreateSessionInput): Promise<CreateSessionResult>;
```

Type names match `JsonSchema` titles (camelCased).

### Tests

- Unit: every transport call mocked; verify request shape + error mapping.
- Integration: spin up `actantdb-server` in CI service container; round-trip the alpha command set.
- Subscription: snapshot → upserts → unsubscribe round-trip.
- Backpressure: a slow consumer receives `lag` and re-snapshots.

## Acceptance criteria

- [ ] `pnpm install && pnpm build && pnpm test` green.
- [ ] Published bundle < 60 KB gzipped (excluding generated types).
- [ ] `tsc --strict --noEmit` clean.
- [ ] Public API has full JSDoc.

## Do NOT

- Do NOT bundle CommonJS in Phase 1.
- Do NOT silently retry command calls.
- Do NOT depend on a runtime — must work in Node, Bun, Deno, browsers.

## Hand-off

`pnpm ci`. Then run the alpha demo from `/specs/10-alpha-demo.md` driven by this SDK.
