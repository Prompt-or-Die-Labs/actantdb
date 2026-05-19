import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite builds the React Studio UI. The Node Studio HTTP server
// (src/server.ts) only serves three exact paths — `/`, `/studio.css`,
// `/studio.js` — so we override Rollup's hashed filename convention to
// keep the existing routes working without touching the server.
//
// Output: dist/ui/{index.html, studio.js, studio.css}
export default defineConfig({
  root: "ui-src",
  plugins: [react()],
  esbuild: {
    jsx: "automatic",
  },
  build: {
    outDir: "../dist/ui",
    emptyOutDir: true,
    // Sourcemap off to keep the published npm tarball small. Studio is a
    // local dev tool; bundle debugging happens via `pnpm dev` (which
    // emits per-file maps automatically). Flip to true if a consumer
    // needs minified-bundle debugging in production.
    sourcemap: false,
    rollupOptions: {
      output: {
        entryFileNames: "studio.js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: (info) => {
          const n = info.name ?? "";
          if (n.endsWith(".css")) return "studio.css";
          return "assets/[name][extname]";
        },
      },
    },
  },
});
