# ActantDB

The local-first accountability backend for agents.

This documentation is generated from the canonical specs under [`/specs`](https://github.com/Prompt-or-Die-Labs/actantdb/tree/main/specs) and the operational docs at the repo root (CHANGELOG, GATES, RELEASE_CHECKLIST, SPECS_STATUS).

Build locally with:

```bash
mdbook build docs
open docs/book/index.html
```

## Start here

- [Golden quickstart](./golden-quickstart.md): scaffold, run, open Studio,
  and check the embedded ledger.
- [Interactive playground](./playground.md): browser-only walkthrough of a
  captured run, authority decision, and replay diff.

## What's in here

- **Specs 00–19**: the canonical product surface. Each spec ships a
  `## Verification` section that is enforced by a Rust test under
  `crates/<crate>/tests/spec_NN_verification.rs`. Run focused crate tests
  locally and leave full-workspace test parity to CI.
- **SLOs**: production targets the v1 substrate is held to.
- **Gates**: repository-verifiable quality gates only.
- **Release checklist**: package and binary release operations.
- **SPECS_STATUS**: which specs are verified and what the verifiers assert.

## Quick start

```bash
npm create actantdb@latest my-agent -- --template minimal --framework hand-rolled --language js --yes
cd my-agent
npm install
npm start
npm run studio
npm run doctor
```

This path uses the embedded SQLite ledger. No server, Docker, hosted service, or
model API key is required.
