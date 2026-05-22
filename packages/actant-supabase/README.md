# @actantdb/supabase

Supabase Edge Function adapter for ActantDB. It records Edge invocations into the ActantDB Postgres `agent_event` ledger from inside the function, without requiring a separate `actantdb-server` process.

## Boundary

Supabase Edge Functions run on Deno. They cannot use `node:sqlite`, so this package does not import `@actantdb/core` and does not run embedded SQLite. It writes typed, hash-chained `agent_event` rows to the Supabase Postgres database that already has the ActantDB migrations applied.

## Install

```bash
npm install @actantdb/supabase @supabase/supabase-js
```

Apply the ActantDB Postgres migrations from `migrations/pg/` to the Supabase project first. The adapter uses `workspace`, `actor`, and `agent_event`.

## Edge Function

```ts
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";
import { withActantSupabaseEdge } from "npm:@actantdb/supabase";

const supabase = createClient(
  Deno.env.get("SUPABASE_URL")!,
  Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!,
);

const handler = withActantSupabaseEdge(
  async (request, { run }) => {
    await run.recordUserMessage("edge request received");
    await run.recordModelCall({
      model: "ollama:llama3.2:8b",
      role: "generator",
      prompt_hash: "local",
      summary: "local model handled the request",
    });

    return Response.json({ ok: true });
  },
  {
    supabase,
    project: "edge-agent",
    workspaceId: "ws_default",
    actorId: "act_edge_agent",
  },
);

Deno.serve(handler);
```

By default, the wrapper upserts the `workspace` and `actor` rows needed by the foreign keys, then appends `agent_run_started`, any events you record through `run`, and `agent_run_finished`.

## Operational notes

- Use a Supabase service role key or an RLS policy that can write the ActantDB tables.
- Keep one run per request. If multiple concurrent functions write the same `session_id`, Postgres can accept both rows but the hash chain can fork. Use distinct sessions or the full ActantDB server when strict per-session serialization is required.
- This adapter does not expose the command engine, approvals queue, Studio, or WebSocket API. Use the Docker Compose self-host path in `deploy/` when you need those server surfaces.
