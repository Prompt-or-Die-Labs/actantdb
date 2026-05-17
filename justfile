# actantDB — common dev commands. Install `just`: https://just.systems
# Run `just --list` to see all recipes.

default:
    @just --list

# Compile everything.
build:
    cargo build --workspace --all-targets

# Fast compile-check (no codegen). Use during scaffolding.
check:
    cargo check --workspace --all-targets

# Run all tests.
test:
    cargo test --workspace --all-targets

# Format everything.
fmt:
    cargo fmt --all

# Format check (CI mode).
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Apply migrations to a local dev database.
migrate db="./actant.dev.sqlite":
    @echo "Migrations will run via actant-storage; see crates/actant-storage."
    @echo "Phase 1: actantdb migrate --db {{db}}"

# Run the server with verbose logs.
serve:
    RUST_LOG=actant=debug cargo run -p actant-server --bin actantdb-server

# Spec verification: every specs/*.md must contain a Verification section.
# STATUS.md is a freeze marker, not a spec — skip it.
verify-specs:
    @for f in specs/*.md; do \
        [ "$(basename "$f")" = "STATUS.md" ] && continue; \
        grep -q '^## Verification' "$f" || { echo "missing Verification in $f"; exit 1; }; \
    done
    @echo "all specs OK"

# Agent-package lint: every agents/actant-*.md must have the required sections.
verify-agents:
    @for f in agents/actant-*.md; do \
        for s in "## Context" "## Scope" "## Specs to read first" "## Acceptance criteria" "## Do NOT"; do \
            grep -qF "$s" "$f" || { echo "missing '$s' in $f"; exit 1; }; \
        done; \
    done
    @echo "all agent packages OK"

# Full CI parity (run locally before pushing).
ci: fmt-check lint test verify-specs verify-agents
