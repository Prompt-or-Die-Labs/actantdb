---
description: Run format checks, workspace lints, spec and agent verifications, and E2E smoke tests.
---

// turbo-all

1. Run Rust formatting check:
   cargo fmt --all -- --check
2. Run Rust workspace clippy lints:
   cargo clippy --workspace --all-targets -- -D warnings
3. Verify all specs have validation sections:
   just verify-specs
4. Verify all agents conform to rules:
   just verify-agents
5. Build all TS packages:
   pnpm -r build
6. Run the end-to-end smoke test suite:
   pnpm smoke
