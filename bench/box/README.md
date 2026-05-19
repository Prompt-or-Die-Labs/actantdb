# `@actantdb-bench/box` — cold-start TTI benchmark

A reproducible **Time-To-Interactive (TTI)** benchmark for
[`@actantdb/box`](../../packages/actant-box), shape-matched to the
[upstash/benchmarks](https://github.com/upstash/benchmarks)
sandbox-provider matrix so the numbers are directly comparable to
E2B / Modal / Daytona / Vercel / Cloudflare cold-starts.

## What's measured

For each box:

```ts
const t0 = performance.now();
const box = await Box.create({ storeRoot: <isolated tmp dir> });
await box.exec.command("echo ready");        // first interactive command
const tti = performance.now() - t0;
await box.delete();
```

`tti` is the **Time-To-Interactive**: API call to first command execution.
This is the same metric Upstash uses, applied to a local subprocess instead
of a cloud container.

## Scenarios (mirrors Upstash exactly)

| Scenario     | What it does                                              |
| ------------ | --------------------------------------------------------- |
| `sequential` | One at a time, `await` each. Lower bound on contention.   |
| `staggered`  | 200ms between launches. Realistic agent-fan-out pattern.  |
| `burst`      | All N started simultaneously via `Promise.all`. Worst case. |

## Run it

```bash
# Build then run all three scenarios at N=100.
pnpm --filter @actantdb-bench/box build
pnpm --filter @actantdb-bench/box bench

# Single scenario, smaller N.
pnpm --filter @actantdb-bench/box bench -- --scenario burst --n 10

# Pick a custom output dir.
pnpm --filter @actantdb-bench/box bench -- --scenario sequential --n 50 --out ./my-results

# Full help.
pnpm --filter @actantdb-bench/box bench -- --help
```

## What you get

For each scenario, stdout looks like:

```
ActantDB Box (local) — sequential, N=100
  TTI:   min=12.0ms  median=18.0ms  p95=45.0ms  p99=120.0ms  max=480.0ms  mean=24.0ms
  ok=100/100  fail=0
  Composite score: 96.2 / 100
  → bench/box/results/sequential-1715990400.json
```

The JSON has every TTI sample, the computed stats, the composite score, and
the host/runtime fingerprint:

```json
{
  "scenario": "sequential",
  "n": 100,
  "startedAt": "2026-05-19T...",
  "ok": 100,
  "fail": 0,
  "errors": [],
  "tti_ms": [12.4, 14.1, 18.0, ...],
  "stats": { "min": 12.0, "max": 480.0, "median": 18.0, "p95": 45.0, "p99": 120.0, "mean": 24.0 },
  "composite": 96.2,
  "actantdb_box_version": "0.0.13",
  "node_version": "v24.x.x",
  "platform": "darwin 25.x.x",
  "hostname": "..."
}
```

## Composite score (0–100)

```
base       = 100 * success_rate
penalty_ms = max(0, median_ms - 50)
penalty    = min(60, penalty_ms / 20)
score      = max(0, base - penalty)
```

The **shape** is matched to Upstash (success-rate base, latency penalty,
0–100); the **values are not** — Upstash's exact formula is not published,
so we don't claim numerical parity. Compare scores across runs of *this*
benchmark only.

## Cleanup invariant

Each box gets its own subdirectory under one per-scenario tmp root
(`/tmp/actantdb-box-bench-<scenario>-XXXXXX/`). The runner `rm -rf`s that
root in a `finally` even if a measurement throws mid-flight. N=100 runs
will not pollute `~/.actantdb/boxes`.

If `Box.delete()` fails mid-measurement, the measurement still counts as
`ok` (we already have the TTI we cared about); the leaked dir is reaped
by the scenario-root cleanup at the end.

## What it's *not*

- **Not a Rust crate.** The Rust `bench/` crate measures HTTP throughput on
  the kernel server; this is a TS-only sibling measuring SDK cold-start.
- **Not a comparison of isolation models.** Local subprocess is faster than a
  cloud container by orders of magnitude — that's the whole point — but cloud
  providers win on multi-tenancy, isolation, and elastic capacity. See
  [`/BENCHMARKS.md`](../../BENCHMARKS.md) for the honest framing.

## Apples-to-apples vs Upstash

See the "Box cold-start" section in
[`/BENCHMARKS.md`](../../BENCHMARKS.md#box-cold-start) for the comparison
against published Upstash numbers for E2B, Modal, Daytona, Vercel
Sandboxes, and Cloudflare Sandboxes.
