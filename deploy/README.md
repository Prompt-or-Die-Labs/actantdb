# ActantDB self-host with Docker Compose

The compose file in this directory spins up the full self-host stack in
one command. It is the recommended path if you do not want to install
the Rust toolchain.

## What you get

| Service           | Port(s)               | Why                                                     |
|-------------------|-----------------------|---------------------------------------------------------|
| `actantdb-server` | `4555`                | HTTP + WebSocket API. The thing your agents talk to.    |
| `postgres`        | (internal only)       | Durable ledger backend.                                 |
| `caddy`           | `80`, `443`           | Reverse proxy with auto-TLS when you set a domain.      |
| `mailpit`         | `1025` SMTP, `8025` UI| Local SMTP catcher so agents can "send mail" in dev.    |

## Quickstart (loopback, no TLS)

From the repo root:

```bash
docker compose -f deploy/docker-compose.yml up
```

Or build the server image locally (instead of pulling the published one):

```bash
docker compose -f deploy/docker-compose.yml up --build
```

Once everything is healthy:

- ActantDB API: <http://localhost:4555/v1/healthz>
- Mailpit UI:   <http://localhost:8025>
- Studio:       run `npx actantdb studio --server http://localhost:4555`
  from your project — Studio is an npm package, not a container service.

First real request:

```bash
curl -sS -X POST http://localhost:4555/v1/command \
    -H 'content-type: application/json' \
    -d '{
          "workspace_id":"default",
          "type":"create_session",
          "payload":{"actor_id":"shawn","title":"hello"}
        }'
```

You should get back a JSON body with `chain_hash` set. That row is now
in the ledger and visible to any client subscribed via WebSocket.

## Pointing it at a real hostname (auto-TLS)

Set `ACTANTDB_DOMAIN` before bringing the stack up:

```bash
export ACTANTDB_DOMAIN=actantdb.example.com
docker compose -f deploy/docker-compose.yml up -d
```

Caddy will provision a Let's Encrypt certificate the first time the
domain resolves to the host. Make sure ports 80 and 443 are reachable
from the public internet for the ACME HTTP-01 challenge to succeed.

## Validating the compose file

CI lints the compose file with:

```bash
docker compose -f deploy/docker-compose.yml config
```

This expands variables and prints the resolved structure without
actually starting any containers — safe for CI, no daemon required.

## Notes on the other recipes in this directory

- `deploy/docker/docker-compose.yaml` is the older local-cluster smoke
  recipe (just server + Postgres). It is still here for the workspace
  test suite. New consumers should prefer this directory's
  `docker-compose.yml`.
- `deploy/helm/actantdb/` is the Kubernetes chart for production
  clusters. The compose file is intentionally simpler than the chart —
  enough to demo the full feature surface locally, not enough to run a
  production cluster.

## Stopping and cleaning up

```bash
docker compose -f deploy/docker-compose.yml down            # stop
docker compose -f deploy/docker-compose.yml down -v         # stop + delete data
```

The named volumes (`actantdb-pg`, `caddy-data`, `caddy-config`,
`mailpit-data`) persist across `up`/`down` cycles. Add `-v` only when
you genuinely want to lose the ledger and the issued certs.
