#!/usr/bin/env node
// Generate an asciinema v2 cast file describing the killer-demo flow.
//
// Output: wedge/demo/killer-demo.cast (UTF-8 JSON-lines).
// Play with `asciinema play wedge/demo/killer-demo.cast`.
//
// The asciinema cast format is:
//   line 1: header object {version, width, height, timestamp, env, title}
//   line 2+: ["timestamp_seconds", "o" (output), "string"]
//
// We embed ANSI color escapes so the playback matches Studio's color scheme.

import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outPath = join(here, "..", "killer-demo.cast");

const RESET = "\x1b[0m";
const DIM = "\x1b[2m";
const BOLD = "\x1b[1m";
const CYAN = "\x1b[36m";
const YELLOW = "\x1b[33m";
const RED = "\x1b[31m";
const GREEN = "\x1b[32m";
const BLUE = "\x1b[34m";
const MAGENTA = "\x1b[35m";

const header = {
  version: 2,
  width: 120,
  height: 30,
  timestamp: Math.floor(Date.now() / 1000),
  env: { SHELL: "/bin/zsh", TERM: "xterm-256color" },
  title: "Actant — killer demo (test-cleanup agent)",
};

/** Each entry is [delay-in-seconds, text]. Time accumulates. */
const script = [
  [0.0, `${DIM}# actant-demo-test-cleanup — see wedge/killer-demo.md${RESET}\r\n`],
  [1.5, `${DIM}# 1. record one agent run through the wrapper${RESET}\r\n`],
  [2.0, `$ ${CYAN}node wedge/demo/demo.mjs${RESET}\r\n`],
  [2.0, `${DIM}[demo] using store: wedge/demo/.actantdb${RESET}\r\n`],
  [1.0, `${BLUE}[run]${RESET}      agent_run_started\r\n`],
  [0.7, `${BLUE}[run]${RESET}      user_message_received  "Clean up the test artifacts."\r\n`],
  [0.9, `${BLUE}[run]${RESET}      context_build  3 included, 0 blocked\r\n`],
  [0.4, `             ${DIM}↳ memory: "build artifacts live under /build and /dist"${RESET} ${YELLOW}⚠ stale${RESET}\r\n`],
  [1.2, `${MAGENTA}[planner]${RESET}  model_call  planner: shell.run "rm -rf build dist"\r\n`],
  [1.4, `${BLUE}[tool]${RESET}     tool_call_requested  shell.run {"command":"rm -rf build dist"}\r\n`],
  [1.6, `${YELLOW}[guard]${RESET}    guard_verdict  ${BOLD}require_approval${RESET} — constrain hint: drop dist\r\n`],
  [2.0, `${YELLOW}[approval]${RESET} approve_constrained → "rm -rf build"\r\n`],
  [1.0, `${BLUE}[tool]${RESET}     tool_call_started  shell.run {"command":"rm -rf build"}\r\n`],
  [1.4, `${GREEN}[ok]${RESET}       effect_completed  exit=0, "Removed 14 files."\r\n`],
  [0.8, `${BLUE}[run]${RESET}      agent_run_finished\r\n`],
  [1.0, `${GREEN}✅ Recorded killer-demo run for project=demo-test-cleanup${RESET}\r\n`],
  [0.6, `   Tool actually executed with: ${BOLD}{"command":"rm -rf build"}${RESET}\r\n\r\n`],
  [2.0, `${DIM}# 2. open Studio in another terminal${RESET}\r\n`],
  [1.5, `$ ${CYAN}ACTANTDB_STORE_DIR=./wedge/demo/.actantdb \\${RESET}\r\n`],
  [0.4, `  ${CYAN}npx actantdb studio --project demo-test-cleanup${RESET}\r\n`],
  [1.2, `${DIM}Actant Studio: http://127.0.0.1:4555${RESET}\r\n`],
  [0.4, `Studio listening on http://127.0.0.1:4555\r\n`],
  [0.4, `Press Ctrl-C to stop.\r\n\r\n`],
  [3.0, `${DIM}# 3. in the browser: click model_call → "Replay from here" → Run replay${RESET}\r\n`],
  [3.0, `${DIM}# Studio renders side-by-side diff (also in wedge/demo/hero.svg):${RESET}\r\n`],
  [1.0, `\r\n`],
  [0.4, `  ${DIM}event              original (recorded)            replay (under overrides)${RESET}\r\n`],
  [0.2, `  ${DIM}─────────────────────────────────────────────────────────────────────${RESET}\r\n`],
  [0.7, `  context_build      3 included, 0 blocked          ${YELLOW}2 included, 1 blocked${RESET}\r\n`],
  [0.7, `  model_call         ${RED}"rm -rf build dist"${RESET}            ${GREEN}"rm -rf build"${RESET}\r\n`],
  [0.7, `  guard_verdict      ${YELLOW}require_approval (constrain)${RESET}   ${GREEN}allow${RESET}\r\n`],
  [0.7, `  tool_call          shell.run "rm -rf build"       shell.run "rm -rf build"\r\n`],
  [0.7, `  effect_completed   exit=0                         exit=0  (recorded reuse)\r\n`],
  [1.5, `\r\n`],
  [0.9, `${BLUE}▌${RESET} Without ${BOLD}mem_42_dist${RESET}, the planner would have proposed the safe command directly.\r\n`],
  [1.4, `${BLUE}▌${RESET} The memory caused the risky proposal; Guard caught it; replay proves the causal link.\r\n`],
  [2.0, `\r\n`],
  [1.0, `$ \r\n`],
];

let t = 0;
const lines = [JSON.stringify(header)];
for (const [delta, text] of script) {
  t += delta;
  lines.push(JSON.stringify([Number(t.toFixed(3)), "o", text]));
}

writeFileSync(outPath, lines.join("\n") + "\n");
console.error(`Wrote ${outPath} (${lines.length - 1} events, total ${t.toFixed(2)}s)`);
