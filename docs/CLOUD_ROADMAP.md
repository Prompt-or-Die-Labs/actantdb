# ActantDB Cloud — roadmap

ActantDB has been local-first and self-hosted since day one. This document
anchors the move to a hosted cloud option, starting with **ActantDB Box** —
our answer to [Upstash Box](https://upstash.com/docs/box).

## Phase 1 — Local Box (this PR)

Package: `@actantdb/box`
Status: implementing now.

Surface mirrors Upstash Box 1:1 so anyone using Box can drop ActantDB in
with a one-line import change. Backed entirely by existing primitives:

| Box method | ActantDB primitive |
|---|---|
| `Box.create` | new local workspace dir + `@actantdb/core` ledger + injected agent |
| `box.agent.run / stream` | `withActant()` over any tools-shaped agent |
| `box.exec.command / stream` | `actant-workers::shell` |
| `box.files.{read,write,list,upload,download}` | `actant-workers::file` |
| `box.git.{clone,diff,commit,push,createPR,exec,checkout,status,updateConfig}` | shell worker + git adapter |
| `box.schedule.{exec,agent,list,get,pause,resume,delete}` | `actant-trigger` crate |
| `box.snapshot / box.listSnapshots / box.deleteSnapshot / Box.fromSnapshot` | `@actantdb/replay::checkpoint` + workspace tar snapshot |
| `box.pause / resume / delete` | session lifecycle + workspace cleanup |
| `box.cd / box.cwd` | workspace-relative path resolution |
| `box.configureModel` | `actant-runtime::models` registry lookup |

Run modes:

- **`mode: "local"` (default)** — local workspace + local subprocess. Free,
  fast, full ledger of every action. Works offline.
- **`mode: "cloud"`** — points at an ActantDB Cloud endpoint. Phase 2
  ships the control plane; the SDK contract is in place from day one so
  consumer code is the same regardless of mode.

What we have that Upstash Box doesn't:

- **Hash-chained ledger** of every action (Upstash has logs).
- **Replay with overrides** — `runFromEvent(eventId, { policy, without_memory, alternatePlannerOutput })`. Not just disk restore — *causal* re-run under different assumptions.
- **Guard verdicts** as typed events with policy snapshot.
- **Capsule sensitivity ceiling** so secrets never reach a cloud model.
- **Any framework**, not just Claude Code / Codex / OpenCode.
- **Embedded OR server**, same API.

## Phase 2 — Cloud control plane

Not implemented in this PR. Anchored here so Phase 1's `mode: "cloud"`
contract stays honest.

Required components:

| Component | Owner crate / package | Status |
|---|---|---|
| Multi-tenant workspace boundary | `actant-tenant` | partial, needs hardening + hot-path verification |
| User identity + linking-code login | `actant-auth` | ✅ shipped |
| Per-actor billing / metering | new — `actant-billing` | not started |
| Hosted runtime (microVM / container per Box) | new — `actant-runtime-host` (Firecracker or runc) | not started |
| Control plane API (`/v1/cloud/box/*`) | extension of `actant-server` | API contract drafted, no impl |
| Public DNS + TLS | infra (Cloudflare / Caddy) | not started |
| Snapshot storage (S3-backed, content-addressed) | `actant-objectstore` already exists | substrate ready |
| Schedule executor for cron triggers | `actant-trigger` already exists | substrate ready |
| Cold-start optimization (warm pool of pre-booted boxes) | `actant-runtime-host` | not started |

Cost target: undercut Upstash's $0.10/CPU-hr by running on bare-metal or
spot capacity. Concrete pricing decided after Phase 2 lands.

## Phase 3 — Differentiated features

Things only ActantDB Cloud can do once Phase 2 ships:

- **Replay-as-a-service** — paste an event ID into a hosted Studio,
  re-run with overrides, get a diff. No local install.
- **Audit-export to your S3** — `actant-sync` already supports this,
  wire it into the hosted control plane.
- **Per-actor approval webhooks** — tie the `approval_required` event
  to Slack / email / Linear without writing glue code.
- **Hosted policy registry** — central policy doc with `iss == workspace_id`
  signed by ActantDB Cloud; consumers verify locally.
- **Cross-org replay** — replay against the same anchored event but
  inside a different tenant's policy + memory state. Useful for
  "what would Vendor A's policy have done with my run?"

These are post-MVP; Phase 1 + 2 ship first.

## What ships in this PR

Just **`@actantdb/box` local mode**, with the cloud contract documented
but not wired. Consumers can write `Box.create({ mode: "cloud" })` today
and get a clear `NotImplemented` pointing here. Same call site works the
day cloud lands.
