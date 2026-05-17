# Metrics

## The one metric for 60 days

```
completed external replays
```

A **completed external replay** is:

1. a non-Wes developer
2. using Actant on their own agent
3. captured a real run
4. clicked replay (or ran `actant replay run`)
5. learned something useful

If any of those five conditions is missing, it doesn't count.

## Targets

| Day | Threshold |
| --- | --- |
| 60  | **5** completed external replays |
| 90  | **15** completed external replays |

If we hit these, the wedge is real. If we don't, see [`kill-criteria.md`](./kill-criteria.md).

## What we don't optimize

These do not count as success in the first 90 days:

- GitHub stars
- npm downloads (could be CI bots, vanity-installs, exploration)
- HN front page
- Twitter / X engagement
- Slack / Discord member count
- Blog post views
- Author satisfaction with the architecture

A 12-replay count beats a 1000-star count for the same effort. The goal is users doing the thing the product exists for.

## How we count

A small `actant-stats` Studio screen renders the counter, anonymized by `(installation_id, project_id)`. Replays from Wes's own installations are excluded by `installation_id`. Replays from a known design-partner installation are tagged but counted.

Counter is also exportable as JSON for sharing the number in updates.

## Secondary metrics (informational only)

These don't drive the gate, but they're useful to track:

- **Install success rate.** Of attempts at install, what fraction reach "Studio open with a captured run." Target 70 %.
- **Time-to-first-replay.** Median minutes from `npm install @actant/mastra` to first replay clicked. Target ≤ 15 minutes.
- **Repeat replays per user.** How often does a user click replay more than once on the same install. Target ≥ 2x for active users.
- **Replay-after-incident usage.** A specific subset: replays where the user opens Studio because something visibly went wrong in their agent. This is the highest-value usage.

If these secondaries are red while the primary is green, the primary is being padded. Audit before celebrating.

## What this rules out

The metric explicitly rules out becoming popular without being used. A flood of stars from an HN front page where nobody actually installs Actant counts as **zero**. A small but engaged group of developers each clicking replay weekly counts as success.

This metric also rules out optimizing for Wes's own dogfood. Replays from Swoosh sessions don't count. That's by design — Swoosh dogfood is necessary but not sufficient.

## When to revisit the metric

When the wedge passes Gate 3 (Aug 17, 2026), the metric evolves:

- **Replay → Resolved replay.** A replay that produced an action: memory demoted, policy added, eval created, code change committed.
- **Adoption → Daily active projects.** Number of distinct `(installation_id, project_id)` pairs that capture ≥ 1 run per day.
- **Revenue / cost.** If a hosted offering ships post-Gate-3, MRR / margin / churn.

Until Gate 3, just replays.
