# ADR-0004: Intent is a separate layer from action

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

In agent systems, the model's tool calls are usually the first place authority gets checked. By then, a great deal of context has been lost: *what did the agent think it was doing?* Policy decides on the action; the intent that produced the action is invisible.

This creates a class of failures that look identical at the tool-call layer but are different in intent:

- The model wants to **inspect** a file. Safe.
- The model wants to **modify** a file. Restricted.
- The model wants to **exfiltrate** a file. Hostile.
- The model wants to **summarize** a file locally. Safe.

All four can appear as `file.read` or `file.write`. Permission alone cannot tell them apart.

`/specs/13-actant-contract.md` §6 names this gap. `/specs/14-extended-primitives.md` §1 introduces the data shape.

## Decision

ActantDB inserts an explicit **intent layer** between context-build and tool-call. An `intent` row carries the declared goal, the proposed action class, the resource targets, the expected benefit, and the expected risk. Every agent-originated tool call MUST reference an open `intent_id`.

Guard's evaluation now includes an **intent–action alignment** check. A proposed effect that does not plausibly fall within the declared intent's `proposed_action_class` and `resource_targets` is rejected (or escalated to approval) with `decision_reason='intent_mismatch'`. The mismatch is published as an `intent_action_mismatch` event so subscribers and drift detection can react.

Intents are not authoritative — they are *claims by the agent*. The system uses them to bound proposals semantically, not to grant authority. Authority still lives in `authority_scope`.

## Consequences

### Positive

- A class of "looks innocent at the tool layer" failures is structurally caught.
- Drift detection (`drift_signal`) has a stable signal to compute against.
- Audit becomes more useful: every action explains the agent's claimed reason at the time.
- Workflows can record intent at each node, making counterfactual replay sharper.

### Negative

- Agents must form intents. Lightweight agents that previously skipped this step now have to declare. Mitigated: a default intent (`generic_inspect_low_risk`) is auto-formed if the agent omits — and the absence of an explicit intent is itself a signal in the drift score.
- Intent–action alignment is heuristic. False positives will block legitimate work; false negatives will let bad work through. We accept both costs because the baseline (no intent layer) had no signal at all.

### Neutral / open

- Phase 2 ships a simple alignment check (string-class match + resource pattern overlap). Phase 4 may swap in a richer model-assisted alignment evaluator. The data shape supports both.
- Whether intents should be reviewable by humans before tool calls is left to Phase 3 workspace policy.

## Alternatives considered

- **Stronger permission patterns alone.** Rejected — patterns describe what's allowed, not what's intended. They cannot distinguish "inspect" from "exfiltrate" when both use `file.read`.
- **Model self-classification on each tool call.** Rejected as the primary mechanism — embeds the agent's word in the audit trail without separating it from the action. The intent row makes the claim explicit and queryable.
- **Intent inferred post-hoc by an evaluator.** Useful for analysis, but useless for prevention. We want intent declared *before* the action.

## References

- `/specs/13-actant-contract.md` §6 (intent separation)
- `/specs/14-extended-primitives.md` §1 (intent schema), §14 (drift)
- `/specs/05-security-model.md` §7 T1 (prompt injection) — intent–action alignment is a complementary mitigation
