# 11 — Run inside a Supabase Edge Function

Use `@actantdb/supabase` when a Supabase app wants ActantDB trace capture inside an Edge Function without standing up a separate ActantDB server.

## What it does

- Records the function invocation as ActantDB `agent_event` rows in Supabase Postgres.
- Computes `payload_hash` and `event_hash` in the Edge runtime.
- Leaves model calls and business logic in your code.

## Setup

Apply the Postgres migrations in `migrations/pg/` to the Supabase database, then install the adapter in the function project:

```bash
npm install @actantdb/supabase @supabase/supabase-js
```

## Function

```ts
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";
import { withActantSupabaseEdge } from "npm:@actantdb/supabase";

const supabase = createClient(
  Deno.env.get("SUPABASE_URL")!,
  Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!,
);

Deno.serve(
  withActantSupabaseEdge(
    async (_request, { run }) => {
      await run.recordUserMessage("Summarize today's support backlog.");
      await run.recordModelCall({
        model: "ollama:llama3.2:8b",
        role: "generator",
        prompt_hash: "support-backlog",
        summary: "local model produced a support summary",
      });

      return Response.json({ ok: true });
    },
    {
      supabase,
      project: "support-agent",
      workspaceId: "ws_default",
      actorId: "act_support_edge",
    },
  ),
);
```

## Boundary

This is an Edge ledger adapter, not embedded SQLite. Supabase Edge Functions do not provide `node:sqlite`, so the package writes to the ActantDB Postgres schema already hosted by Supabase. Use `deploy/docker-compose.yml` when you want the full ActantDB server, Studio, WebSocket subscriptions, approvals, and command engine.
