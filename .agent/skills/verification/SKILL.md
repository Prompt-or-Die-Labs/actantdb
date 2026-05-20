---
name: actantdb-verify
description: Verify code styling, formatting, lint check, spec enforcement, unit tests, and smoke tests before committing or pushing.
---
# ActantDB Verification Skill

Use this skill to run tests and verify the code is fully functional and compliant with guidelines before finalizing a task.

## Instructions

1. **Format Check**:
   ```bash
   just fmt-check
   ```
2. **Lint Checks**:
   ```bash
   just lint
   pnpm -r lint
   ```
3. **Spec compliance**:
   ```bash
   just verify-specs
   ```
   Every markdown spec file in `specs/*.md` must contain a `## Verification` section.
4. **Agent docs compliance**:
   ```bash
   just verify-agents
   ```
   Every agent document in `agents/actant-*.md` must contain `## Context`, `## Scope`, `## Specs to read first`, `## Acceptance criteria`, and `## Do NOT` sections.
5. **Rust tests**:
   - Run specific crate tests to avoid low disk space issues:
     ```bash
     cargo test -p <crate_name>
     ```
   - Do NOT run `cargo test --workspace` on local machine to avoid disk space issues/crashes.
6. **TypeScript & Smoke tests**:
   ```bash
   pnpm install
   pnpm -r build
   pnpm -r test
   pnpm smoke
   ```
7. **Full CI validation**:
   ```bash
   just ci
   pnpm ci
   ```
