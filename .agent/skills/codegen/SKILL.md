---
name: actantdb-codegen
description: Automatically check compatibility and regenerate TypeScript bindings from Rust contracts when contract types change.
---
# ActantDB Codegen Skill

Use this skill when you modify any public types or contracts in `crates/actant-contracts/`.

## Instructions

1. **Check Compatibility first**:
   Run the contract compatibility check before doing any code generation:
   ```bash
   cargo run -p actant-contracts --bin actant-contracts -- check-compat
   ```
2. **Regenerate TypeScript Types**:
   Run the codegen command:
   ```bash
   cargo run -p actant-contracts --bin actant-contracts -- codegen-ts
   ```
3. **Commit Together**:
   Ensure both the Rust contract changes and the regenerated TS types under `packages/actant-types/src/generated/*` are committed in the same PR/commit.

## Critical Rules
- Do NOT hand-edit anything under `packages/actant-types/src/generated/`.
