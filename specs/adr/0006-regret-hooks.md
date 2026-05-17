# ADR-0006: Regret hooks — failures become evals automatically

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Agent systems improve when they learn from their failures. The usual pattern — "add a memory after a bad result" — is fragile: memories drift, conflict, expire. Production failures should produce more durable artifacts:

- a corrected memory (with the bad one demoted),
- an updated policy (with the bad behavior blocked),
- a new eval case (with the bad behavior tested against forever after).

The pieces exist independently in most agent stacks. They are wired together by humans, weeks after the failure, often imperfectly. The trace is forgotten.

## Decision

ActantDB introduces **regret events** as first-class rows. Anyone (the user, a critic model, an evaluator agent, an operator) can `file_regret` against a chain of events that led to a bad outcome. The regret carries:

- the bad outcome type (`user_correction`, `tool_failure`, `policy_violation`, `unintended_effect`, …),
- the causal event IDs implicated,
- a suspected failure mode,
- a suggested corrective action (demote memory / add policy / update tool schema / create eval).

A `resolve_regret` command MUST link to at least one concrete corrective action — a new policy row, a `memory_candidate` rejection, an `eval_case`, an updated tool schema. Without that link, the regret cannot be marked resolved.

Where the suggested corrective action is "create eval," ActantDB exposes `create_eval_from_replay`. The system mints an `eval_case` from a replay checkpoint plus expected/forbidden behaviors. The eval runs from then on, every time CI runs or on demand.

## Consequences

### Positive

- Failures convert into durable improvements automatically. The path from "this was bad" to "we have a test that prevents recurrence" is short and visible.
- The regret/eval graph is auditable. We can see what we learned from each failure.
- Reviewers, evaluators, and critic models become first-class participants. Their work appears in the same causal graph as the agent's work.

### Negative

- Regret resolution requires real corrective work. Teams can game the metric by filing low-quality regrets and resolving them with token policy changes. Mitigated by review of resolution links in the audit dashboard.
- Eval cases can drift from the production system if checkpoints get reaped. The `eval_case_orphaned` event surfaces this; orphaned evals are auto-disabled.
- Auto-generated eval criteria from natural-language regret descriptions is hard. Phase 4 starts with a small DSL; richer authoring lives in later phases.

### Neutral / open

- Whether models should be allowed to file regret automatically (without a human in the loop) is a workspace policy. Phase 3 ships with `false`; Phase 4 makes it configurable.
- The relationship between regret events and the existing audit log is one-way: regret events reference audit events, not the reverse. Audit-event clients do not need to know about regrets.

## Alternatives considered

- **Add this in tooling, not in the substrate.** Rejected — the substrate is the only layer that holds the causal graph, the replay engine, and the policy store. Tooling outside the substrate cannot make the connection structurally.
- **Treat regret as a free-form "feedback" record.** Rejected — feedback without a typed corrective action becomes noise. Forcing resolution to link to a structural change keeps the system honest.
- **Skip regret entirely; rely on offline post-mortems.** Rejected — too slow for real autonomous systems running thousands of sessions a day.

## References

- `/specs/13-actant-contract.md` §14 (replay as loop closure)
- `/specs/14-extended-primitives.md` §6 (regret), §7 (eval)
- `/specs/07-workflows-and-replay.md` §10 worked replay examples — these are the seeds of evals
