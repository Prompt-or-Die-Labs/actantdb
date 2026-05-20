#!/usr/bin/env node
import { mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(new URL("..", import.meta.url).pathname);
const packDir = mkdtempSync(join(tmpdir(), "actantdb-bun-packs-"));
const workDir = mkdtempSync(join(tmpdir(), "actantdb-bun-create-"));
const version = JSON.parse(readFileSync(join(root, "packages/actant-core/package.json"), "utf8"))
  .version;

const packages = [
  ["@actantdb/types", "actant-types", `actantdb-types-${version}.tgz`],
  ["@actantdb/core", "actant-core", `actantdb-core-${version}.tgz`],
  ["@actantdb/policy", "actant-policy", `actantdb-policy-${version}.tgz`],
  ["@actantdb/replay", "actant-replay", `actantdb-replay-${version}.tgz`],
  ["@actantdb/mastra", "actant-mastra", `actantdb-mastra-${version}.tgz`],
  ["@actantdb/studio", "actant-studio", `actantdb-studio-${version}.tgz`],
];

try {
  run("bun", ["--version"], root);
  for (const [, dir] of packages) {
    run("pnpm", ["-C", join(root, "packages", dir), "pack", "--pack-destination", packDir], root);
  }

  run(
    "node",
    [
      join(root, "packages/create-actantdb/dist/index.js"),
      "bun-first-run",
      "--template",
      "minimal",
      "--framework",
      "hand-rolled",
      "--language",
      "js",
      "--runtime",
      "bun",
      "--yes",
    ],
    workDir,
  );

  const appDir = join(workDir, "bun-first-run");
  const pkgPath = join(appDir, "package.json");
  const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
  pkg.overrides = pkg.overrides ?? {};
  for (const [name, , tarball] of packages) {
    const spec = `file:${join(packDir, tarball)}`;
    pkg.overrides[name] = spec;
    if (pkg.dependencies?.[name]) pkg.dependencies[name] = spec;
    if (pkg.devDependencies?.[name]) pkg.devDependencies[name] = spec;
  }
  writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

  run("bun", ["install"], appDir);
  run("bun", ["start"], appDir);

  const ledger = join(appDir, ".actantdb", "bun-first-run", "events.sqlite");
  if (!existsSync(ledger)) {
    throw new Error(`expected embedded ledger at ${ledger}`);
  }
  console.log(`[bun-create] ok: ${ledger}`);
} finally {
  rmSync(packDir, { recursive: true, force: true });
  rmSync(workDir, { recursive: true, force: true });
}

function run(cmd, args, cwd) {
  const res = spawnSync(cmd, args, {
    cwd,
    stdio: "inherit",
    env: process.env,
  });
  if (res.error) throw res.error;
  if (res.status !== 0) {
    throw new Error(`${cmd} ${args.join(" ")} failed with exit ${res.status}`);
  }
}
