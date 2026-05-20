#!/usr/bin/env bun
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

const root = resolve(new URL("..", import.meta.url).pathname);
const rootPackage = readPackage(join(root, "package.json"));
const scriptName = process.argv[2];

if (!scriptName) {
  fail("usage: bun scripts/bun-workspace.mjs <script> [--parallel]");
}

const parallel = process.argv.includes("--parallel");
const packages = discoverPackages();
const runnable = topologicalOrder(packages).filter((pkg) => pkg.scripts[scriptName]);

if (parallel) {
  await runParallel(runnable);
} else {
  for (const pkg of runnable) runPackageScript(pkg);
}

function discoverPackages() {
  const workspacePatterns = rootPackage.workspaces;
  if (!Array.isArray(workspacePatterns)) fail("root package.json workspaces must be an array");

  const packageDirs = new Set();
  for (const pattern of workspacePatterns) {
    if (pattern.endsWith("/*")) {
      const parent = join(root, pattern.slice(0, -2));
      if (!existsSync(parent)) continue;
      for (const entry of readdirSync(parent, { withFileTypes: true })) {
        if (!entry.isDirectory()) continue;
        const packagePath = join(parent, entry.name, "package.json");
        if (existsSync(packagePath)) packageDirs.add(dirname(packagePath));
      }
    } else {
      const packagePath = join(root, pattern, "package.json");
      if (existsSync(packagePath)) packageDirs.add(dirname(packagePath));
    }
  }

  const packages = [];
  for (const dir of [...packageDirs].sort()) {
    const pkg = readPackage(join(dir, "package.json"));
    packages.push({
      name: pkg.name,
      dir,
      scripts: pkg.scripts ?? {},
      dependencies: {
        ...pkg.dependencies,
        ...pkg.devDependencies,
        ...pkg.peerDependencies,
        ...pkg.optionalDependencies,
      },
    });
  }
  return packages;
}

function topologicalOrder(packages) {
  const byName = new Map();
  for (const pkg of packages) {
    if (typeof pkg.name !== "string") fail(`${relative(root, pkg.dir)} is missing package.json#name`);
    byName.set(pkg.name, pkg);
  }

  const visiting = new Set();
  const visited = new Set();
  const ordered = [];

  for (const pkg of packages) visit(pkg);
  return ordered;

  function visit(pkg) {
    if (visited.has(pkg.name)) return;
    if (visiting.has(pkg.name)) fail(`workspace dependency cycle at ${pkg.name}`);
    visiting.add(pkg.name);
    for (const name of Object.keys(pkg.dependencies).sort()) {
      const dep = byName.get(name);
      if (dep) visit(dep);
    }
    visiting.delete(pkg.name);
    visited.add(pkg.name);
    ordered.push(pkg);
  }
}

async function runParallel(packages) {
  const statuses = await Promise.all(packages.map((pkg) => runPackageScriptParallel(pkg)));
  if (statuses.some((status) => status !== 0)) process.exit(1);
}

function runPackageScript(pkg, options = {}) {
  const name = pkg.name;
  const prefix = `${name}:${scriptName}`;
  console.log(`${prefix} | ${relative(root, pkg.dir)}`);
  const result = spawnSync("bun", ["run", scriptName], {
    cwd: pkg.dir,
    env: process.env,
    stdio: options.inherit === false ? "pipe" : "inherit",
    encoding: options.inherit === false ? "utf8" : undefined,
  });
  if (result.error) throw result.error;
  if (result.status !== 0 && options.inherit !== false) process.exit(result.status ?? 1);
  return result;
}

function runPackageScriptParallel(pkg) {
  const prefix = `${pkg.name}:${scriptName}`;
  console.log(`${prefix} | ${relative(root, pkg.dir)}`);
  const child = spawn("bun", ["run", scriptName], {
    cwd: pkg.dir,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });

  child.stdout.on("data", (chunk) => writePrefixed(process.stdout, prefix, chunk));
  child.stderr.on("data", (chunk) => writePrefixed(process.stderr, prefix, chunk));

  return new Promise((resolveStatus) => {
    child.on("error", (error) => {
      console.error(`${prefix} | ${error.message}`);
      resolveStatus(1);
    });
    child.on("close", (status) => resolveStatus(status ?? 1));
  });
}

function writePrefixed(stream, prefix, chunk) {
  for (const line of chunk.toString().split(/\r?\n/)) {
    if (line.length > 0) stream.write(`${prefix} | ${line}\n`);
  }
}

function readPackage(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function fail(message) {
  console.error(`[bun-workspace] ${message}`);
  process.exit(1);
}
