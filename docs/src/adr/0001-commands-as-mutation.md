# ADR-0001: Commands as the only mutation path

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

ActantDB has many tables and many actors. In a typical OLTP system, each component would issue its own SQL `INSERT` / `UPDATE` against the tables it owns. That works for a system whose primary purpose is storing rows.

ActantDB's primary purpose is producing a **governed, replayable, attributable record of autonomous action.** That changes the requirements:

- Every change to state must be attributable to a single actor.
- Every change must be checked against authority.
- Every change must produce one or more `agent_event` rows that chain into the Chronicle.
- Replay must be able to reconstruct exactly how state arrived where it is.

These requirements are *technically* satisfiable via discipline (each component remembers to log, check authority, and append events), but only structurally satisfiable by removing the option to mutate state any other way.

`/specs/05-security-model.md` §2 invariant 1 states: **No mutation without a command.**

## Decision

ActantDB exposes a single typed mutation surface — the **Command Engine** — and forbids direct projection-table writes outside it. Every mutation runs through:

1. authentication (actor)
2. validation (input schema)
3. authorization (Guard)
4. transactional execution (Storage `Transaction`)
5. event emission (Chronicle)
6. subscriber notification

The pattern is inspired by SpacetimeDB's *reducers* — but ActantDB commands are specialized for agent workflows (with explicit authority, sensitivity, and risk-level semantics).

## Consequences

### Positive

- The invariant "every mutation has an attributable, authorized origin" is structurally enforced.
- Replay is straightforward: every state change has a `command_record` and a chain of events.
- New mutations require an explicit, reviewed command spec.
- Tooling (Studio, audit exports, debugging) sees a uniform mutation surface.

### Negative

- More code per mutation than direct SQL. Mitigated by a small `Command` trait and codegen for SDK methods.
- Cross-cutting changes (a single user action that touches many tables) become "fat" commands. We accept this cost; the alternative is opaque dependency chains across mini-commands.

### Neutral / open

- The set of commands grows over time. Backward compatibility is a real concern; covered by versioning the command catalog and codegen.

## Alternatives considered

- **Direct table writes per crate.** Rejected because invariant 1 becomes a code-review concern rather than a structural property. We want the structural property.
- **Generic `mutate(table, payload)` API.** Rejected because it loses input schema, authority, and event emission. Equivalent in flexibility to direct writes, with all the same problems.
- **Event-sourcing-only (no projections).** Rejected because every read would require event replay. Projections are derived state for query, not the source of truth.

## References

- `/specs/01-architecture.md` §"Command Engine"
- `/specs/03-command-spec.md`
- `/specs/05-security-model.md` §2 invariant 1
- SpacetimeDB reducers (design inspiration; clean-room implementation)
