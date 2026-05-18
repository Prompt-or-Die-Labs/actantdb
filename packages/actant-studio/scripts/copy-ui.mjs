// Copies static UI assets into dist/ui so the published package can serve them.
import { cp, mkdir, stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const src = join(here, "..", "ui");
const dest = join(here, "..", "dist", "ui");

try {
  await stat(src);
} catch {
  console.error("[copy-ui] no ui/ source directory, skipping");
  process.exit(0);
}

await mkdir(dest, { recursive: true });
await cp(src, dest, { recursive: true });
console.error(`[copy-ui] copied ${src} -> ${dest}`);
