# Performance budgets

Explicit latency and throughput targets per operation. Bench harness in `bench/` enforces them in CI.

These are **engineering targets** measured on a developer laptop (M-series Mac or 4-core Linux x86 with NVMe), single-process, single workspace, warm caches. Cloud-mode numbers differ; multi-tenant + network adds a budget elsewhere.

## Hot-path operations (synchronous)

| Operation                  | p50         | p99         | Notes                                       |
| -------------------------- | ----------- | ----------- | ------------------------------------------- |
| `append_user_message`      | < 2 ms      | < 20 ms     | Phase 1 alpha command set                   |
| `append_agent_message`     | < 2 ms      | < 20 ms     |                                             |
| `create_session`           | < 3 ms      | < 25 ms     |                                             |
| `request_tool_call`        | < 5 ms      | < 30 ms     | includes alignment check                    |
| `approve_tool_call`        | < 5 ms      | < 30 ms     |                                             |
| `deny_tool_call`           | < 4 ms      | < 25 ms     |                                             |
| `record_tool_result`       | < 3 ms      | < 20 ms     |                                             |
| `propose_memory`           | < 3 ms      | < 20 ms     |                                             |
| `approve_memory`           | < 4 ms      | < 25 ms     |                                             |
| `reject_memory`            | < 3 ms      | < 20 ms     |                                             |
| `enqueue_effect`           | < 3 ms      | < 20 ms     |                                             |
| `complete_effect` (intake) | < 3 ms      | < 20 ms     | chained downstream record runs sync         |
| Subscription fanout        | —           | < 10 ms p50 | per matching subscriber                     |
| Hot projection read        | < 0.5 ms    | < 5 ms      | L0 cache hit; L2 miss adds ~5 ms            |
| Compiled policy check      | < 0.1 ms    | < 1 ms      | capability-token + DAG; no string match     |
| Rate-limit / budget check  | < 0.05 ms   | < 0.5 ms    | in-memory counter                           |

## Async-lane operations

These have **throughput** and **freshness** targets instead of latency budgets.

| Lane                  | Freshness target               | Throughput target (single-node)   |
| --------------------- | ------------------------------ | --------------------------------- |
| workflow-advance      | < 100 ms after trigger         | 100 advancements / s              |
| memory-candidate      | < 5 s after observation        | 50 candidates / s                 |
| memory-embed          | < 30 s after approval          | 200 embeds / s (FastEmbed-small)  |
| entity-extract        | < 30 s                         | 50 documents / s                  |
| retrieval-trace       | < 50 ms                        | 200 retrievals / s                |
| rerank (in-line)      | included in retrieval p99      | n/a                               |
| risk-explanation      | < 2 s                          | 20 / s (local model)              |
| eval-shadow           | < 60 s                         | depends on eval cost              |
| OTel export           | continuous, sampled            | unbounded (collector-side)         |
| audit-export          | nightly                        | 100 MB / minute                   |
| compliance-evidence   | nightly                        | as needed                         |

## Retrieval profiles (admission control)

`actant-index::plan` picks one per request based on `(latency_goal_ms, sensitivity_ceiling, budget_remaining, task_complexity, backpressure)`.

| Profile          | Target end-to-end | Components                                        |
| ---------------- | ----------------- | ------------------------------------------------- |
| `fast`           | < 50 ms           | lexical + memory recency + minimal rerank skip    |
| `balanced`       | < 200 ms          | hybrid dense+sparse, rerank top 50                |
| `deep`           | < 800 ms          | hybrid + graph, rerank top 200                    |
| `local_private`  | < 300 ms          | local-only routes; no cloud rerank                |
| `degraded`       | < 30 ms           | hot-cache only; emitted under backpressure        |

## Bench harness

`bench/` ships with:

- A workload generator producing a realistic mix of the alpha command set (60 % message-append, 20 % tool-request, 10 % approve, 10 % memory propose/approve).
- A latency reporter (HDR Histogram) per operation.
- A CI gate: regressions > 10 % vs the recorded baseline fail the PR.
- A profiler hook for `cargo flamegraph` on slow runs.

Bench data lives under `bench/baselines/` keyed by `(os, cpu, ram)`. The CI matrix runs `ubuntu-latest-x64` and `macos-latest-arm`.

## What the bench harness does NOT measure

- LLM call latency. Models are external; we cap their effect-queue latency separately.
- Vector store query latency under cold-start. LanceDB warmup is excluded.
- Cross-region replication. Phase 6 adds that.

## Performance regressions

When a regression is detected:

1. CI fails the PR with an HDR histogram diff.
2. The committer must either fix or update the baseline (with an explicit justification).
3. Baselines never go up by more than 10 % without an ADR.

## Tuning levers (Phase 3+)

Once Phase 1 lands and we have real data, the levers we expect to use:

- L0 cache size per hot projection.
- Snapshot frequency for projections.
- Subscription buffer size per actor.
- Rate-limit-snapshot frequency.
- Lane batch size (especially embed + analytics).
- WAL fsync policy (group commit vs immediate; defaults to group commit in `local-fast` mode for 10x throughput).
