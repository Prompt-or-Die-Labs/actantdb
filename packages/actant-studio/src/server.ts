import { readFile } from "node:fs/promises";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { ApprovalStore, Ledger, openLedger } from "@actantdb/core";
import { diffReplayAgainstOriginal, runFromEvent, tighten } from "@actantdb/replay";
import { demoPolicy } from "@actantdb/policy";
import type {
  ActantEvent,
  ApprovalDecision,
  ReplayDiff,
  ReplayOverrides,
  ReplayRun,
} from "@actantdb/types";

const here = dirname(fileURLToPath(import.meta.url));
const UI_DIR = join(here, "ui");

export interface StudioServerOptions {
  ledger: Ledger;
  port: number;
  /** When true, suppress "listening on…" logs (tests). */
  silent?: boolean;
}

export interface StudioHandle {
  url: string;
  close(): Promise<void>;
}

export function startStudioServer(opts: StudioServerOptions): Promise<StudioHandle> {
  const { ledger, port, silent } = opts;
  const server = createServer((req, res) => {
    handle(req, res, ledger).catch((err) => {
      res.statusCode = 500;
      res.setHeader("content-type", "application/json");
      res.end(JSON.stringify({ error: (err as Error).message ?? String(err) }));
    });
  });

  return new Promise<StudioHandle>((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      const addr = server.address();
      const actualPort = typeof addr === "object" && addr ? addr.port : port;
      const url = `http://127.0.0.1:${actualPort}`;
      if (!silent) console.error(`Actant Studio: ${url}`);
      resolve({
        url,
        close: () =>
          new Promise<void>((res, rej) => server.close((e) => (e ? rej(e) : res()))),
      });
    });
  });
}

async function handle(
  req: IncomingMessage,
  res: ServerResponse,
  ledger: Ledger,
): Promise<void> {
  const url = new URL(req.url ?? "/", "http://localhost");
  const route = url.pathname;

  if (req.method === "GET" && (route === "/" || route === "/index.html")) {
    return staticFile(res, "index.html", "text/html");
  }
  if (req.method === "GET" && route === "/studio.css") {
    return staticFile(res, "studio.css", "text/css");
  }
  if (req.method === "GET" && route === "/studio.js") {
    return staticFile(res, "studio.js", "application/javascript");
  }
  if (req.method === "GET" && route === "/api/info") {
    return json(res, {
      project: ledger.project,
      dbPath: ledger.path(),
      runs: runsSummary(ledger),
    });
  }
  if (req.method === "GET" && route === "/api/events") {
    const runId = url.searchParams.get("run") ?? undefined;
    const events = ledger.query(runId ? { runId } : {});
    return json(res, { events });
  }
  if (req.method === "GET" && route === "/api/approvals") {
    const approvals = approvalsStore(ledger).all();
    return json(res, { approvals });
  }
  if (req.method === "POST" && route === "/api/approvals/decide") {
    const body = await readBody(req);
    const { toolCallId, decision } = JSON.parse(body) as {
      toolCallId: string;
      decision: ApprovalDecision;
    };
    const store = approvalsStore(ledger);
    const rec = store.get(toolCallId);
    if (!rec) {
      res.statusCode = 404;
      return json(res, { error: `approval not found: ${toolCallId}` });
    }
    const decided = store.decide(toolCallId, decision);
    ledger.append({
      kind: "approval_decision",
      runId: rec.runId,
      payload: { tool_call_id: toolCallId, ...decision },
      sensitivity: "low",
    });
    return json(res, { approval: decided });
  }
  if (req.method === "POST" && route === "/api/replay") {
    const body = await readBody(req);
    const { eventId, overrides, useStrictPolicy } = JSON.parse(body) as {
      eventId: string;
      overrides?: ReplayOverrides;
      useStrictPolicy?: boolean;
    };
    const policy = useStrictPolicy
      ? tighten(demoPolicy, {
          deny: [
            {
              tool: "shell.run",
              pattern: "\\bdist\\b",
              reason: "no shell.run without explicit dist guard",
            },
          ],
        })
      : undefined;
    const replay: ReplayRun = runFromEvent({
      ledger,
      eventId,
      ...(overrides !== undefined ? { overrides } : {}),
      ...(policy !== undefined ? { policy } : {}),
    });
    const dif: ReplayDiff = diffReplayAgainstOriginal(ledger, replay);
    return json(res, { replay, diff: dif });
  }

  res.statusCode = 404;
  return json(res, { error: "not found", route });
}

async function staticFile(res: ServerResponse, name: string, mime: string): Promise<void> {
  const body = await readFile(join(UI_DIR, name));
  res.setHeader("content-type", mime + "; charset=utf-8");
  res.statusCode = 200;
  res.end(body);
}

function json(res: ServerResponse, payload: unknown): void {
  if (!res.headersSent) {
    res.setHeader("content-type", "application/json");
    if (!res.statusCode || res.statusCode === 200) res.statusCode = 200;
  }
  res.end(JSON.stringify(payload));
}

async function readBody(req: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString("utf8");
}

function runsSummary(ledger: Ledger): Array<{ runId: string; events: number; startedAt: string }> {
  const all = ledger.query({});
  const byRun = new Map<string, ActantEvent[]>();
  for (const e of all) {
    const list = byRun.get(e.run_id) ?? [];
    list.push(e);
    byRun.set(e.run_id, list);
  }
  return Array.from(byRun.entries()).map(([runId, events]) => ({
    runId,
    events: events.length,
    startedAt: events[0]?.created_at ?? "",
  }));
}

function approvalsStore(ledger: Ledger): ApprovalStore {
  return new ApprovalStore(ledger);
}

export { openLedger };
