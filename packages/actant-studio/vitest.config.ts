import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Vitest config — separate from vite.config.ts (which has root: "ui-src"
// for the production build). Tests live in both src/ (server) and
// ui-src/ (React UI), so vitest needs the package root.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}", "ui-src/**/*.test.{ts,tsx}"],
    setupFiles: ["ui-src/test-setup.ts"],
    globals: true,
    pool: "forks",
    // jsdom is the default for UI tests; server tests work fine there too
    // (they use node:http + node:fs, which jsdom doesn't shadow).
    environmentMatchGlobs: [
      ["src/**/*.test.ts", "node"],
      ["ui-src/**/*.test.{ts,tsx}", "jsdom"],
    ],
  },
});
