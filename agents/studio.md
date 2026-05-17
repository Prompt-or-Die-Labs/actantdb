# Work package: `studio/`

## Context

Actant Studio: the dashboard product. Lives under `studio/` at the repository root (not inside `crates/` — it is a TypeScript / React app, not Rust). Phase 1 ships a minimal version (Chat, Approval Center, Audit Trail, Memory Review). Phases 2 → 6 add screens. The full screen catalog and design constraints live in `/planning/studio-design.md`.

## Specs to read first

- `/planning/studio-design.md` — full design.
- `/specs/08-api-spec.md` §5 (subscription contract).
- `/specs/09-sdk-design.md` §8 (TypeScript SDK ergonomics).

## Scope

### Phase 1 deliverables

```
studio/
├── package.json                (vite + react + ts; pinned versions)
├── tsconfig.json               (strict mode)
├── tailwind.config.ts
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── routes/
│   │   ├── index.tsx           (Chat)
│   │   ├── approvals.tsx       (Approval Center)
│   │   ├── audit.tsx           (Audit Trail)
│   │   └── memory.tsx          (Memory Review)
│   ├── components/             (shared UI; shadcn/ui re-exports here)
│   ├── lib/
│   │   ├── client.ts           (the SDK instance)
│   │   └── format.ts           (timestamps, sensitivity badges)
│   └── styles/index.css
└── README.md
```

### Phase 2-6

Add screens per `/planning/studio-design.md`. Every new screen:

- has its own route under `src/routes/`,
- gets a corresponding test file under `src/routes/__tests__/`,
- uses only the generated SDK types (no hand-written wire types).

### Interaction rules (binding)

- All mutations go through the SDK (which goes through the command API).
- No optimistic UI in Phase 1–5. Phase 6 may revisit per screen.
- Risk-tier-aware confirmation: `medium`+ approvals require a typed confirmation; `critical` shows the policy_snapshot summary.

## Acceptance criteria

- [ ] `pnpm install && pnpm build` succeeds.
- [ ] `pnpm test` passes.
- [ ] Lighthouse perf score ≥ 90 on Approval Center loaded with 500 rows.
- [ ] WCAG 2.1 AA audit clean for the four Phase 1 routes.
- [ ] Subscription drop / reconnect / re-snapshot exercised in an integration test.

## Do NOT

- Do NOT introduce a new state framework (Zustand, Jotai, Redux). React + SDK only in Phase 1.
- Do NOT bundle the SDK source; depend on the published `@actantdb/client` package.
- Do NOT use cookies for auth. Bearer tokens in `Authorization` header.

## Hand-off

`pnpm ci` plus a manual run-through of the four Phase 1 routes against `actantdb-server`.
