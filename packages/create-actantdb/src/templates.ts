/**
 * Template registry mirrored from the Rust `actant-templates` crate.
 *
 * Keep this list in sync with `/templates/STATUS.md`. The scaffolder ships
 * an inline minimal template so `npm create actantdb` works without
 * additional downloads; richer templates are fetched from the monorepo or
 * (post-publish) from the published `@actantdb/templates` package.
 */

export type FrameworkChoice =
  | "mastra"
  | "langgraph"
  | "vercel-ai"
  | "openai-agents"
  | "hand-rolled";

export type LanguageChoice = "ts" | "js";

export interface Template {
  /** Identifier used as the --template flag value. */
  id: string;
  /** Human-readable title shown in the picker. */
  title: string;
  /** Short description. */
  description: string;
  /** Phase in which this template ships (matches /templates/STATUS.md). */
  phase: number;
  /** Default framework for this template. */
  defaultFramework: FrameworkChoice;
}

export const TEMPLATES: Template[] = [
  {
    id: "minimal",
    title: "Minimal",
    description: "Embedded ledger + withActant() wrapper, no real agent. Smallest install.",
    phase: 1,
    defaultFramework: "hand-rolled",
  },
  {
    id: "coding-agent",
    title: "Coding agent",
    description: "Mastra coding agent with replay-able tool calls and approval gates.",
    phase: 1,
    defaultFramework: "mastra",
  },
  {
    id: "research-agent",
    title: "Research agent",
    description: "Multi-step research with durable workflows, retries, and approvals.",
    phase: 3,
    defaultFramework: "mastra",
  },
  {
    id: "support-agent",
    title: "Support agent",
    description:
      "Customer-support agent with reviewable memory candidates + replay-on-complaint.",
    phase: 3,
    defaultFramework: "mastra",
  },
  {
    id: "fanout-agent",
    title: "Fan-out agent",
    description: "Spawn parallel sub-agents, aggregate, gate with Guard verdicts.",
    phase: 2,
    defaultFramework: "mastra",
  },
];

export const FRAMEWORKS: { id: FrameworkChoice; title: string }[] = [
  { id: "mastra", title: "Mastra" },
  { id: "langgraph", title: "LangGraph" },
  { id: "vercel-ai", title: "Vercel AI SDK" },
  { id: "openai-agents", title: "OpenAI Agents SDK" },
  { id: "hand-rolled", title: "Hand-rolled (no framework)" },
];

export function getTemplate(id: string): Template | undefined {
  return TEMPLATES.find((t) => t.id === id);
}
