# `@actantdb/box`

Local-first ActantDB Box. A sandboxed agent workspace with file, exec, git,
schedule, and snapshot primitives — every action captured in a hash-chained
ActantDB ledger.

The SDK surface is a 1:1 mirror of [Upstash Box](https://upstash.com/docs/box)
so porting is a one-line import change. The cloud control plane lands in a
future release (see [docs/CLOUD_ROADMAP.md](../../docs/CLOUD_ROADMAP.md)); for
now, `mode: "cloud"` resolves the contract but throws on every operation.

## Install

```bash
npm install @actantdb/box
```

No Rust toolchain, no Docker, no exposed ports.

## Quickstart

```ts
import { Box, Agent, ClaudeCode } from "@actantdb/box";

// Box.create with a preset coding-agent harness — drop-in for @upstash/box.
const box = await Box.create({
  name: "my-workspace",
  agent: {
    harness: Agent.ClaudeCode,        // or Agent.Codex / Agent.OpenCode / Agent.Cursor
    model: ClaudeCode.Sonnet_4_6,
    apiKey: process.env.ANTHROPIC_API_KEY,
  },
});

const run = await box.agent.run({ prompt: "Add a /healthz endpoint to server.ts" });
console.log(run.result);

// Or stream chunks:
for await (const chunk of box.agent.stream({ prompt: "Refactor auth.ts" })) {
  if (chunk.type === "text-delta") process.stdout.write(chunk.text);
}
```

The harness spawns the configured CLI (`claude`, `codex`, `opencode`, …) inside
the workspace. Install the CLI on PATH or set `CLAUDE_PATH` / `CODEX_PATH` /
`OPENCODE_PATH`. Every spawn lands as a typed event in the box's hash-chained
ledger so you can replay the run later.

## Custom agent (no preset)

If you'd rather plug in your own Mastra / LangGraph / hand-rolled agent:

```ts
const box = await Box.create({ name: "my-workspace" });

// Files
await box.files.write({ path: "hello.txt", content: "world" });
const back = await box.files.read("hello.txt"); // → "world"

// Exec
const run = await box.exec.command("ls -la");
console.log(run.result); // { exit: 0, output: "...", stderr: "" }

// Stream exec
for await (const chunk of box.exec.stream("npm test")) {
  if (chunk.type === "stdout") process.stdout.write(chunk.line + "\n");
}

// Git
await box.git.clone({ repo: "https://github.com/you/repo.git" });
await box.git.commit({ message: "feat: new thing", authorEmail: "you@example.com" });

// Schedules (no extra deps — internal setInterval scheduler)
await box.schedule.exec({ everyMs: 60_000, command: "git pull" });

// Snapshot the workspace
const snap = await box.snapshot({ name: "before-experiment" });

// ... later, restore into a brand-new box:
const replica = await Box.fromSnapshot(snap.id);

await box.delete();
```

## Migrating from `@upstash/box`

```diff
- import { Box } from "@upstash/box";
+ import { Box } from "@actantdb/box";

  const box = await Box.create({
    name: "demo",
-   apiKey: process.env.UPSTASH_BOX_KEY,   // ignored in local mode
  });
```

That's it. Every Upstash-Box method has the same shape in this package. The
difference is what it costs and where it runs:

| Concern               | `@upstash/box`           | `@actantdb/box` (local)   |
| --------------------- | ------------------------ | ------------------------- |
| Pricing               | $/CPU-hr                 | free                      |
| Network required      | yes                      | no                        |
| Audit log             | server-side trace        | hash-chained local ledger |
| Replay with overrides | n/a                      | `@actantdb/replay`        |
| Policy engine         | n/a                      | `@actantdb/policy`        |

When `mode: "cloud"` lands, the contract is already in place — the call site
above doesn't change.

## API reference

### `Box.create(config?)` / `Box.get(id)` / `Box.getByName(name)` / `Box.list()`

```ts
const box = await Box.create({
  name: "demo",
  mode: "local",        // default. "cloud" throws on every method.
  agent: myAgent,       // optional — needed for box.agent.run.
  storeRoot: "/tmp/x",  // override ~/.actantdb/boxes.
  cwd: "src",           // initial workspace-relative cwd.
  model: "claude-opus", // display-only.
  initCommand: "git clone ...", // optional one-shot at create.
  keepAlive: true,
});
```

`Box.list()` walks `~/.actantdb/boxes/*/box.json` and returns
`BoxData[]`. `Box.fromSnapshot(snapshotId, config)` creates a fresh Box whose
workspace is hydrated from a saved snapshot.

### `box.agent.run({ prompt, responseSchema?, timeout?, policy?, autoApprove? })`

Wraps the user-supplied agent with `@actantdb/mastra::withActant`. Records the
full timeline (`agent_run_started`, `user_message_received`, `model_call`,
`tool_call_*`, `guard_verdict`, `approval_*`, `agent_run_finished`) into the
box's ledger. Returns a `Run`.

`box.agent.stream(...)` yields `AgentChunk` values:

```ts
type AgentChunk =
  | { type: "text-delta"; text: string }
  | { type: "tool-call"; toolName: string; input: unknown }
  | { type: "tool-result"; toolName: string; result: unknown }
  | { type: "finish"; result: unknown };
```

If your agent exposes a `stream()` function, we pass through; otherwise we
synthesize a single `finish` chunk from `generate()`.

### `box.exec.command(cmd, opts?)` / `box.exec.stream(cmd, opts?)`

Spawns a subprocess inside the workspace via `node:child_process`. Buffers
output, persists `tool_call_completed` (with exit code + stdout + stderr) and
an `effect_observed{ kind: "exec_completed" }` event.

```ts
const run = await box.exec.command("npm run build", { timeoutMs: 60_000 });
if (run.status !== "ok") console.error(run.result);
```

Streaming yields line-buffered `ExecChunk` values:

```ts
for await (const c of box.exec.stream("yarn install")) {
  if (c.type === "stdout") process.stdout.write(c.line + "\n");
  if (c.type === "stderr") process.stderr.write(c.line + "\n");
  if (c.type === "exit") console.log("done", c.code);
}
```

### `box.files.write / read / list / upload / download`

```ts
await box.files.write({ path: "src/index.ts", content: "export {};" });
const text = await box.files.read("src/index.ts");
const entries = await box.files.list("src");
await box.files.upload([{ path: "/host/secret.env", destination: ".env" }]);
await box.files.download({ folder: "/tmp/exported-box" });
```

Every path is resolved relative to `box.cwd` and refused if it escapes the
workspace.

### `box.git.*`

```ts
await box.git.clone({ repo: "...", branch: "main" });
const diff = await box.git.diff();
const status = await box.git.status();          // { branch, ahead, behind, files, clean }
await box.git.updateConfig({ userName: "Alice", userEmail: "a@x" });
await box.git.commit({ message: "wip" });
await box.git.push({ branch: "main" });
const pr = await box.git.createPR({ title: "Wire up X", body: "..." });
if (!pr.submitted) console.log("gh missing, run by hand:", pr.command);
await box.git.checkout({ branch: "feat/y", create: true });
await box.git.exec({ args: ["log", "--oneline", "-n", "5"] });
```

### `box.schedule.*`

Zero-dep scheduler. Persists to `<workspace>/.actantdb/schedules.json` so
`Box.get(...)` resurrects timers on process restart.

```ts
const s = await box.schedule.exec({ everyMs: 30_000, command: "git pull" });
// or:
await box.schedule.agent({ cron: "*/5 * * * *", prompt: "review the queue" });

await box.schedule.pause(s.id);
await box.schedule.resume(s.id);
await box.schedule.delete(s.id);
```

Cron strings parse the common forms (`*/N * * * *`, `0 */N * * *`,
`0 0 */N * *`); anything else falls back to 60s. Prefer `everyMs` for
precision.

### `box.snapshot({ name? })` / `box.listSnapshots()` / `box.deleteSnapshot(id)` / `Box.fromSnapshot(id, config?)`

Snapshots are a deep copy of the workspace dir (the per-box ledger is
recreated on restore, intentionally). Local snapshots live under
`<storeRoot>/.snapshots/<id>/`.

```ts
const snap = await box.snapshot({ name: "before-experiment" });
const replica = await Box.fromSnapshot(snap.id, { name: "replica" });
```

### Lifecycle

```ts
await box.pause();   // stops schedules, marks status=paused.
await box.resume();
await box.delete();  // closes ledger, removes the box dir.
await box.cd("src"); // workspace-relative.
box.cwd;             // current workspace-relative path.
box.modelConfig;     // { harness: "local", model }.
await box.configureModel("claude-opus");
box.keepAlive = false;
```

### `Run`

Returned by exec / agent methods.

```ts
run.id;            // ledger run id
run.status;        // "pending" | "running" | "ok" | "error" | "cancelled"
run.result;        // tool output (exec) or model output (agent)
run.cost;          // { inputTokens: 0, outputTokens: 0, computeMs, totalUsd: 0 }
await run.cancel();
run.logs();        // ActantEvent[] for this run
```

> Local mode never infers token counts. `inputTokens`/`outputTokens` are
> always `0`; `computeMs` is measured. The cloud surface will populate the
> rest.

### Errors

Every error thrown from the public API is a `BoxError`:

```ts
import { BoxError } from "@actantdb/box";

try {
  await box.files.read("missing");
} catch (err) {
  if (err instanceof BoxError && err.code === "not_found") { /* ... */ }
}
```

Codes: `not_found`, `already_exists`, `io_error`, `exec_failed`,
`git_failed`, `schedule_not_found`, `snapshot_not_found`,
`invalid_argument`, `cloud_unsupported`, `deleted`.

## Cloud mode (Phase 2)

`Box.create({ mode: "cloud" })` resolves to a Box whose every operation
throws `cloud_unsupported`. The contract is here so consumer code is portable
the day the control plane lands. See
[docs/CLOUD_ROADMAP.md](../../docs/CLOUD_ROADMAP.md).

## License

Apache-2.0.
