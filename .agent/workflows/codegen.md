---
description: Run Rust contract check-compat, TS type codegen, and pnpm rebuild.
---

// turbo-all

1. Run compatibility checks on Rust contracts:
   cargo run -p actant-contracts -- check-compat
2. Generate TypeScript bindings from contracts:
   cargo run -p actant-contracts -- codegen-ts
3. Build all TypeScript packages to apply the changes:
   pnpm -r build
