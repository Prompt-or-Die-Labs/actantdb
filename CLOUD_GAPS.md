# CLOUD_GAPS — what ActantDB Cloud needs that the self-host substrate doesn't

This file maps every gap between **today's self-hostable ActantDB** and a
**hosted product** that competes with Supabase Cloud, Convex Cloud, Vercel,
and Upstash. The self-host substrate is in [GAPS.md](./GAPS.md) — that's the
*free OSS* surface. CLOUD_GAPS is everything required to charge money for a
managed version on top of it.

Cross-reference: [docs/CLOUD_ROADMAP.md](./docs/CLOUD_ROADMAP.md) for the
phasing rationale. [GAPS.md](./GAPS.md) for what's already in the substrate.
[COMPARISON.md](./COMPARISON.md) for product positioning.

## Status legend

| Symbol | Meaning |
|---|---|
| 🟢 **substrate covers it** | The OSS code already has what we need; the cloud just plugs it in. |
| 🚧 **partial in substrate** | Something exists; hardening / wrapping required to ship. |
| 🛣 **Phase 2 — control plane** | Must ship for *any* paid hosting at all. The minimum viable cloud. |
| 🌐 **Phase 3 — differentiated** | Ships after Phase 2 lands. Things only the hosted version can do. |
| 💼 **business-side** | Not engineering — legal, marketing, finance, ops. Wes-scope. |
| 📋 **compliance** | Audit-driven; SOC2, GDPR, HIPAA, etc. |

Last updated: 2026-05-19. Substrate at `@actantdb/all@0.0.13`. Cloud at:
*zero — not yet started.*

---

## Part A — Phase 2 control plane (the minimum to host)

Without every row here, "ActantDB Cloud" cannot run. None of these are
deferrable past the launch of any paid offering.

| # | Component | Status | Notes |
|---|---|--------|-------|
| C1  | **Multi-tenant workspace boundary** | 🚧 | `crates/actant-tenant/` exists but is not hardened against an adversarial tenant. Hot-path checks needed: every command, every event read, every storage write asserts `workspace_id` matches the authenticated actor's claims. Need a `tenant_isolation_property_test.rs` running fuzz across all routes. |
| C2  | **Auth: linking-code + password (existing)** | 🟢 | Per [`UI_AUTH_DESIGN.md`](./UI_AUTH_DESIGN.md), `crates/actant-auth/` ships argon2id + HttpOnly cookie + CSRF + linking code. Reusable for cloud sign-up. |
| C3  | **Auth: OAuth providers (Google / GitHub / Apple / Microsoft)** | 🛣 | GAPS row #32. OIDC token verify exists; provider clients (callback URLs, secret rotation) are new code. Needs `actant-auth/src/oauth/{google,github,apple,microsoft}.rs` + Studio sign-in buttons. |
| C4  | **Auth: SSO / SAML for enterprise tier** | 🛣 | Most managed-product enterprise revenue gates on this. Use `samlify` or wrap a hosted SaaS like WorkOS. Defer to first enterprise lead — but track explicitly so it doesn't surprise. |
| C5  | **Auth: invite team members to a workspace** | 🛣 | Invite link → existing user → adds to `workspace_owner` (or new `workspace_member` table with role). New migration + 3 endpoints. |
| C6  | **Auth: API tokens with scope** | 🛣 | Long-lived bearer tokens for CI / agents. Scopes: `read:events`, `write:commands`, `admin:workspace`. New `api_token` table + middleware. |
| C7  | **Billing — Stripe integration** | 🛣 | Stripe Customer / Subscription / Usage Record API. New `actant-billing` crate or external service. Needs webhooks → mark workspace `subscription_status` on Stripe events. |
| C8  | **Billing — metering** | 🛣 | Capture every billable unit (CPU-second, storage-byte-hour, request, event-row written) and aggregate per workspace per billing period. Writes go to a new `usage_meter` table. Hot path so this has to be cheap. |
| C9  | **Quotas + rate limits per workspace** | 🚧 | `actant-reliability::throttle` exists. Wire it per workspace in the API gateway layer. Quota types: requests/sec, storage MB, monthly compute minutes. |
| C10 | **Hosted runtime — microVM / container per Box** | 🛣 | New `actant-runtime-host` crate. Options: Firecracker (AWS Lambda-style), Cloud Run, Cloudflare Workers + Durable Objects (for state), runc directly on EC2. Decision-driving constraint: cold-start budget. Per `BENCHMARKS.md`, local Box does 7 ms. Hosted should target ≤ 200 ms p50. |
| C11 | **Warm pool for cold-start optimization** | 🛣 | Pre-boot N idle boxes per region; serve a warmed-up one on `Box.create`. Cuts user-visible cold-start to the request RTT. Cost: idle compute. Manage with a token-bucket-style replenisher. |
| C12 | **Control-plane API** | 🛣 | New routes under `/v1/cloud/*`: `box/create`, `box/list`, `box/{id}/delete`, `box/{id}/snapshot`, `workspace/create`, `workspace/members`, `billing/usage`, `region/list`. Drafted in [`docs/CLOUD_ROADMAP.md`](./docs/CLOUD_ROADMAP.md), not implemented. |
| C13 | **Public DNS + automatic TLS** | 🛣 | Per-workspace subdomain (`<ws>.actantdb.cloud`) or custom domain. Caddy or Cloudflare Origin Rules + auto-issued Let's Encrypt certs. Wildcard cert for the platform domain. |
| C14 | **Region selection (US, EU, AP)** | 🛣 | Workspace pins a region at create. Data residency = region's storage. Cross-region replication is out of scope for Phase 2. Need at least US-East + EU-West to claim European customers. |
| C15 | **Snapshot storage (S3-backed, content-addressed)** | 🟢 | `crates/actant-objectstore` already ships `S3Store` + content-hash keys. The cloud just plumbs a per-tenant S3 prefix. |
| C16 | **Connection pooler for the Postgres path** | 🛣 | GAPS row #33. PgBouncer or Supavisor in front of the per-region Postgres. |
| C17 | **Cron / schedule executor at scale** | 🟢 | `actant-trigger::Scheduler` exists. Hosted variant runs one tick loop per region, fan-outs to per-workspace queues. |
| C18 | **Reverse proxy / API gateway** | 🛣 | Routes `<ws>.actantdb.cloud` → the right region's backend pod. Caddy / Envoy / Cloudflare Workers. Handles auth gate before backend hop. |
| C19 | **Egress / ingress IP allowlist** | 🛣 | Customer-configurable IP allowlists for their workspace's API endpoint. New `workspace_network_policy` table. |
| C20 | **Workspace lifecycle: create → suspend → delete** | 🚧 | `actant-server` has the bits; the cloud needs the workflow: provision storage → spin pods → emit billing-start → on delete, drain → snapshot → tear down → emit billing-stop. |
| C21 | **Status page (status.actantdb.cloud)** | 🛣 | Hosted Statuspage or a static-site generator + uptime checks. Phase 2 needs it before announcing GA — customers expect one. |
| C22 | **Health-check + auto-restart loop** | 🚧 | `/v1/healthz/{live,ready}` exists in `actant-server`. Cloud needs the orchestrator (k8s liveness probes, ECS health checks, etc.) wired against them with proper restart policy. |

**Part A totals:** 3 🟢, 4 🚧, **15 🛣 (the actual Phase 2 backlog)**.

---

## Part B — Phase 3 differentiated features

Things only ActantDB Cloud can do once Phase 2 lands. Where competitors
have nothing comparable; these are the reason to pay.

| # | Component | Status | Notes |
|---|---|--------|-------|
| D1 | **Replay-as-a-service** | 🌐 | Paste an event ID into the hosted Studio, get a re-run under override (different policy / memory / tool). The substrate ships the replay engine; the differentiator is the hosted UI + per-org sandbox to run replays without a local install. |
| D2 | **Audit-export to consumer's own S3** | 🌐 | `actant-sync` already implements this via `actant-objectstore`. Cloud surfaces the config in the dashboard + supplies the IAM role assume / OIDC trust setup. |
| D3 | **Approval webhooks** | 🌐 | `approval_required` event → Slack / Linear / email / generic HTTP. New `approval_webhook` table + delivery worker. Backed by `actant-reliability::circuit` + `throttle`. |
| D4 | **Hosted policy registry** | 🌐 | Central policy doc signed by ActantDB Cloud (`iss == workspace_id`). Consumers verify locally with the cached pubkey. Lets multiple agents in the same org share one policy doc and get rotation for free. |
| D5 | **Cross-org replay** | 🌐 | Replay the same anchored event under a *different* tenant's policy + memory. Useful for vendor compliance reviews ("what would my policy have done with your run?"). Privacy story: only the *decision diff* leaves the source tenant. |
| D6 | **Branching / preview deployments** | 🌐 | GAPS row #35. Create a workspace branch from a snapshot; PRs get a preview URL. Schema migrations land in the branch first. Convex + Supabase both ship this. |
| D7 | **Hosted log streaming + search UI** | 🌐 | GAPS row #34. OTLP receiver → ClickHouse or Tempo + a search UI in Studio. Substrate exports OTLP today; the UI is new. |
| D8 | **Metrics dashboards** | 🌐 | TPS, latency p50/p95/p99, errors, cost/run, top tools by frequency. Backed by Prometheus or VictoriaMetrics scraping `/metrics`. New Studio panels. |
| D9 | **Templated cloud workspace from a GitHub repo** | 🌐 | `actantdb init --from gh:org/repo` clones a template repo into a new hosted workspace + scaffolds the initial event ledger. Tied to GAPS row #25. |
| D10 | **Free-tier policy registry** | 🌐 | Publish vetted policy docs (`@actantdb/policy-pack-coding`, `@actantdb/policy-pack-support`) that consumers can adopt with one click. |
| D11 | **Replay scrubber for compliance** | 🌐 | Per-event redaction (e.g. PII strings) on export. The capsule sensitivity ceiling already prevents secrets from reaching the ledger; this is the export-side equivalent. |

**Part B totals:** 11 🌐 (all Phase 3).

---

## Part C — Operations

The non-feature work to actually keep the cloud running.

| # | Component | Status | Notes |
|---|---|--------|-------|
| O1 | **CI/CD for cloud deploys** | 🛣 | Separate pipeline from the OSS `ci.yml` — cloud has prod/staging environments, blue-green, rollback. |
| O2 | **Infrastructure-as-code** | 🛣 | Terraform or Pulumi for all hosted infra. Region kubernetes clusters, VPCs, S3 buckets, RDS, Cloudflare config. |
| O3 | **Monitoring + alerting** | 🛣 | Prometheus + Alertmanager (or Datadog). SLOs: API p99 < 1 s, Box cold-start p99 < 1 s, uptime ≥ 99.9% Phase-2 / ≥ 99.95% Phase-3. |
| O4 | **On-call rotation + runbooks** | 💼 | PagerDuty or Opsgenie. Runbooks for: workspace stuck in "provisioning", billing webhook failure, database CPU pegged, region down. |
| O5 | **Backup + disaster recovery** | 🚧 | `actantdb backup --mode=incremental` ships per GAPS row #21. Cloud needs the cross-region replica + the actual restore drill in CI. |
| O6 | **Secret management** | 🛣 | Where do Stripe keys, OAuth client secrets, region Postgres passwords live? AWS Secrets Manager or Vault. Operator runbook for rotation. |
| O7 | **Logging pipeline (cloud-side)** | 🛣 | Distinct from D7. The control-plane's own logs (not customer logs). Goes to whatever ops team uses (Datadog, Honeycomb). |
| O8 | **Audit log of admin actions** | 🛣 | Every action a cloud operator takes (impersonate a workspace, refund a customer, override a quota) lands as a typed event. We have the substrate for this — apply it inside our own admin tool. |
| O9 | **Incident response process** | 💼 | Status-page comms, postmortem template, customer notification flow. |

**Part C totals:** 1 🚧, 6 🛣, 2 💼.

---

## Part D — Business surface

Not engineering; tracked here so the eng team knows the dependencies exist.

| # | Item | Status | Notes |
|---|---|--------|-------|
| B1 | **Pricing model** | 💼 | Free tier definition + paid tier prices. Has to land before any signup flow. |
| B2 | **Billing math validation** | 💼 | Stripe rules around aggregation, proration, refunds. Tax handling per region. |
| B3 | **Marketing site (`actantdb.com` vs `actantdb.dev`)** | 💼 | Landing page, pricing page, customer logos when we have them. |
| B4 | **Onboarding flow (sign-up → first project → first run)** | 💼 | Welcome email, in-app wizard. Convex's is the bar. |
| B5 | **Documentation: cloud-specific** | 💼 | "How to deploy to cloud" + "How to migrate from self-host" — distinct from the OSS docs. |
| B6 | **Support tiering** | 💼 | Community (Discord/GitHub) vs paid email vs enterprise SLA. |
| B7 | **Legal: Terms of Service** | 💼 | Lawyer-reviewed ToS. |
| B8 | **Legal: Privacy Policy** | 💼 | Lawyer-reviewed Privacy Policy. |
| B9 | **Legal: Data Processing Agreement (DPA)** | 💼 | Required for EU customers under GDPR. |
| B10 | **Legal: Sub-processor list** | 💼 | Required for SOC2 / GDPR. Stripe + AWS/GCP + Cloudflare + whatever SAML vendor. |
| B11 | **Customer-success runbook** | 💼 | First 5 paid customers get a real human touch. Doc the flow. |
| B12 | **Refund / chargeback policy** | 💼 | What's the policy when a customer disputes a charge? |

**Part D totals:** 12 💼.

---

## Part E — Compliance

The audit-driven work. None of it is hard engineering; all of it is
deliberate process + evidence collection.

| # | Item | Status | Notes |
|---|---|--------|-------|
| E1 | **SOC2 Type I** | 📋 | ~3 months from a clean process baseline. Vanta / Drata / Secureframe automate the evidence collection. |
| E2 | **SOC2 Type II** | 📋 | 6-month observation window after Type I. Required for most B2B customers > $50k/yr ACV. |
| E3 | **GDPR data-residency claims** | 📋 | Per Part A C14 (region selection). Document the data flow that proves EU data stays in EU. |
| E4 | **HIPAA Business Associate Agreement** | 📋 | Only if/when a healthcare customer asks. Substrate audit-trail features are useful here. |
| E5 | **ISO 27001** | 📋 | Defer until SOC2 Type II ships. Substantial overlap; tackle as a follow-up. |
| E6 | **Penetration test (external)** | 📋 | Annual third-party pentest. Required for SOC2 + most enterprise procurement. |
| E7 | **Vulnerability disclosure program** | 📋 | `SECURITY.md` + a documented response SLA. Optional bug bounty (HackerOne / Bugcrowd) later. |
| E8 | **Data Subject Request workflow** | 📋 | GDPR / CCPA: customer says "delete my data" → workflow that cascades through every region + tenant + audit log. |

**Part E totals:** 8 📋.

---

## Overall tally

| Status | Count | Action |
|---|---:|---|
| 🟢 substrate covers it | **3** | Wire into the cloud control plane |
| 🚧 partial in substrate | **5** | Harden + wrap |
| 🛣 Phase 2 backlog | **16** | The critical path to launching paid hosting (incl. new F7 Mailpit) |
| 🌐 Phase 3 backlog | **17** | After Phase 2; the "why pay" features (incl. new F1–F6) |
| 💼 business-side | **14** | Company/process concerns |
| 📋 compliance | **8** | Audit + process |
| **Total rows** | **63** | |

## Phase 2 critical path (the 15 🛣 in Part A, ordered)

In dependency order — each item assumes the prior is in place:

1. **C13** Public DNS + TLS (without this nothing else is reachable).
2. **C18** Reverse proxy / API gateway (route requests to the right pod).
3. **C12** Control-plane API (`/v1/cloud/*` endpoints).
4. **C20** Workspace lifecycle workflow (provision → suspend → delete).
5. **C10** Hosted runtime — microVM / container per Box.
6. **C11** Warm pool optimization (cold-start budget).
7. **C16** Connection pooler in front of Postgres.
8. **C8** Metering (start writing usage rows before charging anyone).
9. **C7** Stripe billing integration (consume the meter).
10. **C9** Quotas + rate limits per workspace.
11. **C3** OAuth providers (Google + GitHub at minimum — most consumer flows).
12. **C5** Workspace member invites (most cloud customers are teams).
13. **C6** API tokens with scope.
14. **C14** Region selection (US-East first, then EU-West for European launch).
15. **C19** Egress / ingress IP allowlist (enterprise expectation).

**C4 SAML and C21 status page** are not on the critical path — they can land
between C9 and GA.

Estimated effort (per item): ranges from 2 days (C13 DNS+TLS via Cloudflare)
to 4 weeks (C10 hosted runtime). The honest budget for Phase 2 is **8–12
engineer-weeks** assuming one full-time engineer and using off-the-shelf
infrastructure (Cloudflare + AWS + Stripe + WorkOS) rather than rolling
everything ourselves.

## Part F — Big-ticket features previously framed as anti-scope

Reclassified: every item below is something we DO want to ship as part
of the Cloud product story. They live here so the Cloud roadmap accounts
for them; the substrate work for each is tracked in
[`DEVX_GAPS.md`](./DEVX_GAPS.md) Part K.

| # | Item | Status | Notes |
|---|---|--------|-------|
| F1 | **Auto-generated REST API from schema** | 🌐 | Substrate work in `DEVX_GAPS.md` X88. Cloud surface: per-workspace REST endpoint at `<ws>.actantdb.cloud/rest/v1`. PostgREST-shape. |
| F2 | **GraphQL endpoint** | 🌐 | Substrate work in `DEVX_GAPS.md` X89. Cloud surface: `<ws>.actantdb.cloud/graphql`. |
| F3 | **Vector DB as primary product** | 🌐 | Substrate in `DEVX_GAPS.md` X90. Cloud: managed embedding-model registry per workspace + usage metering. Direct comparator: Pinecone, Weaviate Cloud. |
| F4 | **Visual workflow canvas + no-code agent builder** | 🌐 | Substrate in `DEVX_GAPS.md` X91 + X95. Cloud: hosted canvas, sharing, marketplace of pre-built agents. |
| F5 | **Browser embedded mode** | 🌐 | Substrate in `DEVX_GAPS.md` X92. Cloud: hosted ledger sync for browser-embedded apps (offline-first with cloud as canonical store). |
| F6 | **Generic pub/sub broker** | 🌐 | Substrate in `DEVX_GAPS.md` X93. Cloud: per-workspace topic quotas + Pusher/Ably-style billing. |
| F7 | **Local SMTP catcher (Mailpit)** | 🛣 | Substrate in `DEVX_GAPS.md` X94. Cloud: optional managed SMTP relay per workspace for outbound test mail. |

**Part F totals:** 6 🌐, 1 🛣. Each row also has a corresponding substrate
gap (`DEVX_GAPS.md` X88–X95) that has to ship before the cloud surface can
go live.

## Part G — Cross-OS sync relay (post-CloudKit phase)

CloudKit covers Apple-ecosystem sync (Phase 1; see
[`docs/SYNC_DESIGN.md`](./docs/SYNC_DESIGN.md)). For Android / Linux /
Windows devices or for sharing workspaces between two people, ActantDB
Cloud needs to ship a sync relay. These rows track that work; all are
🌐 Phase 3.

| # | Item | Status | Notes |
|---|---|--------|-------|
| G1 | **ActantDB Cloud sync relay endpoint** | 🌐 | Hosted CKRecord-equivalent — accept event rows from any device, deliver to subscribed devices. WebSocket + REST. Per-workspace sharded; uses the existing `actant-objectstore` (S3) for large payload offload. |
| G2 | **Group / workspace-shared sync** | 🌐 | Two or more humans share one workspace. Auth model: workspace owner invites collaborators (already partially in `actant-auth`); sync relay enforces membership. Conflict policy unchanged (HLC-LWW) — just more devices in the convergence pool. |
| G3 | **Cross-OS device pairing** | 🌐 | Linking-code (CLOUD_GAPS.md C2 substrate) extended to "pair this Android phone with my Mac" flow. QR code on Mac → scan with Android → both devices register against the same relay-side workspace. |
| G4 | **Push notification fan-out** | 🌐 | APNs (iOS), FCM (Android), Web Push (PWAs / Chrome). Triggered by the relay when new events land for a sleeping device. |
| G5 | **Per-device sync quotas + billing** | 🌐 | Hosted sync is paid-tier; quota per workspace (e.g. 10 GB synced data, X devices). Wires into CLOUD_GAPS.md C8 metering. |

**Part G totals:** 5 🌐. Lands after CloudKit Phase 1 proves the
replication semantics and after Phase 2 Cloud control plane lands.

## Cross-link audit (so nothing slips between docs)

| Doc | What lives there | What does NOT live there |
|---|---|---|
| [`GAPS.md`](./GAPS.md) | Self-host substrate gaps + BaaS-parity bar | Anything cloud-specific |
| [`CLOUD_GAPS.md`](./CLOUD_GAPS.md) | This file | Self-host gaps (those are in GAPS) |
| [`docs/CLOUD_ROADMAP.md`](./docs/CLOUD_ROADMAP.md) | Phase 1/2/3 narrative + component table | Detailed gap list |
| [`COMPARISON.md`](./COMPARISON.md) | Substrate-level competitive landscape | Cloud pricing comparison (lives in pricing page when it ships) |
| [`BENCHMARKS.md`](./BENCHMARKS.md) | Substrate performance numbers | Hosted SLA targets |

Any new row that doesn't cleanly fit one of those locations is the trigger
to spin up a new doc, not to stretch an existing one.
