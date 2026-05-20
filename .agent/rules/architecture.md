# ActantDB Workspace Architecture Rules

- **TypeScript packages** live in `packages/`. Each package publishes as `@actantdb/<name>` and uses ESM only. Requires Node ≥ 22.5.
- **Rust crates** live in `crates/`. The server binary is `actant-server` (built as `actantdb-server`); the CLI binary is `actant-cli` (built as `actantdb`).
- **Default install path** is `npm install @actantdb/all`. Never add Rust toolchain instructions or Docker details to the default consumer install instructions.
