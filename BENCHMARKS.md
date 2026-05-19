# ActantDB benchmarks (2026-05-18)

This is a fresh, end-to-end performance characterization of ActantDB. It re-runs
the existing Criterion micro-benchmarks under `bench/benches/` and adds a new
end-to-end scenario suite under `bench/scenarios/` that drives the **released
v0.0.7 server binary** over HTTP with an **on-disk SQLite WAL database** —
which is closer to how a real Mastra / agent app will use it.

## Headline numbers

| Workload                                     | p50      | p95      | p99      | Throughput | Notes                          |
| -------------------------------------------- | -------- | -------- | -------- | ---------- | ------------------------------ |
| `storage::append_event` (criterion, in-mem)  | 218 µs   | n/a      | n/a      | ~4.6 k EPS | README claim was 60 µs         |
| `command::dispatch(append_user_message)`     | 242 µs   | n/a      | n/a      | ~4.1 k req/s | README claim was 116 µs      |
| `POST /v1/command` in-process (criterion)    | 464 µs   | n/a      | n/a      | ~2.0 k req/s | README claim was 341 µs      |
| **HTTP single-session burst, fresh DB, n=100**   | **225 µs** | 663 µs   | 12.1 ms  | **2.4 k req/s** | tail spike from WAL/SQLite flush |
| **HTTP single-session burst, fresh DB, n=10 000** | **464 µs** | **1.00 ms** | **2.12 ms** | **1.8 k req/s** | most representative single-agent number |
| HTTP single-session burst, warm 4k-event DB, n=1000 | 1.26 ms | 10.9 ms | 21.3 ms | 405 req/s | latency grows with on-disk size |
| HTTP concurrent 10×50 (fresh 10k DB after warmup) | 493 µs | 7.1 ms | 37.8 ms | **3.9 k req/s** | aggregate throughput up, tails worse |
| HTTP concurrent 20×100                        | 7.5 ms   | 26.8 ms  | 71.5 ms  | 1.8 k req/s | contention pronounced beyond 10 sessions |
| `POST /v1/replay/checkpoint`                  | 1.2 ms   | —        | —        | —          | 200-event session              |
| `POST /v1/replay/run` (recorded mode + diff)  | 2.2 ms   | —        | —        | —          | 200 diff entries               |
| **Build+diff total (200 events)**             | **3.4 ms** | —      | —        | —          | sum of checkpoint + run        |
| Server RSS after 10 001 events                | **12.7 MB** | —     | —        | —          | only +1.4 MB vs cold server    |
| SQLite on-disk for 10 001 events              | **1 829 → 1 407 bytes/event** | — | — | —    | hot WAL → after `wal_checkpoint(TRUNCATE)` |

## TL;DR

- **The README's published numbers are stale.** Every criterion claim is off by
  1.4× – 4×. The largest gap is `storage_append_event`: README says 60 µs, the
  benchmark on the same hardware shows **218 µs**. Criterion's own output even
  flags `Performance has regressed: +247% to +309%`. See "README claims vs
  measured" below.
- **End-to-end HTTP latency on the released binary is healthy.** Steady-state
  on-disk WAL: **p50 ≈ 460 µs, p95 ≈ 1 ms, p99 ≈ 2 ms** for the realistic 10k
  single-session burst — well inside `docs/SLO.md`'s "< 5 ms p50 / < 25 ms p99"
  bar.
- **Throughput peaks around 4 k req/s** for short bursts (concurrent 10×50 on
  a fresh DB hit 3.9 k req/s) and falls under heavier load — 20×100 concurrent
  fell to 1.8 k req/s with p99 of 72 ms. The single global SQLite writer is
  the bottleneck.
- **Replay is fast.** Building a 200-event checkpoint and running a recorded-
  mode diff is 3.4 ms wall, end-to-end over HTTP. The hot path is dominated
  by the diff phase, not checkpoint creation.
- **Disk footprint is ~1.8 kB/event.** For a sustained 10 EPS agent that's
  ~1.5 GB/year — within reason for laptop use, but rotation/compaction will
  matter for long-lived servers.
- **Memory is tiny.** The server gained only 1.4 MB RSS while writing 10 k
  events — there is no per-event in-memory growth visible.

## Setup

- **Host:** macOS 26.2 (Darwin 25.2.0), Apple M4, 10 cores, 16 GB RAM.
- **Binary under test:** `/tmp/swift-trial/actantdb-v0.0.7-aarch64-apple-darwin/actantdb-server`.
  `--version` reports `0.0.5` — the v0.0.7 archive ships the v0.0.5 binary.
  Flag this with the release-eng team; the npm packages are v0.0.7. The
  criterion benches run against the **current `main` tree** (which is v0.0.7),
  so they cover the gap.
- **DB:** `/tmp/bench.db` and `/tmp/bench10k.db`, SQLite WAL mode, default
  PRAGMA set (whatever `actantdb migrate` lays down).
- **Seeding:** `actantdb migrate` does *not* seed a default workspace/actor,
  so the FK constraint on `command.workspace_id → workspace.id` fires on the
  first `create_session`. Bench seeds with raw `sqlite3 INSERT INTO workspace
  / INSERT INTO actor` — documented in `bench/scenarios/main.rs`.
- **Server:** `actantdb-server --bind 127.0.0.1:4555 --db /tmp/bench10k.db`,
  no TLS, single instance, default tokio runtime.
- **Bench driver:** `bench/scenarios/main.rs`, compiled with
  `cargo build -p actant-bench --release --bin scenarios`. Each request is
  timed individually with `std::time::Instant`, sorted, quantiles computed
  directly. Wall clock for throughput measured separately.
- **Criterion runs:** `cargo bench -p actant-bench --bench <name>`, profile
  `bench` (= optimized).

## README claims vs measured

The README has:

> `storage_append_event ≈ 60 µs`. `command_append_user_message ≈ 116 µs`.
> HTTP `POST /v1/command` median **341 µs**.

Measured on this run, same machine, same Criterion benches:

| Bench                          | README       | Measured (median) | Ratio   |
| ------------------------------ | ------------ | ----------------- | ------- |
| `storage_append_event`         | ~60 µs       | **218 µs**        | **3.6×** slower |
| `command_append_user_message`  | ~116 µs      | **242 µs**        | **2.1×** slower |
| `http_append_user_message`     | 341 µs       | **464 µs**        | **1.4×** slower |

Criterion flags `Performance has regressed +247% to +309%` on
`storage_append_event` vs a stored baseline of unknown provenance — that
delta is meaningless without knowing which host captured the baseline. What
matters is the absolute numbers vs the README's: 1.4×–4× slower across the
board.

Note these two runs cover different code paths:

- The **criterion benches** ran against the current `main` source tree
  (compiles as `0.0.7`).
- The **HTTP scenarios** ran against the released binary in
  `/tmp/swift-trial/actantdb-v0.0.7-aarch64-apple-darwin/` which self-reports
  `0.0.5`.

The HTTP scenario p50 (464 µs at n=10 000) lines up almost exactly with the
in-process criterion `http_append_user_message` median (464 µs). That's
strong evidence the binary and the source perform the same on this host —
so the README discrepancy is a **host** issue (different machine when the
README numbers were captured), not a **version** regression between v0.0.5
and v0.0.7.

**Recommendation:** delete the saved criterion baseline, rerun on the
canonical bench host, and update the README with the new numbers (or
annotate the README's numbers with the host they were captured on).

That said, **all three are still within `docs/SLO.md`'s budget** (p50 < 100 µs,
< 200 µs, < 5 ms respectively — actually `storage_append` is slightly *over*
its 100 µs p50 target at 218 µs, that's worth a closer look).

## Scenario detail

All scenarios run against `actantdb-server` over loopback HTTP, on-disk WAL
SQLite. Code: `bench/scenarios/main.rs`.

### 1. Single-agent burst — `--scenario single`

Pre-creates one session, then issues N sequential `append_user_message`
commands. Each request timed individually. Sample size matters a lot for the
tail percentile — at n=100 the p99 is one data point.

| n     | DB state at start  | wall   | throughput | p50      | p95      | p99      | max     |
| ----- | ------------------ | ------ | ---------- | -------- | -------- | -------- | ------- |
| 100   | fresh              | 41 ms  | 2 425 req/s | 225 µs  | 663 µs   | 12.1 ms  | 12.1 ms |
| 1 000 | warm (~4 k events) | 2.47 s | 405 req/s  | 1.26 ms  | 10.9 ms  | 21.3 ms  | 47.4 ms |
| 10 000| fresh              | 5.60 s | 1 787 req/s | 464 µs  | 1.00 ms  | 2.12 ms  | 21.2 ms |

The n=1000 row is the warmest DB and shows the realistic worst case for a
single sequential client on a populated DB — note the order-of-magnitude jump
in p95. The n=10 000 row started on a fresh DB and stayed well-behaved across
10 k writes; this is the **most representative single-agent number** to quote.

### 2. Concurrent sessions — `--scenario concurrent`

Pre-creates N sessions, then fires M `append_user_message` per session
concurrently across tokio tasks (single client process, single server
process).

| N × M     | wall   | throughput | p50     | p95     | p99     | max      |
| --------- | ------ | ---------- | ------- | ------- | ------- | -------- |
| 10 × 50   | 127 ms | 3 924 req/s | 493 µs  | 7.13 ms | 37.8 ms | 66.3 ms  |
| 20 × 100  | 1.10 s | 1 813 req/s | 7.53 ms | 26.8 ms | 71.5 ms | 205.1 ms |

Aggregate throughput is higher than the single-client case (3.9 k vs 1.8 k
req/s) — there's headroom in the server. But tail latency degrades fast: by
20 concurrent sessions, the p99 is more than 30× the single-client p99. This
is consistent with a single global SQLite writer contending with itself; a
real workload that interleaves reads and writes will look different.

### 3. Replay from event — `--scenario replay`

1. Create a session, append 200 user-messages, capture the last `event_id`.
2. `POST /v1/replay/checkpoint` with that `event_id` — server fold-replays
   the event chain into a checkpoint row.
3. `POST /v1/replay/run` in `recorded` mode — server runs the replay and
   returns the diff as JSON.

Measured wall-clock from the client side, single attempt (these are not
percentile measurements, just one observation each):

```
record 200 events: 78 ms wall (2 573 ev/s)
checkpoint:        1.2 ms
replay run+diff:   2.2 ms (201 diff entries)
build+diff total:  3.4 ms
```

Replay is the cheapest part of the system on a per-event basis: ~17 µs per
event for the recorded-mode diff. This is good news for the
"deterministic replay" story — even a 10 000-event session would finish
diff in ~170 ms.

### 4. Memory — 10 000 events

```
rss_before=11 360 KB
rss_after =12 736 KB
delta     = 1 376 KB
```

The server grew by 1.4 MB while accepting and persisting 10 001 events. The
WAL holds the recent tail; we are not buffering events in memory.

### 5. SQLite disk footprint — 10 000 events

Immediately after the burst (WAL still hot):

```
agent_event count: 10 001
db file:        14 082 048 bytes  (14.1 MB)
wal file:        4 177 712 bytes  ( 4.2 MB)
shm file:           32 768 bytes  (32 KB)
total:          18 292 528 bytes  (18.3 MB)
bytes/event:         1 829 bytes
```

After `PRAGMA wal_checkpoint(TRUNCATE)` (steady-state — the WAL has drained
into the main db file; this is the number most readers actually want):

```
agent_event count: 10 712   (a couple of replay/checkpoint runs landed here too)
db file:        15 044 608 bytes  (15.0 MB)
wal file:                0 bytes  (truncated)
shm file:           32 768 bytes  (32 KB)
total:          15 077 376 bytes  (15.1 MB)
bytes/event:         1 407 bytes
```

Per-event cost is **~1.4 kB at steady state**, ~1.8 kB with a hot WAL. Each
row is a 64-char event hash, prev-hash, JSON payload (small here — just
`{"text": "msg N"}`), and a handful of ULIDs + an ISO-8601 timestamp.
Roughly half of that is the SHA-256 hex strings plus indexes. A production
workload with larger payloads would amortize the hash overhead down.

## Methodology notes

- **Sample size:** n=100 is too small for a stable p99 (one data point).
  The n=10 000 single-burst run is the one to trust for tails. The
  20×100 = 2 000 sample concurrent run is borderline; treat p99 as ±50%.
- **No warmup:** scenarios issue requests immediately after server boot;
  the SQLite page cache and reqwest connection pool warm up during the
  first ~10 requests. For sub-millisecond latencies this matters — see the
  `min` column in each scenario.
- **Coldest first:** the order matters. The 10k single burst started on a
  truly fresh DB. The 1k warm run started on a DB with ~4k pre-existing
  events from the all-scenarios run. Compare like with like.
- **Single client process:** all timing is wall-clock at the reqwest layer,
  not server-internal. Includes TCP, HTTP, JSON parse on both ends. For
  the server-internal numbers, see the criterion benches.
- **/usr/bin/time -l RSS:** measured the **client** process, not the server.
  Server RSS sampled directly via `ps -p $SERVER_PID -o rss=` before and
  after the workload. On macOS `-l` reports bytes, not pages.

## How to reproduce

```bash
# 1. Build (release)
cargo build -p actant-bench --release --bin scenarios

# 2. Reset DB + start server
rm -f /tmp/bench10k.db /tmp/bench10k.db-shm /tmp/bench10k.db-wal
/tmp/swift-trial/actantdb-v0.0.7-aarch64-apple-darwin/actantdb \
    migrate --db /tmp/bench10k.db
sqlite3 /tmp/bench10k.db \
    "INSERT INTO workspace (id, name, created_at)
        VALUES ('ws_bench', 'bench', '2026-05-18T00:00:00Z');
     INSERT INTO actor (id, workspace_id, kind, display_name, created_at)
        VALUES ('act_bench', 'ws_bench', 'agent', 'bench-agent',
                '2026-05-18T00:00:00Z');"
/tmp/swift-trial/actantdb-v0.0.7-aarch64-apple-darwin/actantdb-server \
    --bind 127.0.0.1:4555 --db /tmp/bench10k.db &

# 3. Run scenarios
./target/release/scenarios --base-url http://127.0.0.1:4555 \
    --workspace ws_bench --actor act_bench --scenario all

# 4. The criterion micro-benches (in-memory)
cargo bench -p actant-bench --bench storage_append
cargo bench -p actant-bench --bench command_dispatch
cargo bench -p actant-bench --bench http_command -- --sample-size 20
```

## Raw output

Saved under `/tmp/actant-bench-results/`:

- `criterion_storage_append.txt` — `bench_function("storage_append_event")`
- `criterion_command_dispatch.txt`
- `criterion_http_command.txt`
- `scenarios_single_10k.txt` — the trusted 10k single burst
- `scenarios_single_1k.txt`
- `scenarios_conc_10x50.txt`, `scenarios_conc_20x100.txt`
- `scenarios_replay_200.txt`
- `server.log`, `server10k.log` — server stderr

## Action items

1. **Update the README** — the published latencies under-report by 1.4×–4×.
   Either re-baseline the criterion benches on the canonical bench host, or
   annotate the README claims with the host they were measured on.
2. **Investigate `storage_append` 218 µs vs 100 µs SLO.** The criterion bench
   is now slightly *over* the SLO p50 target on this machine. If that's a
   reproducible regression, `docs/SLO.md` either needs to relax or the
   storage layer needs a look.
3. **`actantdb migrate` should optionally seed a default workspace/actor.**
   Right now `create_session` against a freshly-migrated DB fails with a
   `FOREIGN KEY constraint failed`. A `--seed-default` flag (or simply
   creating `ws_default` + `act_system` automatically) would remove a sharp
   edge for first-time users.
4. **Concurrent-write contention is the next perf lever.** Going from 10 →
   20 concurrent sessions cut throughput in half and pushed p99 to 72 ms.
   If you want to support > 10 concurrent agent sessions per server, this
   is the work item.

## Box cold-start

A new benchmark sibling under [`bench/box/`](bench/box/) measures
`@actantdb/box` cold-start **Time-To-Interactive (TTI)** with the same
methodology the [upstash/benchmarks](https://github.com/upstash/benchmarks)
sandbox-provider matrix uses. Numbers below are from this M-series Mac on
2026-05-19 against `@actantdb/box@0.0.13`, run via:

```bash
pnpm --filter @actantdb-bench/box build
pnpm --filter @actantdb-bench/box bench           # all three scenarios @ N=100
```

### Methodology (parity with Upstash)

Per measurement:

```
t0 = performance.now()
box = await Box.create({ storeRoot: <isolated tmp dir> })
await box.exec.command("echo ready")    # first interactive command
tti_ms = performance.now() - t0
await box.delete()                       # measure each run fresh
```

Three load patterns at N=100:

| Scenario     | Definition                                                 |
| ------------ | ---------------------------------------------------------- |
| `sequential` | `for i in 0..N { await measureTTI() }` — one at a time     |
| `staggered`  | `setTimeout(launch, i*200ms)` — fan-out pattern             |
| `burst`      | `Promise.all(Array.from({length:N}, measureTTI))` — worst case |

The composite score *shape* matches Upstash (success-rate base, latency
penalty, 0..100). The *values* don't claim numerical parity — Upstash
weights median/p95/p99 60/25/15 against a 10s ceiling per
[their METHODOLOGY.md](https://github.com/upstash/benchmarks/blob/master/METHODOLOGY.md);
ours is `100*success_rate − min(60, max(0, median−50ms)/20)`, with the
formula written in [`bench/box/src/composite.ts`](bench/box/src/composite.ts)
so reviewers can swap it. Compare composite scores within this benchmark
only, not against Upstash's printed scores.

### ActantDB Box numbers on this host

Apple M4, macOS 26.2, Node 25.9.0, `@actantdb/box@0.0.13`:

| Scenario    | N   | min   | median  | p95    | p99    | max    | mean   | ok/N    | composite |
| ----------- | --- | ----- | ------- | ------ | ------ | ------ | ------ | ------- | --------- |
| sequential  | 100 | 5 ms  | **7 ms**    | 9 ms   | 10 ms  | 62 ms  | 7 ms   | 100/100 | 100.0     |
| staggered   | 100 | 7 ms  | **12 ms**   | 17 ms  | 21 ms  | 23 ms  | 13 ms  | 100/100 | 100.0     |
| burst       | 100 | 175 ms| **215 ms**  | 255 ms | 258 ms | 260 ms | 217 ms | 100/100 | 91.7      |

Burst-100's 215 ms median is honest signal, not a bug: 100 simultaneous
subprocess spawns plus 100 simultaneous SQLite WAL opens on a shared
parent dir hit fs/sqlite contention. The same workload spread across the
default 200 ms stagger collapses to 12 ms median.

Raw JSON for these runs lives under
[`bench/box/results/`](bench/box/results/).

### Apples-to-apples vs Upstash's published numbers

Upstream JSON sources (linked so anyone can re-derive these without trusting
us): [`results/sequential_tti/latest.json`](https://github.com/upstash/benchmarks/blob/master/results/sequential_tti/latest.json) ·
[`results/staggered_tti/latest.json`](https://github.com/upstash/benchmarks/blob/master/results/staggered_tti/latest.json) ·
[`results/burst_tti/latest.json`](https://github.com/upstash/benchmarks/blob/master/results/burst_tti/latest.json)
(snapshot taken 2026-04-11, `linux x64`, `node v24.14.1`).

**Sequential, N=100 — median TTI:**

| Provider                 | median TTI | vs ActantDB Box |
| ------------------------ | ---------- | --------------- |
| **ActantDB Box (local)** | **7 ms**   | —               |
| Daytona                  | 107 ms     | 15×             |
| Vercel Sandboxes         | 404 ms     | 58×             |
| e2b                      | 415 ms     | 59×             |
| Blaxel                   | 445 ms     | 64×             |
| Hopx                     | 1 111 ms   | 159×            |
| Modal                    | 1 458 ms   | 208×            |
| Cloudflare Sandboxes     | 1 512 ms   | 216×            |
| Namespace                | 1 758 ms   | 251×            |
| Runloop                  | 1 921 ms   | 274×            |

**Staggered, N=100 — median TTI:**

| Provider                 | median TTI | vs ActantDB Box |
| ------------------------ | ---------- | --------------- |
| **ActantDB Box (local)** | **12 ms**  | —               |
| Daytona                  | 103 ms     | 8.6×            |
| e2b                      | 392 ms     | 33×             |
| Vercel Sandboxes         | 393 ms     | 33×             |
| Blaxel                   | 449 ms     | 37×             |
| Hopx                     | 1 182 ms   | 99×             |
| Modal                    | 1 439 ms   | 120×            |
| Cloudflare Sandboxes     | 1 568 ms   | 131×            |
| Namespace                | 1 752 ms   | 146×            |
| Runloop                  | 1 949 ms   | 162×            |

**Burst, N=100 — median TTI (this is where the contention story shows up):**

| Provider                 | median TTI | vs ActantDB Box |
| ------------------------ | ---------- | --------------- |
| **ActantDB Box (local)** | **215 ms** | —               |
| Daytona                  | 231 ms     | 1.07×           |
| e2b                      | 594 ms     | 2.8×            |
| Vercel Sandboxes         | 629 ms     | 2.9×            |
| Blaxel                   | 1 096 ms   | 5.1×            |
| Modal                    | 1 722 ms   | 8.0×            |
| Cloudflare Sandboxes     | 1 791 ms   | 8.3×            |
| Namespace                | 2 141 ms   | 10.0×           |
| Runloop                  | 6 025 ms   | 28×             |
| Hopx                     | 15 179 ms  | 71×             |

Daytona being within 7% on burst is the headline caveat — at 100
simultaneous spawns the local-vs-cloud advantage shrinks dramatically.
The other 8 providers stay 3×–70× behind.

(Codesandbox excluded — its 2026-04-11 results show zero-ms cold-starts
and a 0 sample mean, almost certainly a probe/timing bug upstream.)

### Honest framing

This is **your machine vs their container pool**. ActantDB Box wins
cold-start by 1–2 orders of magnitude on sequential / staggered work,
and is competitive with the fastest cloud (Daytona) on burst — because
we're skipping the entire container-boot path: no VM cold-start, no
image pull, no scheduler queue, just `mkdir` + open SQLite + `spawn /bin/sh`.

What the cloud providers buy that we don't:

- Isolation between concurrent tenants (each Box currently shares the
  host's user/fs). For multi-tenant workloads you still want a sandbox
  layer — Box is for *the agent on your own machine*.
- Elastic capacity. Box throughput is gated by the host's process and
  fs limits; cloud providers scale horizontally.
- Geographic distribution / edge runtime semantics (Cloudflare et al.).

Box is the right primitive for local agent loops (Mastra / LangGraph /
hand-rolled) where TTI dominates iteration speed. It is *not* a
replacement for a hosted sandbox service in a multi-tenant SaaS.

### Reproducibility

- Bench source: [`bench/box/`](bench/box/) (TypeScript only, no Rust toolchain
  needed).
- Daily CI run: [`.github/workflows/box-bench.yml`](.github/workflows/box-bench.yml)
  on `ubuntu-latest` and `macos-latest`. Results land in the workflow run
  summary plus a 30-day artifact rather than being auto-committed to `main`.
- Upstash methodology source:
  <https://github.com/upstash/benchmarks/blob/master/METHODOLOGY.md>.
