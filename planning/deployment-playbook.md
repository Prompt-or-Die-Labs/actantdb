# Deployment playbook

Three deployment shapes, all from the same binaries. Phase 1 ships local mode; Phase 6 adds team and cloud.

## Shape A — Local (Phase 1+)

A single user on a single machine.

```
laptop
  ├── actantdb-server (port 7373, SQLite at ~/.actant/db.sqlite)
  ├── actant-worker-shell
  ├── actant-worker-file
  ├── actant-worker-model         (talks to local Ollama / MLX / cloud per route)
  ├── actant-worker-mcp
  └── studio (browser at http://localhost:7373/studio)
```

### Install (Phase 1 alpha)

```
cargo install --path crates/actant-cli
cargo install --path crates/actant-server
actantdb start --db ~/.actant/db.sqlite
```

Workers are launched alongside:

```
actant-worker-shell --server http://localhost:7373 --token $WORKER_TOKEN
actant-worker-file  --server http://localhost:7373 --token $WORKER_TOKEN
actant-worker-model --server http://localhost:7373 --token $WORKER_TOKEN
```

### Backups

`~/.actant/db.sqlite` plus the artifact store directory. The Chronicle's `event_hash` chain makes corruption detectable.

## Shape B — Team self-hosted (Phase 6)

A small team running their own server.

```
internal network
  ├── actantdb-server (3 replicas behind LB)
  ├── Postgres cluster (single primary in Phase 6; multi-AZ in Phase 6.5)
  ├── artifact store (S3-compatible, internal)
  ├── secret vault (HashiCorp Vault / cloud KMS)
  ├── worker fleet (per-kind concurrency limits)
  └── studio (single deployment behind SSO)
```

### Deployment

- Docker image: `actantdb/server:vX.Y.Z`.
- Helm chart under `deploy/helm/`.
- Postgres backend selected via `ACTANT_DB=postgres://...`.
- SSO via OIDC; service accounts for workers.

### Sizing (Phase 6 starter)

- Server: 2 vCPU / 4 GiB per replica, 3 replicas, horizontal-scale on `commands/s` rate.
- Postgres: 4 vCPU / 16 GiB primary; read replica for `subscribe` snapshot queries.
- Artifact store: S3-compatible, lifecycle policy 90 days hot / archive after.
- Workers: sized per-kind; shell + file workers per-host, model + mcp workers in a sandboxed pool.

### Observability

- Prometheus metrics from server + workers.
- Tracing via OpenTelemetry (traceparent flows through subscription frames).
- Logs in JSON to stdout; collector aggregates.

### Backups

- Postgres: `pg_basebackup` nightly + WAL archive.
- Artifact store: S3 versioning + cross-region replication.
- Audit exports nightly via `actant-audit-export`.

## Shape C — ActantDB Cloud (Phase 6)

Managed service.

```
public internet
  ├── ActantDB Cloud (control plane)
  │     ├── workspace router
  │     ├── multi-tenant Postgres
  │     ├── artifact store
  │     └── studio
  └── customer environments
        ├── local agents
        ├── cloud workers (optional)
        └── phone approval app
```

Phase 6 ships the **control plane**. Data plane configurations:

- All-cloud: workers run in the cloud beside the server. Used by web-only customers.
- Hybrid: cloud server, on-prem workers. Common pattern; workers connect outbound via mTLS.
- BYO: server hosted in cloud; data stored in customer-owned S3 buckets and Postgres.

### Tenancy

- One workspace per customer team by default.
- One workspace per *person* in personal mode.
- Cross-workspace federation in Phase 6.5 for parent-org → child-team setups.

## Upgrades

- Forward-only migrations (`migrations/` numbered).
- Rolling deploy: old + new server replicas coexist; new schema columns must be additive (no `DROP COLUMN` in Phase 6+).
- Worker upgrades: `worker.version` mismatch produces a graceful drain; old workers stop claiming until upgraded.

## Disaster recovery

| Failure                       | Recovery                                                            |
| ----------------------------- | ------------------------------------------------------------------- |
| Server replica crash          | LB reroutes; other replicas continue. No state on replicas (all in Postgres). |
| Postgres primary failure      | Promote read replica; `pg_basebackup` restored from object store.    |
| Artifact-store outage         | Reads degrade gracefully (events still flow; large outputs unavailable). Server emits `artifact_unavailable` warnings. |
| Vault outage                  | Secrets cannot be materialized; effects requiring secrets stall in `awaiting_secret`. Studio surfaces the queue. |
| Full data loss                | Restore Postgres from base + WAL. Replay engine reconstructs projections from `agent_event` if projections lag. |

## Security checklist (Phase 6)

- [ ] OIDC issuer pinned, JWKs rotation policy documented.
- [ ] mTLS for worker → server. CA per workspace.
- [ ] Vault sealed at rest; unseal procedure documented.
- [ ] Database encrypted at rest.
- [ ] All HTTP forced over TLS 1.3 (1.2 for legacy mTLS clients only).
- [ ] Worker fleet runs with non-root users + read-only root FS.
- [ ] Egress allowlists per worker kind (model worker → providers only; shell worker → none by default).
- [ ] Audit exports verified by an out-of-band hash before customer delivery.

## Compliance starter pack (Phase 6)

For a SOC 2 evidence flow:

1. Continuous audit exports stored 7 years (per workspace policy).
2. Access logs from `actantdb-server`.
3. Vault access logs.
4. Migration history.
5. Worker-version inventory.
6. Eval-pass history.

Studio's Audit Explorer + Audit Exports satisfy the evidence retrieval need.
