# ADR-0020: Deployment modes

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

ActantDB serves wildly different audiences:

- A solo developer running a personal coding agent on their laptop.
- A small team running a self-hosted instance with team approvals.
- An enterprise running multi-tenant ActantDB Cloud with SOC 2 evidence flow.
- A regulated industry user (healthcare, finance) requiring legal hold + payments record-keeping.
- A research team running experimental rerankers + multivector retrieval.

Forcing every developer to pay the cost of enterprise compliance generation kills the local-fast experience. Forcing enterprise to opt out of compliance generation kills enterprise. The answer is named deployment modes.

## Decision

ActantDB ships **six deployment modes**. Same kernel, different enabled async lanes.

| Mode             | Audience                             | Lanes enabled                                                |
| ---------------- | ------------------------------------ | ------------------------------------------------------------ |
| `local-fast`     | personal agents, dev laptops         | minimal: workflow, embedding (local only), retrieval-trace   |
| `developer`      | building + iterating                 | + memory-candidate, OTel stdout, eval shadow, replay checkpoints |
| `team`           | small team, self-hosted              | + governance evidence, audit export, prompt registry, model routing decisions, multi-tenant trim |
| `enterprise`     | regulated companies                  | + policy-as-code, supply-chain verification, OTel OTLP export, SIEM bridge, legal hold |
| `regulated`      | healthcare, finance, payments        | + AP2 records, retention enforcement, attestation, key-managed encryption |
| `research`       | labs, experimental retrieval         | + experimental rerankers, multivector + ColBERT, A/B retrieval modes |

The mode is a column on `workspace`. Switching modes flips which lanes subscribe; the hot kernel is unchanged. A workspace may be `local-fast` for personal use and a sibling workspace in the same install may be `team` for team work; they don't interfere.

Templates choose a default mode:

```
minimal             → local-fast
coding-agent        → developer
support-agent       → team
research-agent      → developer (research available as flag)
desktop-agent       → local-fast
multi-agent-board   → team
enterprise-agent    → enterprise
```

## Consequences

### Positive

- `actant dev` on `local-fast` mode boots in seconds; no compliance overhead.
- Enterprise customers get policy-as-code, OTLP export, and legal hold without re-architecting.
- Adding a new lane is a per-mode opt-in; existing modes don't pay until they upgrade.
- One install can host workspaces of different modes (developer + team + enterprise) cleanly.

### Negative

- Six modes is non-trivial surface. Mitigated by `actant doctor --mode` which lists exactly which lanes are running for the current workspace.
- Switching modes is not free: enabling enterprise on an existing `team` workspace runs a backfill of audit evidence. Documented; one-time.
- Mode-related bugs (lane forgot to gate on the mode flag) leak features into the wrong tier. Mitigated by CI: every lane has a test that asserts it doesn't run in modes that haven't enabled it.

### Neutral / open

- Whether the `research` mode is a flag on `developer` or a peer mode is unresolved. Phase 4 picks; Phase 1 ships only `local-fast` + `developer`.

## Alternatives considered

- **No modes; everyone gets everything.** Rejected — kills local-fast experience.
- **Modes per command.** Rejected — too fine-grained; developers can't reason about what's enabled.
- **Modes per workspace.** Adopted — matches the multi-tenant boundary we already have.

## References

- `/specs/19-performance-architecture.md` §11.
- `/planning/cli-templates.md` — template default modes.
- `/planning/lane-catalog.md` — which lanes belong in which mode.
