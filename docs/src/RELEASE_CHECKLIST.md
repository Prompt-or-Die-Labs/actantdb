# RELEASE_CHECKLIST.md — the precise sequence to close Gates 2 + 3

This is the artifact that converts "ready" to "shipped". Every step below
is one the **user** runs (not the agent). The repo is staged such that each
step is a single command or a single short outreach.

Pre-conditions verified by the agent (current at HEAD):

- [x] `cargo test --workspace` → **331 passing, 0 failed**, 13 ignored
- [x] `pnpm -r test` → **25 passing**
- [x] `pnpm smoke` → green
- [x] `swift test --package-path sdks/swift` → **62 tests in 12 suites passing**
- [x] `(cd sdks/python && python3 -m unittest discover -s tests)` → **10 passing**, 1 skipped (integration test needs `ACTANTDB_TEST_URL`)
- [x] Three public examples exist: [`examples/test-cleanup/`](./examples/test-cleanup), [`examples/langgraph-router/`](./examples/langgraph-router), [`examples/cli-only/`](./examples/cli-only)
- [x] All 8 `@actantdb/*` packages published to npm at `0.0.6` (`latest` + `shadow` tags)
- [x] CI publish workflow: [`.github/workflows/publish-npm.yml`](./.github/workflows/publish-npm.yml) — `workflow_dispatch`, builds + tests + smoke + dry-run-publish + publish + tag-mirror
- [x] CI binary-release workflow: [`.github/workflows/release-binaries.yml`](./.github/workflows/release-binaries.yml) — tag-driven and manual; produces `actantdb` + `actantdb-server` for macOS-arm64, macOS-x64, linux-x64

---

## Step 1 — npm publish (DONE)

Eight `@actantdb/*` packages live on npm under `latest` and `shadow`
tags. Manual republish:

```bash
# Go to Actions → "publish-npm" → Run workflow.
# Defaults: tag=latest, also_tag_shadow=true, dry_run=false.
# Workflow installs, builds, tests, smokes, dry-runs the publish,
# then publishes and mirrors to shadow.
```

To verify externally:

```bash
mkdir /tmp/actantdb-check && cd /tmp/actantdb-check && npm init -y > /dev/null
npm install @actantdb/mastra
node -e "import('@actantdb/mastra').then(m => console.log(typeof m.withActant))"
# Expect: "function"
```

## Step 2 — Cold-README test outreach (Gate 2 §1)

Per [`README.md` §1](./README.md), send the
[root README](./README.md) — and only the README — to 15 working agent
developers. No call. No explanation.

Suggested target list (pick 15 across these pools; the agent cannot make
the picks):

- 5 from Mastra's Discord (look for users posting about production deployments).
- 5 from LangGraph's GitHub issues (people debugging tool-call flows).
- 5 from a personal network — anyone shipping an agent in 2025–2026.

Threshold to pass (PIVOT gate language):

- [ ] ≥ 5 ask for install instructions
- [ ] ≥ 3 say they would add it to an existing agent **this week**
- [ ] ≥ 2 name a current pain it would solve

Track replies in a spreadsheet (suggest: a Numbers / Google Sheet at
`gates/cold-readme-results.csv`, ungitted).

## Step 3 — 10-minute install test (Gate 2 §2)

Per `README.md` §2, give 10 developers a 10-minute install
script. The repo already contains one (verified against `0.0.6`):

```bash
npm install @actantdb/mastra
# Then wrap one of their agents:
import { withActant } from "@actantdb/mastra";
const wrapped = withActant(theirAgent, { project: "their-project", autoApprove: false });
# Then open Studio:
npx actantdb studio --project their-project
```

Threshold:

- [ ] 7/10 install in <10 minutes without help from you
- [ ] 5/10 capture a real agent run end-to-end
- [ ] 3/10 produce a replay or approval trace they can show someone

Every failure produces exactly one ticket against this repo — no silent
failures (`README.md` §2 "Iteration rule").

## Step 4 — Design partner conversion (Gate 2 §"adoption", Gate 3 §"named")

From the developers who passed §3, ask the switch-test question after one
week:

> "What would make you remove this after one week?"

If their answer is in the "addressable" cluster (bugs, missing
integration, performance), they're a viable design partner candidate.
Convert 2 of them into weekly-feedback design partners by Jul 31, 2026
(Gate 2 threshold).

By Aug 17, 2026 (Gate 3 threshold):

- [ ] 5 non-Wes devs have shipped or staged Actant in production / serious staging
- [ ] 1 named (publicly attributable) design partner

These two cannot be manufactured. They are the only remaining work after
the artifacts above ship.

## Step 5 — Public examples + screen recording (Gate 1 leftovers)

The agent built three demos under `examples/test-cleanup*`. To close Gate 1 leftovers
([GATES.md](./GATES.md)):

```bash
# Run each so a screen recorder can capture it.
pnpm --filter actant-demo-test-cleanup demo
pnpm --filter actant-demo-langgraph-router demo
pnpm --filter actant-demo-cli demo

# Then in a second terminal for each:
npx actantdb studio --project demo-test-cleanup --store-dir examples/test-cleanup/.actantdb
```

Record a 90-second screencast with QuickTime / OBS / asciinema (the agent
authored an asciinema cast at [`examples/test-cleanup/killer-demo.cast`](./examples/test-cleanup/killer-demo.cast) — playable
with `asciinema play`). Upload the cast or the video to YouTube /
asciinema.org / GitHub Releases.

## What is impossible without you

The agent cannot:

- Send emails, Discord messages, or GitHub mentions to developers.
- Land a named design partner (requires a human relationship).
- Verify on 10+ external machines that the install works (requires those
  machines to exist outside this sandbox).

Note: `npm publish` itself is no longer impossible — the
`publish-npm.yml` workflow uses the repo's `NPM_TOKEN` automation token
and runs from a manual trigger.

## Status at the time of this checklist

- **Gate 1** — implementation-complete; the four leftovers (screencast,
  hero PNG, three-platform-developer verification, public examples) are
  one human action each, with public examples already done.
- **Gate 2** — every prerequisite that lives in code is green; the
  threshold itself requires Steps 2–4 above. Step 1 (publish) is done.
- **Gate 3** — three runnable demos exist; threshold itself requires
  Steps 4–5.

Run Steps 2 → 3 → 4 → 5 in order, and the gates close.
