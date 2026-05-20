import { withActant as baseWithActant } from "@actantdb/mastra";

export const withActant = baseWithActant;
export const withLangGraph = baseWithActant;

export { buildContextManifest, runGatedTool } from "@actantdb/mastra";

export type {
  ContextManifest,
  MastraAgentLike,
  MastraAgentLike as LangGraphAgentLike,
  MastraToolLike,
  MastraToolLike as LangGraphToolLike,
  WithActantOptions,
  WrappedAgent,
  WrappedAgent as WrappedLangGraph,
} from "@actantdb/mastra";
