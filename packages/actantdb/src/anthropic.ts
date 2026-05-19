// Default export collides with @actantdb/openai's default export, so only
// the named exports and the default are reachable via this subpath.
export { Anthropic, default } from "@actantdb/anthropic";
export type { ActantClientOptions, AnthropicConstructorOptions } from "@actantdb/anthropic";
