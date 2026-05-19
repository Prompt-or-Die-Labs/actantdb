#!/usr/bin/env node
/**
 * actantdb — local CLI for the Studio package.
 *
 * Subcommands (per /wedge/anti-scope.md §5):
 *   actantdb studio                            open local Studio
 *   actantdb approve <tool_call_id> [--scope=<>]
 *   actantdb deny    <tool_call_id> [--reason=<>]
 *   actantdb replay create --from-event <id>   write a checkpoint JSON to stdout
 *   actantdb replay run    --from-event <id> [--without-memory <id>] [--strict-policy]
 *   actantdb replay diff   <run_a> <run_b>
 *   actantdb approvals
 *
 * No other subcommands ship in v0.1.
 */

import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { platform } from "node:os";
import { openLedger, ApprovalStore } from "@actantdb/core";
import {
  diff,
  diffReplayAgainstOriginal,
  runFromEvent,
  tighten,
} from "@actantdb/replay";
import { demoPolicy } from "@actantdb/policy";
import { startStudioServer } from "./server.js";

interface ParsedArgs {
  positional: string[];
  flags: Record<string, string | boolean>;
}

function parseArgs(argv: string[]): ParsedArgs {
  const positional: string[] = [];
  const flags: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const eq = key.indexOf("=");
      if (eq >= 0) {
        flags[key.slice(0, eq)] = key.slice(eq + 1);
      } else if (i + 1 < argv.length && !argv[i + 1]!.startsWith("--")) {
        flags[key] = argv[i + 1]!;
        i++;
      } else {
        flags[key] = true;
      }
    } else {
      positional.push(a);
    }
  }
  return { positional, flags };
}

function projectFrom(flags: Record<string, string | boolean>): string {
  const env = process.env.ACTANTDB_PROJECT;
  const v = (flags.project as string | undefined) ?? env;
  if (!v) {
    console.error(
      "actantdb: a project name is required.\n" +
        "  Pass --project <name> or set ACTANTDB_PROJECT.\n" +
        "  Example: actantdb studio --project demo-test-cleanup",
    );
    process.exit(2);
  }
  return v;
}

function storeDirFrom(flags: Record<string, string | boolean>): string | undefined {
  return (flags["store-dir"] as string | undefined) ?? process.env.ACTANTDB_STORE_DIR;
}

function usage(): void {
  console.error(`actantdb — local CLI for ActantDB Studio

USAGE:
  actantdb <subcommand> [options]
  actantdb --version

SUBCOMMANDS:
  studio                   open Studio (HTTP UI on http://127.0.0.1:4555)
  approvals                list pending approvals
  approve <tool_call_id>   approve a pending tool call
                           [--scope <once|session>] [--constrained]
  deny    <tool_call_id>   deny a pending tool call [--reason <text>]
  replay create  --from-event <id>
                           print a checkpoint JSON
  replay run     --from-event <id>
                           [--without-memory <id>] [--strict-policy] [--alt-output <text>]
  replay diff    <run_a> <run_b>
                           diff two run id pairs

GLOBAL OPTIONS:
  --project <name>         project identifier (or ACTANTDB_PROJECT env)
  --store-dir <path>       storage root (default: ~/.actantdb; or ACTANTDB_STORE_DIR)
  --port <num>             studio HTTP port (default: 4555; pass 0 for ephemeral)
  --quiet                  suppress "Studio listening on…" banner
  --no-open                don't auto-open the browser (open by default in local mode)
  --open                   force-open even when ACTANTDB_NO_OPEN is set

EXAMPLES:
  # Open Studio against the in-repo demo store:
  ACTANTDB_STORE_DIR=./wedge/demo/.actantdb actantdb studio --project demo-test-cleanup

  # List pending approvals for a project:
  actantdb approvals --project my-agent

  # Approve a pending tool call, accepting the constrained variant:
  actantdb approve <tool_call_id> --constrained --scope once --project my-agent
`);
}

const require = createRequire(import.meta.url);
const pkg = require("../package.json") as { version: string };
const VERSION: string = pkg.version;

async function main(): Promise<void> {
  const [sub, ...rest] = process.argv.slice(2);
  if (!sub || sub === "-h" || sub === "--help" || sub === "help") {
    usage();
    process.exit(sub ? 0 : 2);
  }
  if (sub === "-V" || sub === "--version" || sub === "version") {
    console.log(`actantdb ${VERSION}`);
    process.exit(0);
  }
  const { positional, flags } = parseArgs(rest);

  switch (sub) {
    case "studio":
      await cmdStudio(flags);
      return;
    case "approvals":
      cmdApprovals(flags);
      return;
    case "approve":
      cmdApprove(positional, flags);
      return;
    case "deny":
      cmdDeny(positional, flags);
      return;
    case "replay":
      cmdReplay(positional, flags);
      return;
    default:
      console.error(`actantdb: unknown subcommand: ${sub}`);
      usage();
      process.exit(2);
  }
}

async function cmdStudio(flags: Record<string, string | boolean>): Promise<void> {
  const project = projectFrom(flags);
  const storeDir = storeDirFrom(flags);
  const port = Number(flags.port ?? 4555);
  const quiet = Boolean(flags.quiet);
  const ledger = openLedger(project, storeDir);
  const { url } = await startStudioServer({ ledger, port, silent: quiet });
  if (!quiet) {
    process.stdout.write(`Studio listening on ${url}\n`);
    process.stdout.write("Press Ctrl-C to stop.\n");
  }
  // Auto-open the browser when running locally. Opt out via --no-open or
  // ACTANTDB_NO_OPEN=1. Opt-in to override env via --open. Honored only
  // for loopback URLs (http://127.0.0.1, http://localhost) so a remote
  // deploy never tries to spawn a browser on the server host.
  const noOpenFlag = Boolean(flags["no-open"]);
  const openFlag = Boolean(flags.open);
  const envOptOut = process.env.ACTANTDB_NO_OPEN === "1";
  const isLoopback = /^https?:\/\/(127\.0\.0\.1|localhost)\b/.test(url);
  if (isLoopback && !noOpenFlag && (openFlag || !envOptOut)) {
    openBrowser(url);
  }
}

/**
 * Open `url` in the host OS's default browser. Detached + ignored so the
 * child doesn't hold the CLI's stdio open. Fire-and-forget; spawn errors
 * are swallowed because a missing `open`/`xdg-open` should never crash
 * Studio.
 */
function openBrowser(url: string): void {
  const opener =
    platform() === "darwin"
      ? { cmd: "open", args: [url] }
      : platform() === "win32"
        ? { cmd: "cmd", args: ["/c", "start", "", url] }
        : { cmd: "xdg-open", args: [url] };
  try {
    const child = spawn(opener.cmd, opener.args, {
      detached: true,
      stdio: "ignore",
    });
    child.on("error", () => {
      // Swallow — most likely cause is the opener binary not being on
      // PATH (headless containers, slim CI images). The URL is still in
      // the launch banner above; user can open it manually.
    });
    child.unref();
  } catch {
    // Same — silent fall-through.
  }
}

function cmdApprovals(flags: Record<string, string | boolean>): void {
  const project = projectFrom(flags);
  const storeDir = storeDirFrom(flags);
  const ledger = openLedger(project, storeDir);
  const store = new ApprovalStore(ledger);
  const pending = store.pending();
  if (pending.length === 0) {
    console.error("(no pending approvals)");
  } else {
    for (const p of pending) {
      const hint = p.request.hint ? ` hint=${JSON.stringify(p.request.hint)}` : "";
      console.log(
        `${p.toolCallId}  ${p.request.tool}  ${JSON.stringify(p.request.args)}${hint}`,
      );
    }
  }
  ledger.close();
}

function cmdApprove(positional: string[], flags: Record<string, string | boolean>): void {
  const toolCallId = positional[0];
  if (!toolCallId) {
    console.error("actantdb approve: <tool_call_id> is required");
    process.exit(2);
  }
  const project = projectFrom(flags);
  const storeDir = storeDirFrom(flags);
  const ledger = openLedger(project, storeDir);
  const store = new ApprovalStore(ledger);
  const rec = store.get(toolCallId);
  if (!rec) {
    console.error(`actantdb: approval not found: ${toolCallId}`);
    process.exit(2);
  }
  const scope = String(flags.scope ?? "once");
  const constrained = Boolean(flags.constrained);
  const decision =
    constrained && rec.request.constrained_input !== undefined
      ? {
          decision: "approve_constrained" as const,
          approver: process.env.USER ?? "cli",
          scope,
          accepted_input: rec.request.constrained_input,
        }
      : {
          decision: "approve" as const,
          approver: process.env.USER ?? "cli",
          scope,
        };
  store.decide(toolCallId, decision);
  ledger.append({
    kind: "approval_decision",
    runId: rec.runId,
    payload: { tool_call_id: toolCallId, ...decision },
    sensitivity: "low",
  });
  console.error(`approved ${toolCallId} (scope=${scope}, constrained=${constrained})`);
  ledger.close();
}

function cmdDeny(positional: string[], flags: Record<string, string | boolean>): void {
  const toolCallId = positional[0];
  if (!toolCallId) {
    console.error("actantdb deny: <tool_call_id> is required");
    process.exit(2);
  }
  const project = projectFrom(flags);
  const storeDir = storeDirFrom(flags);
  const ledger = openLedger(project, storeDir);
  const store = new ApprovalStore(ledger);
  const rec = store.get(toolCallId);
  if (!rec) {
    console.error(`actantdb: approval not found: ${toolCallId}`);
    process.exit(2);
  }
  const decision = {
    decision: "deny" as const,
    approver: process.env.USER ?? "cli",
    reason: String(flags.reason ?? "denied by operator"),
  };
  store.decide(toolCallId, decision);
  ledger.append({
    kind: "approval_decision",
    runId: rec.runId,
    payload: { tool_call_id: toolCallId, ...decision },
    sensitivity: "low",
  });
  console.error(`denied ${toolCallId}`);
  ledger.close();
}

function cmdReplay(positional: string[], flags: Record<string, string | boolean>): void {
  const sub = positional[0];
  if (sub === "create") {
    const project = projectFrom(flags);
    const storeDir = storeDirFrom(flags);
    const eventId = String(flags["from-event"] ?? "");
    if (!eventId) {
      console.error("actantdb replay create: --from-event is required");
      process.exit(2);
    }
    const ledger = openLedger(project, storeDir);
    const cp = ledger.checkpoint(eventId);
    console.log(JSON.stringify(cp, null, 2));
    ledger.close();
    return;
  }
  if (sub === "run") {
    const project = projectFrom(flags);
    const storeDir = storeDirFrom(flags);
    const eventId = String(flags["from-event"] ?? "");
    if (!eventId) {
      console.error("actantdb replay run: --from-event is required");
      process.exit(2);
    }
    const without = ([] as string[]).concat(flags["without-memory"] as string[] | string ?? []);
    const ledger = openLedger(project, storeDir);
    const policy = flags["strict-policy"]
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
    const alt = flags["alt-output"] ? String(flags["alt-output"]) : undefined;
    const replay = runFromEvent({
      ledger,
      eventId,
      overrides: { without_memory: typeof without === "string" ? [without] : without },
      ...(policy ? { policy } : {}),
      ...(alt !== undefined ? { alternatePlannerOutput: alt } : {}),
    });
    const dif = diffReplayAgainstOriginal(ledger, replay);
    console.log(JSON.stringify({ replay, diff: dif }, null, 2));
    ledger.close();
    return;
  }
  if (sub === "diff") {
    const project = projectFrom(flags);
    const storeDir = storeDirFrom(flags);
    const a = positional[1];
    const b = positional[2];
    if (!a || !b) {
      console.error("actantdb replay diff: <run_a> <run_b> required");
      process.exit(2);
    }
    const ledger = openLedger(project, storeDir);
    const ea = ledger.query({ runId: a });
    const eb = ledger.query({ runId: b });
    console.log(JSON.stringify(diff(ea, eb), null, 2));
    ledger.close();
    return;
  }
  console.error("actantdb replay: subcommand required (create | run | diff)");
  process.exit(2);
}

main().catch((err) => {
  console.error(err instanceof Error ? err.stack ?? err.message : String(err));
  process.exit(1);
});
