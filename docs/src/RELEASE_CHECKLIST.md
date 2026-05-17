# RELEASE_CHECKLIST.md — the precise sequence to close Gates 2 + 3

This is the artifact that converts "ready" to "shipped". Every step below
is one the **user** runs (not the agent). The repo is staged such that each
step is a single command or a single short outreach.

Pre-conditions verified by the agent (all green at the time of writing):

- [x] `cargo test --workspace` → 53 passing, 0 failures
- [x] `pnpm -r test` → 19 passing
- [x] `pnpm smoke` → green
- [x] Three public examples exist: [`wedge/demo/`](./wedge/demo), [`wedge/demo-langgraph/`](./wedge/demo-langgraph), [`wedge/demo-cli/`](./wedge/demo-cli)
- [x] Publish-ready tarballs in [`dist-publish/`](./dist-publish/) — every `@actantdb/*` package, install-verified into a fresh `npm install`-only sandbox

---

## Step 1 — Publish to npm (closes Gate 2 "publishable" prereq)

The tarballs are pre-built. The first publish bumps versions from
`0.0.1-pre` to `0.0.1` and sets the right tag.

```bash
# 0. Confirm you're on the right npm user.
npm whoami
# Expect: your npm user. If not: `npm login --scope=@actantdb`.

# 1. Verify the dry-run.
for p in actantdb-types actantdb-core actantdb-policy actantdb-replay actantdb-mastra actantdb-studio; do
  echo "=== $p ==="
  npm publish dist-publish/${p}-0.0.1-pre.tgz --dry-run
done

# 2. Real publish.
for p in actantdb-types actantdb-core actantdb-policy actantdb-replay actantdb-mastra actantdb-studio; do
  npm publish dist-publish/${p}-0.0.1-pre.tgz --tag pre --access public
done

# 3. Sanity-check the installable surface from outside the workspace.
mkdir -p /tmp/actant-check && cd /tmp/actant-check && npm init -y > /dev/null
npm install @actantdb/mastra@pre
node -e 'import("@actantdb/mastra").then(m => console.log(typeof m.withActant))'
# Expect: "function"
```

**Why this matters for Gate 2:** the gate threshold is "10 non-Wes
developers installed". Until `npm install @actantdb/mastra` actually works
from a stranger's machine, the gate cannot close on anyone's effort but
yours.

---

## Step 2 — Cold-README test outreach (Gate 2 §1)

Per [`wedge/validation-tests.md` §1](./wedge/validation-tests.md), send the
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

---

## Step 3 — 10-minute install test (Gate 2 §2)

Per `wedge/validation-tests.md` §2, give 10 developers a 10-minute install
script. The repo already contains one:

```bash
# What you send them (or paste into a call):
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
failures (`wedge/validation-tests.md` §2 "Iteration rule").

---

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

---

## Step 5 — Public examples + screen recording (Gate 1 leftovers)

The agent built three demos under `wedge/demo*`. To close Gate 1 leftovers
([GATES.md](./GATES.md)):

```bash
# Run each so a screen recorder can capture it.
pnpm --filter actant-demo-test-cleanup demo
pnpm --filter actant-demo-langgraph-router demo
pnpm --filter actant-demo-cli demo

# Then in a second terminal for each:
npx actantdb studio --project demo-test-cleanup --store-dir wedge/demo/.actantdb
```

Record a 90-second screencast with QuickTime / OBS / asciinema (the agent
authored an asciinema cast at [`wedge/demo/killer-demo.cast`](./wedge/demo/killer-demo.cast) — playable
with `asciinema play`). Upload the cast or the video to YouTube /
asciinema.org / GitHub Releases.

---

## What is impossible without you

The agent cannot:

- Execute `npm publish` (requires your npm authentication; reversible
  failures + irreversible side-effects).
- Send emails, Discord messages, or GitHub mentions to developers.
- Land a named design partner (requires a human relationship).
- Verify on 10+ external machines that the install works (requires those
  machines to exist outside this sandbox).

Everything that could be staged in code has been staged. The remaining
work is in the world.

## Status at the time of this checklist

- **Gate 1** — implementation-complete; the four leftovers (screencast,
  hero PNG, three-platform-developer verification, public examples) are
  one human action each, with public examples already done.
- **Gate 2** — every prerequisite that lives in code is green; the threshold
  itself requires Steps 1–4 above.
- **Gate 3** — two of the "2 public examples" exist (three, actually);
  threshold itself requires Steps 4–5.

Run Steps 1 → 2 → 3 → 4 → 5 in order, and the gates close.
