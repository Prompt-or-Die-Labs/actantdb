# ADR-0007: Trust is calibrated by behavior

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Static permissions assume actors stay equally trustworthy over time. They do not. A model that produced invalid tool arguments 11 times last week should not be granted the same unattended `shell.run` it was granted last month. A subagent with a 92 % approval-grant rate is differently risky from one with 18 %.

Authority in `/specs/05-security-model.md` §5 is multidimensional but currently static. The Actant Contract names this gap (`/specs/13-actant-contract.md` §19). We need a way for authority to reflect behavior — without requiring a human to edit scopes by hand.

## Decision

ActantDB maintains a **`trust_profile`** per `(actor_id, capability_area)`. The profile aggregates operational signals:

- tool success rate
- policy violation rate
- approval denial rate
- memory correction rate
- workflow completion rate
- explicit user feedback
- eval scores
- replay divergence

A score in `[0.0, 1.0]` is updated periodically (`recalculate_trust` runs on a workspace cadence) with an associated `confidence` reflecting `sample_size`.

Guard MAY consult the trust profile when evaluating a request. The effect is conservative: low trust can escalate `risk_level` (turning an `allow` into `allow_with_approval`); it does not by itself grant authority. Trust never *adds* permission — it can only ask for more scrutiny.

When trust crosses a workspace-defined threshold, `trust_downgrade` (or `trust_upgrade`) events fire. Subscribers see them; policy can react.

## Consequences

### Positive

- Authority adapts to behavior without manual scope edits.
- A new agent earns autonomy. A misbehaving agent loses it. This matches how teams trust contractors.
- The signal is fully owned by ActantDB. It does not require integration with external telemetry.
- Trust pinning by an admin (`pin_trust`) is itself an audited action, so overrides are visible.

### Negative

- Behavioral trust is itself a system that can be gamed by adversarial agents. Mitigated by treating trust as *advisory* — it can only escalate, never relax. A high-trust actor still faces every Guard check; trust just doesn't add a step.
- Trust profile computation introduces compute load. Mitigated by computing per capability_area, on a cadence, with stable inputs.
- Trust dropouts (sudden score collapse) can cause user-visible behavior change. Mitigated by surface in Studio + a `trust_downgrade` notification.

### Neutral / open

- Whether trust should affect autonomy budgets (low-trust actors get smaller budgets) is deferred to a separate ADR if the need arises.
- Cross-workspace trust portability is out of scope. Trust is per-workspace by design.

## Alternatives considered

- **Hard-coded penalty tables.** Rejected — too rigid; every workspace has different signal weights.
- **Reputation systems with peer voting.** Rejected — interesting but speculative; ActantDB needs a substrate-level mechanism that does not depend on a peer network existing.
- **No behavioral trust; rely on humans to revoke scopes.** Rejected — does not scale and is unfair to the user (the system saw the failure pattern faster than the human will).

## References

- `/specs/13-actant-contract.md` §19 (behavioral trust)
- `/specs/14-extended-primitives.md` §10 (trust_profile schema)
- `/specs/05-security-model.md` §5 (authority — trust composes with scopes, does not replace them)
