# 00 — Overview

## Executive definition

**ActantDB** is a backend system for autonomous agents. It is not just a database. It is:

- a realtime database
- a causal event ledger
- an agent state machine
- a permission system
- an effect queue
- a memory provenance engine
- a model-context firewall
- a workflow runtime
- a replay engine
- a live subscription layer
- an audit/control plane

The core idea:

> Every autonomous action should be recorded, permissioned, replayable, inspectable, and attributable.

SpacetimeDB is the design inspiration because it fuses database tables, server-side logic, reducers, and realtime subscriptions. ActantDB takes that pattern and specializes it for agents.

```
SpacetimeDB:  realtime app/game backend
ActantDB:     realtime autonomous-action backend
```

The fundamental invariant — repeated in `05-security-model.md` and enforced across every subsystem:

> **Every autonomous action becomes a governed, replayable event.**

## Why ActantDB exists

### The problem

Current agent stacks are fragmented:

| Concern              | Today's storage             |
| -------------------- | --------------------------- |
| chat history         | Postgres / SQLite           |
| tool calls           | logs                        |
| memory               | vector DB                   |
| files / artifacts    | object storage              |
| workflows            | Temporal / custom DAG       |
| approvals            | custom app state            |
| audit logs           | observability platform      |
| model traces         | LangSmith-style tracing     |
| permissions          | custom middleware           |
| worker state         | queue / Redis               |

This fragmentation creates six failures that the system itself cannot recover from:

| Failure                   | What goes wrong                                             |
| ------------------------- | ----------------------------------------------------------- |
| **No causality**          | You cannot reconstruct *why* an agent did something.        |
| **No replay**             | You cannot rerun from before a bad model/tool decision.     |
| **No memory provenance**  | You cannot explain why the agent "knows" something.         |
| **No context visibility** | You cannot inspect what the model saw.                      |
| **No authority boundary** | Tools, models, workflows, and subagents blur permissions.   |
| **No unified live state** | App, CLI, dashboard, workers, and agents drift out of sync. |

### The design thesis

Autonomous agents need a backend that treats **action** as the primitive — not rows, not messages, not vectors, not traces.

Every autonomous system has the same lifecycle:

```
intent
→ context
→ model decision
→ proposed action
→ permission check
→ side effect
→ observation
→ memory update
→ workflow state change
→ audit trail
→ possible replay
```

ActantDB stores and governs that lifecycle, end to end.

## The category, in one sentence

ActantDB is the **autonomous-action backend**: the unified runtime that turns the lifecycle above into typed commands, governed effects, replayable events, and live state — so that any agent built on top of it is, by construction, accountable, debuggable, and safe.

## Who this is for

| Audience                         | Why they care                                                          |
| -------------------------------- | ---------------------------------------------------------------------- |
| Agent framework developers       | Stop reinventing approval, memory, replay, and audit in every product. |
| AI infrastructure teams          | Run a fleet of agents with one control plane.                          |
| Personal-agent product builders  | Ship desktop agents with provenance and user-visible permissions.      |
| Enterprise automation teams      | Replace ad-hoc agent middleware with governed workflows.               |
| AI safety / governance teams     | Inspect, replay, and bound the authority of every agent in production. |
| Research labs                    | Run reproducible agent experiments with policy variation.              |

## Mental model

There are exactly four kinds of things in ActantDB:

1. **Actors** — humans, agents, subagents, models, tools, workers, the system itself.
2. **Commands** — typed, permission-checked requests to change state.
3. **Events** — immutable records of what happened (the Chronicle).
4. **Projections** — current, queryable state derived from events.

Side effects (model calls, tool runs, shell commands, browser clicks, file writes, network calls) are not part of the database transaction. They are scheduled, executed, and re-recorded as events. This is the single most important architectural choice in ActantDB and is detailed in `04-effect-protocol.md`.

## What ActantDB is *not*

| Not                                      | Because                                                                         |
| ---------------------------------------- | ------------------------------------------------------------------------------- |
| A model-serving runtime                  | Models are workers behind the effect queue.                                     |
| A vector database                        | Embeddings live in companion stores; ActantDB stores references and policy.     |
| A secret manager                         | Secrets live in Keychain / Vault / KMS; ActantDB stores `secret_ref` only.      |
| An object storage system                 | Large artifacts live in an artifact store; ActantDB stores URIs and hashes.     |
| A general-purpose OLTP database          | The schema and APIs are agent-specific. Use Postgres for non-agent workloads.   |
| An LLM observability product             | It includes observability, but the surface is action governance, not traces.   |
| A workflow engine like Temporal          | Flow Engine is part of ActantDB and reuses Chronicle + Effect Queue.            |

## Naming

- **ActantDB**: the product.
- **actantdb**: the binary, package, and CLI when written as one word.
- **Actant**: any entity capable of action (the *etymological* root); used as a prefix for crates (`actant-core`) and types (`ActantEvent`).
- **Chronicle**: the event ledger subsystem (the source of truth).
- **Studio**: the dashboard product (`actant-studio`).

## Glossary pointer

All defined terms are catalogued in `12-glossary.md`. When in doubt, check there before introducing a new one.

## Verification

This file is consistent with the rest of the spec set when:

- [ ] Every "core idea" claim is realized by at least one subsystem in `01-architecture.md`.
- [ ] Every audience listed has a section in `09-sdk-design.md` (developer ergonomics) or `11-roadmap.md` (delivery timeline).
- [ ] Every "what ActantDB is not" boundary is preserved by every command in `03-command-spec.md` (no commands write raw secrets, raw vectors, raw blobs, etc.).
