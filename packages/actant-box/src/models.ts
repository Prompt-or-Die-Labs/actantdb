/**
 * Preset model identifiers — string enums so consumer code reads naturally
 * (`ClaudeCode.Sonnet_4_6`) and gets autocomplete.
 *
 * Values are the canonical `<provider>/<model>` strings the CLI harnesses
 * accept directly. Pass any string here too — these are convenience names,
 * not a closed set.
 */

/** Claude Code (Anthropic). */
export const ClaudeCode = {
  Opus_4_7: "anthropic/claude-opus-4-7",
  Opus_4_6: "anthropic/claude-opus-4-6",
  Opus_4_5: "anthropic/claude-opus-4-5",
  Sonnet_4_6: "anthropic/claude-sonnet-4-6",
  Sonnet_4_5: "anthropic/claude-sonnet-4-5",
  Sonnet_4: "anthropic/claude-sonnet-4",
  Haiku_4_5: "anthropic/claude-haiku-4-5",
} as const;
export type ClaudeCodeModel = (typeof ClaudeCode)[keyof typeof ClaudeCode];

/** OpenAI Codex (Codex CLI). */
export const OpenAICodex = {
  GPT_5_4: "openai/gpt-5.4",
  GPT_5_4_Mini: "openai/gpt-5.4-mini",
  GPT_5_3_Codex: "openai/gpt-5.3-codex",
  GPT_5_3_Codex_Spark: "openai/gpt-5.3-codex-spark",
  GPT_5_2_Codex: "openai/gpt-5.2-codex",
  GPT_5_1_Codex_Max: "openai/gpt-5.1-codex-max",
} as const;
export type OpenAICodexModel = (typeof OpenAICodex)[keyof typeof OpenAICodex];

/** OpenRouter — single entry point for many providers. */
export const OpenRouterModel = {
  Claude_Opus_4_5: "openrouter/anthropic/claude-opus-4-5",
  Claude_Sonnet_4: "openrouter/anthropic/claude-sonnet-4",
  Claude_Haiku_4_5: "openrouter/anthropic/claude-haiku-4-5",
  DeepSeek_R1: "openrouter/deepseek/deepseek-r1",
  Gemini_2_5_Pro: "openrouter/google/gemini-2.5-pro",
} as const;
export type OpenRouterModelId = (typeof OpenRouterModel)[keyof typeof OpenRouterModel];

/** Cursor's own model name. Used when `harness === Agent.Cursor`. */
export const CursorModel = {
  Auto: "cursor/auto",
  Sonnet_4_6: "cursor/sonnet-4-6",
} as const;
export type CursorModelId = (typeof CursorModel)[keyof typeof CursorModel];

/** OpenCode model identifiers. The CLI accepts the same `<provider>/<model>` shape. */
export const OpenCodeModel = {
  Claude_Sonnet_4_6: "anthropic/claude-sonnet-4-6",
  Claude_Opus_4_7: "anthropic/claude-opus-4-7",
  GPT_5_4: "openai/gpt-5.4",
} as const;
export type OpenCodeModelId = (typeof OpenCodeModel)[keyof typeof OpenCodeModel];
