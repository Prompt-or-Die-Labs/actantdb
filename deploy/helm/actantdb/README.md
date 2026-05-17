# ActantDB Helm chart

Phase-6 cloud/team chart for `actantdb-server`. Supports two storage modes:

- `storage.backend=sqlite` — single replica, PVC-backed.
- `storage.backend=postgres` — multi-replica capable, can provision its own
  Postgres StatefulSet or use an `externalUrl`.

## Install

```bash
helm install actant ./deploy/helm/actantdb \
  --set image.repository=ghcr.io/actantdb/actantdb-server \
  --set image.tag=0.0.1 \
  --set storage.backend=postgres \
  --set storage.postgres.auth.password=$(openssl rand -base64 24)
```

Or with an externally-managed Postgres:

```bash
helm install actant ./deploy/helm/actantdb \
  --set storage.postgres.externalUrl=postgres://user:pass@host:5432/actantdb
```

## Endpoints

The chart's Service exposes port `4555`. Liveness/readiness probe is
`GET /v1/healthz`.

## Status

This chart ships in the same session as the underlying Postgres backend
work. The image referenced (`ghcr.io/actantdb/actantdb-server`) is not yet
published — see `RELEASE_CHECKLIST.md` for the build/push steps.
