# ActantDB Binding and Type Rules

1. **Single Source of Truth**:
   The single source of truth for every public type is `crates/actant-contracts/`.
2. **No New Public Types**:
   Never introduce any public type outside of `actant-contracts`. Edit `crates/actant-contracts/src/lib.rs` first.
3. **No Hand-editing Generated Code**:
   Do NOT hand-edit anything under `packages/actant-types/src/generated/*`. They are regenerated via `cargo run -p actant-contracts --bin actant-contracts -- codegen-ts`.
