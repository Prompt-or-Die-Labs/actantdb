import { describe, expect, it } from "vitest";
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { parseArgs, scaffold } from "./index.js";
import { getTemplate, TEMPLATES } from "./templates.js";
import { renderTemplate } from "./render.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "create-actantdb-"));
}

describe("argv parsing", () => {
  it("parses positional + long flags", () => {
    const args = parseArgs([
      "my-app",
      "--template",
      "coding-agent",
      "--framework",
      "mastra",
      "--language",
      "ts",
      "--port",
      "5173",
      "--no-interactive",
    ]);
    expect(args.positional).toEqual(["my-app"]);
    expect(args.template).toBe("coding-agent");
    expect(args.framework).toBe("mastra");
    expect(args.language).toBe("ts");
    expect(args.studioPort).toBe(5173);
    expect(args.interactive).toBe(false);
  });

  it("supports --flag=value syntax", () => {
    const args = parseArgs(["app", "--template=minimal", "--language=js"]);
    expect(args.template).toBe("minimal");
    expect(args.language).toBe("js");
  });

  it("--yes implies non-interactive", () => {
    const args = parseArgs(["app", "--yes"]);
    expect(args.yes).toBe(true);
    expect(args.interactive).toBe(false);
  });
});

describe("template registry", () => {
  it("includes the five required templates", () => {
    const ids = TEMPLATES.map((t) => t.id);
    for (const id of ["minimal", "coding-agent", "research-agent", "support-agent", "fanout-agent"]) {
      expect(ids).toContain(id);
    }
  });

  it("getTemplate returns undefined for unknown ids", () => {
    expect(getTemplate("nonsense")).toBeUndefined();
    expect(getTemplate("minimal")).toBeDefined();
  });
});

describe("renderTemplate", () => {
  it("renders minimal template files with project-name substitution", () => {
    const files = renderTemplate({
      projectName: "test-scaffold",
      template: "minimal",
      framework: "hand-rolled",
      language: "js",
      studioPort: 4173,
      actantdbVersion: "^0.0.13",
    });
    const byPath = Object.fromEntries(files.map((f) => [f.path, f.content]));
    expect(byPath["package.json"]).toContain('"name": "test-scaffold"');
    expect(byPath["package.json"]).toContain('"@actantdb/core": "^0.0.13"');
    expect(byPath["agent.mjs"]).toBeDefined();
    expect(byPath["README.md"]).toContain("test-scaffold");
  });

  it("renders ts variant when language=ts", () => {
    const files = renderTemplate({
      projectName: "ts-app",
      template: "coding-agent",
      framework: "mastra",
      language: "ts",
      studioPort: 4173,
      actantdbVersion: "^0.0.13",
    });
    const paths = files.map((f) => f.path);
    expect(paths).toContain("tsconfig.json");
    expect(paths).toContain("src/agent.ts");
  });
});

describe("scaffold", () => {
  it("writes a runnable minimal project layout to disk", () => {
    const dir = join(freshDir(), "my-app");
    try {
      const r = scaffold(
        dir,
        {
          projectName: "my-app",
          template: "minimal",
          framework: "hand-rolled",
          language: "js",
          studioPort: 4173,
        },
        { force: true, version: "^0.0.13" },
      );
      expect(r.filesWritten).toContain("package.json");
      expect(r.filesWritten).toContain("agent.mjs");
      expect(existsSync(join(dir, "package.json"))).toBe(true);
      const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
      expect(pkg.name).toBe("my-app");
      expect(pkg.dependencies["@actantdb/core"]).toBe("^0.0.13");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("refuses to scaffold into a non-empty dir without --force", () => {
    const dir = join(freshDir(), "filled");
    scaffold(
      dir,
      {
        projectName: "filled",
        template: "minimal",
        framework: "hand-rolled",
        language: "js",
        studioPort: 4173,
      },
      { force: true, version: "^0.0.13" },
    );
    expect(() =>
      scaffold(
        dir,
        {
          projectName: "filled",
          template: "minimal",
          framework: "hand-rolled",
          language: "js",
          studioPort: 4173,
        },
        { force: false, version: "^0.0.13" },
      ),
    ).toThrow(/not empty/);
    rmSync(dir, { recursive: true, force: true });
  });
});
