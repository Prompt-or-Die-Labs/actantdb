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
  const request = studioRequest(req);
  if (await handleStaticRoute(request, res)) return;
  if (handleGetRoute(request, res, ledger)) return;
  if (await handlePostRoute(request, req, res, ledger)) return;

  res.statusCode = 404;
  return json(res, { error: "not found", route: request.route });
}

interface StudioRequest {
  url: URL;
  route: string;
  method: string | undefined;
  staticMethod: string | undefined;
  headOnly: boolean;
}

function studioRequest(req: IncomingMessage): StudioRequest {
  const url = new URL(req.url ?? "/", "http://localhost");
  const headOnly = req.method === "HEAD";
  return {
    url,
    route: url.pathname,
    method: req.method,
    staticMethod: headOnly ? "GET" : req.method,
    headOnly,
  };
}

async function handleStaticRoute(
  request: StudioRequest,
  res: ServerResponse,
): Promise<boolean> {
  const staticRoute = staticRouteFor(request.route);
  if (request.staticMethod !== "GET" || !staticRoute) return false;
  await staticFile(res, staticRoute.name, staticRoute.mime, request.headOnly);
  return true;
}

function staticRouteFor(route: string): { name: string; mime: string } | undefined {
  if (route === "/" || route === "/index.html") {
    return { name: "index.html", mime: "text/html" };
  }
  if (route === "/studio.css") return { name: "studio.css", mime: "text/css" };
  if (route === "/studio.js") return { name: "studio.js", mime: "application/javascript" };
  return undefined;
}

function handleGetRoute(
  request: StudioRequest,
  res: ServerResponse,
  ledger: Ledger,
): boolean {
  if (request.method !== "GET") return false;
  if (request.route === "/api/info") {
    json(res, {
      project: ledger.project,
      dbPath: ledger.path(),
      runs: runsSummary(ledger),
    });
    return true;
  }
  if (request.route === "/api/events") {
    const runId = request.url.searchParams.get("run") ?? undefined;
    const events = ledger.query(runId ? { runId } : {});
    json(res, { events });
    return true;
  }
  if (request.route === "/api/approvals") {
    json(res, { approvals: approvalsStore(ledger).all() });
    return true;
  }
  return false;
}

async function handlePostRoute(
  request: StudioRequest,
  req: IncomingMessage,
  res: ServerResponse,
  ledger: Ledger,
): Promise<boolean> {
  if (request.method !== "POST") return false;
  if (request.route === "/api/approvals/decide") {
    await decideApproval(req, res, ledger);
    return true;
  }
  if (request.route === "/api/replay") {
    await replayFromRequest(req, res, ledger);
    return true;
  }
  return false;
}

async function decideApproval(
  req: IncomingMessage,
  res: ServerResponse,
  ledger: Ledger,
): Promise<void> {
  const { toolCallId, decision } = JSON.parse(await readBody(req)) as {
    toolCallId: string;
    decision: ApprovalDecision;
  };
  const store = approvalsStore(ledger);
  const rec = store.get(toolCallId);
  if (!rec) {
    res.statusCode = 404;
    json(res, { error: `approval not found: ${toolCallId}` });
    return;
  }
  const decided = store.decide(toolCallId, decision);
  ledger.append({
    kind: "approval_decision",
    runId: rec.runId,
    payload: { tool_call_id: toolCallId, ...decision },
    sensitivity: "low",
  });
  json(res, { approval: decided });
}

async function replayFromRequest(
  req: IncomingMessage,
  res: ServerResponse,
  ledger: Ledger,
): Promise<void> {
  const { eventId, overrides, useStrictPolicy } = JSON.parse(await readBody(req)) as {
    eventId: string;
    overrides?: ReplayOverrides;
    useStrictPolicy?: boolean;
  };
  const replay: ReplayRun = runFromEvent({
    ledger,
    eventId,
    ...(overrides !== undefined ? { overrides } : {}),
    ...(useStrictPolicy ? { policy: strictReplayPolicy() } : {}),
  });
  const dif: ReplayDiff = diffReplayAgainstOriginal(ledger, replay);
  json(res, { replay, diff: dif });
}

function strictReplayPolicy() {
  return tighten(demoPolicy, {
    deny: [
      {
        tool: "shell.run",
        pattern: "\\bdist\\b",
        reason: "no shell.run without explicit dist guard",
      },
    ],
  });
}

async function staticFile(
  res: ServerResponse,
  name: string,
  mime: string,
  headOnly = false,
): Promise<void> {
  const body = await readFile(join(UI_DIR, name));
  res.setHeader("content-type", mime + "; charset=utf-8");
  res.setHeader("content-length", String(body.byteLength));
  res.statusCode = 200;
  if (headOnly) res.end();
  else res.end(body);
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
