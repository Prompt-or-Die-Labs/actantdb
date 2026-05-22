# RELEASE_CHECKLIST.md — artifact release checklist

This checklist contains only release work that can be verified from the repo,
CI, package registries, or release assets. Market-facing outcomes are
intentionally out of scope.

## Pre-release gates

- [x] `pnpm -r build`
- [x] `pnpm -r test`
- [x] `pnpm smoke`
- [x] `just verify-specs`
- [x] `just verify-agents`
- [x] `cargo run -p actant-contracts --bin actant-contracts -- check-compat`
- [x] CI workflow exists for format, lint, tests, spec verification, and agent verification
- [x] Three runnable examples exist: [`examples/test-cleanup/`](./examples/test-cleanup), [`examples/langgraph-router/`](./examples/langgraph-router), [`examples/cli-only/`](./examples/cli-only)
- [x] CI publish workflow exists: [`.github/workflows/publish-npm.yml`](./.github/workflows/publish-npm.yml)
- [x] CI binary-release workflow exists: [`.github/workflows/release-binaries.yml`](./.github/workflows/release-binaries.yml)

## npm package release

The workspace packages are versioned at `0.0.15`.

```bash
# GitHub Actions -> publish-npm -> Run workflow
# Defaults: tag=latest, also_tag_shadow=true, dry_run=false.
```

Verify after the workflow completes:

```bash
npm view @actantdb/all version
npm view @actantdb/mastra version
npm view @actantdb/studio version

mkdir /tmp/actantdb-check && cd /tmp/actantdb-check && npm init -y > /dev/null
npm install @actantdb/mastra
node -e "import('@actantdb/mastra').then(m => console.log(typeof m.withActant))"
```

Expected: all versions match the release version and the final command prints
`function`.

## Binary release

For a tagged binary release:

```bash
git tag v0.0.15
git push origin v0.0.15
```

Or use GitHub Actions -> `release-binaries` -> Run workflow.

Verify after the workflow completes:

```bash
gh release view v0.0.15
gh release download v0.0.15 --pattern '*actantdb*' --dir /tmp/actantdb-release-check
```

Expected: release assets include `actantdb` and `actantdb-server` binaries for
the configured platforms.

## Compatibility release gate

Before publishing any release that changes `crates/actant-contracts`, run:

```bash
cargo run -p actant-contracts --bin actant-contracts -- check-compat
cargo run -p actant-contracts --bin actant-contracts -- codegen-ts
git diff --exit-code packages/actant-types/src/generated
```

Expected: `check-compat` passes, generated files are already current, or the
generated diff is committed with the contract change.

## Rollback rule

If a publish or binary release fails after artifacts become visible, do not
rewrite history. Publish a follow-up patch version and document the failed
artifact in the release notes.
