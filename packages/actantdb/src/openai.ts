// Default export collides with @actantdb/anthropic's default export, so only
// the named exports and the default are reachable via this subpath.
export { OpenAI, default } from "@actantdb/openai";
export type { ActantClientOptions, OpenAIConstructorOptions } from "@actantdb/openai";
