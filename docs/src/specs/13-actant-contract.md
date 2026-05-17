# 13 — The Actant Contract

This document is the **deeper framing** of ActantDB. Specs 00–12 define the system at the level of subsystems, schemas, and commands. This file states what ActantDB *is*, why the previous chapters are not optional, and what makes the system genuinely novel rather than a recombination of existing parts.

Readers should treat this file as authoritative on the system's *intent*. When a lower-level spec is silent, the Actant Contract decides.

---

## 1. The category claim

ActantDB is **the operating substrate for accountable autonomous action.**

It is not:

- a database for agents,
- an agent framework,
- a workflow engine,
- a vector memory layer,
- a trace store,
- an approval queue,

though it can host or replace each of those.

It is the layer **underneath** all of them, with one job:

> Turn autonomous behavior into structured, governed, replayable state.

## 2. The system invariant

Every autonomous action must be:

```
attributable
permissioned
contextualized
replayable
inspectable
reversible where possible
and connected to memory, tools, workflow, and policy.
```

If a system cannot answer those, it is not safe enough for serious autonomous agents.

## 3. Actants

ActantDB calls every entity capable of action an **actant**:

```
human            tool                browser session
agent            workflow            shell process
subagent         worker              scheduler
model            memory extractor    policy engine
                                     critic model
```

Most agent systems overload the word "agent" — they say "the agent did X" when actually the planner *proposed* X, the policy engine *did not block* X, the human *approved* X, and the shell worker *performed* X.

ActantDB makes each of those a separate first-class identity with its own:

- authority
- scope
- capabilities
- trust profile
- runtime state
- audit history

Every action in the Chronicle names exactly one acting actant. Accountability is therefore distributed, not collapsed onto a vague "the agent."

See `02-data-model.sql` `actor` (with `kind`) and `14-extended-primitives.md` `trust_profile`.

## 4. The eight obligations: the Actant Contract

Every operation must answer:

1. **Actor.** Who is acting?
2. **Intent.** What are they trying to accomplish?
3. **Context.** What information are they using?
4. **Authority.** What are they allowed to do?
5. **Action.** What side effect or state change is requested?
6. **Observation.** What happened?
7. **Memory.** What should be remembered or updated?
8. **Replay.** Can we reconstruct or rerun this?

This is the **Actant Contract**.

A subsystem, command, or product feature that cannot satisfy the contract is structurally rejected — it cannot ship.

## 5. The behavioral loop

Most systems model agents as a transcript:

```
conversation → model call → tool call → log result
```

ActantDB models agents as a **closed feedback loop**:

```
actant perceives
→ ActantDB builds governed context
→ model / tool / workflow proposes action
→ ActantDB checks authority
→ effect is approved, leased, executed
→ observation returns as structured evidence
→ memory candidate emerges
→ workflow advances
→ event graph grows
→ replay / eval / improvement become possible
```

Mapped to a nervous-system metaphor:

```
sensation    = observations           (see 14 §"observation")
perception   = context builds         (see 06)
decision     = model calls / commands (see 03)
action       = effects                (see 04)
feedback     = results, regrets       (see 14 §"regret")
learning     = memories, evals, policy updates (see 06, 14)
```

The novelty is not any single arrow. It is that every arrow is closed:

```
No effect without observation.
No observation without potential learning.
No learning without review or policy.
No policy change without audit.
```

## 6. Intent is separate from action

A single layer is what lets the system tell apart:

- The model wants to **inspect** a file.
- The model wants to **modify** a file.
- The model wants to **exfiltrate** a file.
- The model wants to **summarize** a file locally.

These look identical at the tool-call layer. They are different at the **intent** layer.

ActantDB inserts an explicit intent record before any tool call (see `14-extended-primitives.md` §"Intent"). Guard's check therefore includes an **intent–action alignment** check:

```
Declared intent:        inspect test failures
Proposed tool call:     rm -rf ~/.ssh
Verdict:                intent mismatch → block and escalate
```

This is not permission checking. It is **semantic action alignment**.

## 7. Observations are structured evidence

Tool results are not strings. They are observations with:

```
source_effect_id
evidence_type
summary
raw_artifact_ref
confidence
sensitivity
created_by_worker
verification_status
downstream_uses
```

Memory rows reference observations as evidence. The system can therefore say:

> "I know this repo uses pytest because observation `obs_5e9` recorded `pytest collected 42 tests` from effect `eff_123` on 2026-05-17."

That is **evidence-backed memory** — not "the agent remembers something it saw."

## 8. Sensitivity travels

The single most underrated property in the extended model is **sensitivity lineage**: sensitivity (and visibility, residency, retention) flow through transformations.

```
source record
→ context item
→ model call
→ tool call
→ artifact
→ memory candidate
→ approved memory
→ sync replication
```

Sensitivity can be *upgraded* in transit:

```
low + personal identifier   = medium
medium + health context     = high
high + token-like string    = secret
```

Privacy enforcement at ingestion is necessary but not sufficient. Sensitivity must be a property of derivations, not just sources.

The mechanism: **data capsules** (see `14-extended-primitives.md` §"Capsule"). Any value derived from a capsule inherits the capsule's policy.

## 9. Authority is a calculus, not a flag

Authority in ActantDB is multidimensional:

```
who
can do what
to which resource
under which sensitivity ceiling
for how long
with what approval mode
for which model visibility
```

A scope like "this memory may be used locally but not in cloud prompts" — or "this tool may run unattended only inside this workflow" — is expressible. A binary `allowed: true/false` cannot do this.

Authority is therefore a calculus over `(actor × permission × resource × sensitivity × visibility × time × approval_mode × budget)` — see `05-security-model.md` §5 and `14-extended-primitives.md` §"Budget" / §"Delegation".

## 10. Approval is durable infrastructure

Approval is not a UI popup. It is a first-class state machine with:

```
risk summary
redacted arguments
policy snapshot
requesting actor
required permission
expiration
available scopes
decision
```

Approval modes go beyond yes/no:

```
deny
allow_once
allow_for_session
allow_for_workflow
allow_for_resource
allow_for_actor
allow_until_expiry
allow_read_only_variant
```

And approvals carry **reversibility metadata** so the human sees:

- "This file write can be reverted."
- "This email send cannot be undone."
- "This shell command has no automatic compensation."

See `14-extended-primitives.md` §"Compensation plan" for the compensation model.

## 11. Effects are supervised objects

The Effect Engine already separates intent from execution (`04-effect-protocol.md`). The Actant Contract adds:

- Every effect carries a **lease** — a time-bounded, idempotency-bounded mandate (see ADR-0008 in `14-extended-primitives.md`).
- Workers receive **policy-routed** effect leases — the same `tool.shell.run` can be routed to a local Mac, a Docker sandbox, or refused based on the calling context.
- Long-running effects stream **observations** as new chronicle rows, so subscribers see progress.

Compromised workers have limited power: they cannot alter arguments, escalate permission, reuse leases, or access unrelated resources.

## 12. The chronicle is causal

`agent_event` rows have `parent_event_id` (`02-data-model.sql`). The extended model adds `causal_parent_ids` — a set, not a single value — so the chronicle is a **causal DAG**, not a chain.

This is what makes counterfactual questions tractable:

- "Show me every downstream action caused by this memory."
- "Show me every tool call influenced by this context item."
- "Show me every workflow run that used this revoked permission."

Logs are chronological. The Chronicle is causal. Causal observability is the difference between an audit trail and a debugger.

## 13. Memory is a hypothesis

The phrase **memory is hypothesis** captures the design rule:

```
memory ≠ text
memory = evidence-backed belief usable under policy
```

A memory carries:

```
text                     allowed_contexts
category                 forbidden_contexts
confidence               embedding_refs
sensitivity              usage_history
source_events            last_verified
evidence_pointers        expires_at
```

The system can answer:

- "Why do you believe this?"
- "When did you learn it?"
- "Where may it be used?"
- "How is it doing?" (usefulness vs harm score)
- "Does it conflict with newer memory?"

See `06-context-and-memory.md` §5–§9 and the new memory-conflict primitive in `14-extended-primitives.md` §"Memory conflict".

## 14. Replay is counterfactual

`07-workflows-and-replay.md` already specifies replay modes. The Actant Contract elevates replay from a debugging tool to a **first-class loop closure**:

```
production failure
→ replay diff
→ regret event
→ eval case
→ policy update or memory demotion
→ tested by future replay
```

Failures become evals automatically. See `14-extended-primitives.md` §"Regret" and §"Eval case".

## 15. Bounded autonomy

An agent is not safe simply because tokens are cheap. ActantDB introduces **autonomy budgets** beyond model cost:

```
token budget               approval budget
cost budget                memory-write budget
time budget                file-write budget
tool-call budget           external-message budget
risk budget
```

A bounded actant can:

- proceed within budget,
- escalate when near limit,
- halt when exceeded,
- request more from a delegating actant.

This is the structural answer to "agent runaway." See `14-extended-primitives.md` §"Budget".

## 16. Delegation is authority transfer

Subagents do not inherit context implicitly. A `delegation` record names:

```
parent_actor_id          deadline
child_actor_id           return_channel
goal                     status
allowed_context_refs
authority_scope_ids
budget
```

The system can inspect:

- What did this subagent know?
- What could it do?
- What did it actually do?
- What did it report back?

Least-privilege subagents become structurally enforceable.

## 17. Intervention is native

Humans can intervene at structured points — and **interventions enter the causal graph** like any other action:

```
pause session            change model
edit memory              add instruction
modify context           restrict permission
deny tool                fork replay
                         take over workflow node
```

Interventions are not metadata; they are commands. Replay can reproduce a workflow with or without an intervention.

## 18. Drift detection

When an agent's actions stop matching its declared intent, that is **autonomy drift**:

```
task:  summarize repo
agent: starts installing packages

task:  read calendar
agent: drafts emails

task:  fix tests
agent: modifies unrelated files
```

ActantDB computes a drift score from `(declared intent vs proposed effects, resources touched, risk escalation, unusual tool sequence)` and can interrupt — or require fresh approval — when the score crosses a threshold.

This is **autonomy boundary enforcement** built into the substrate.

## 19. Trust is behavioral

A subagent or model should not retain the same authority forever. Trust profiles update from operational signals:

```
tool success rate            workflow completion rate
policy violations            user feedback
approval denial rate         eval scores
memory correction rate       replay divergence
```

ActantDB can then say "this model often produces invalid tool args; require a repair model" — without a human changing every scope by hand.

## 20. Subscriptions coordinate, not just render

Subscriptions exist in `01-architecture.md` §4 for live UIs. The extended model recognizes they are also a **coordination fabric**:

```
workers          subscribe to claimable effects
humans           subscribe to pending approvals
agents           subscribe to assigned tasks
memory reviewers subscribe to memory candidates
watchdogs        subscribe to stale heartbeats
evaluators       subscribe to failed workflows
```

A worker does not need to know any agent. It subscribes to the queue. A reviewer does not need to know any workflow. They subscribe to memory candidates. This is how autonomous systems scale without becoming spaghetti.

## 21. What this is *not*

| Not                              | Because                                                                |
| -------------------------------- | ---------------------------------------------------------------------- |
| An agent framework               | It hosts agents; it does not prescribe their reasoning style.          |
| A vector database                | Embeddings live in a companion store; ActantDB references and governs. |
| A trace store                    | Tracing records; ActantDB **governs**.                                 |
| A workflow engine                | Workflows are users of the substrate, not its replacement.             |
| An approval queue                | Approvals are state, not a queue; they participate in causality.       |
| A general-purpose database       | The schema and APIs are agent-specific. Use Postgres for OLTP.         |

## 22. What is genuinely novel

No single primitive here is unprecedented in isolation. The novelty is the **unification**:

- commands as autonomous state transitions
- effects as supervised side effects
- context as a governed manifest
- memory as evidence-backed hypothesis under policy
- authority as a multidimensional calculus
- replay as counterfactual execution
- subscriptions as a coordination fabric
- chronicle as a causal DAG
- intent as a separate layer from action
- capsules as policy-carrying derivable content
- budgets as authority spent, not just tokens
- regret hooks turning failure into eval
- drift detection as a structural safety primitive
- trust profiles updated by behavior

The combination is what we call ActantDB.

## 23. Final mental model

```
actant perceives
→ ActantDB builds governed context
→ model/tool/workflow proposes action
→ ActantDB checks authority and intent alignment
→ effect is approved, leased, executed
→ observation returns as evidence
→ memory candidate emerges
→ workflow advances
→ causal graph grows
→ replay, eval, regret-driven improvement become possible
```

The shortest description:

> **ActantDB turns autonomous behavior into structured, governed, replayable state.**

## Verification

- [ ] Every one of the eight obligations in §4 maps to at least one command in `03-command-spec.md` or `14-extended-primitives.md`.
- [ ] Every primitive named in §22 is defined in either `02-data-model.sql` (existing) or `14-extended-primitives.md` (new).
- [ ] Every "ActantDB is not …" claim in §21 is enforced by an invariant in `05-security-model.md` §2 or a structural rule in this file.
- [ ] No subsystem may ship a command that violates the Actant Contract in §4.
