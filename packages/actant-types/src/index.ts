/**
 * @actantdb/types — generated TS bindings of `crates/actant-contracts`.
 *
 * Regenerate with:
 *   cargo run -p actant-contracts --bin actant-contracts -- codegen-ts
 *
 * Hand-edits to `src/generated/*` are forbidden. New cross-package types
 * must be added to `crates/actant-contracts` first. See
 * `/CLAUDE.md`.
 */

export * from "./generated/index.js";

import schemasJson from "./generated/schemas.json" with { type: "json" };

/** JSON Schema set for every contract type (parsed). */
export const schemas: Record<string, unknown> = schemasJson as Record<string, unknown>;

/** Identifier types — kept as nominal string aliases. */
export type ProjectId = string;
export type RunId = string;
export type EventId = string;
export type ToolCallId = string;
export type PolicyRef = string;

/** Severity for ledger errors surfaced through the public API. */
export type ActantErrorKind =
  | "storage_error"
  | "invalid_input"
  | "permission_denied"
  | "approval_required"
  | "approval_denied"
  | "not_found"
  | "conflict"
  | "idempotent_replay"
  | "policy_halt"
  | "internal_error"
  | "not_implemented";

export interface ActantError {
  kind: ActantErrorKind;
  code?: ActantErrorKind;
  message: string;
  hint?: string;
  fix?: string | null;
  cause?: unknown;
}
