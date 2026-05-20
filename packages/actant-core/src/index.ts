/**
 * @actantdb/core — embedded TypeScript backend for ActantDB.
 *
 * Local ledger + approval + replay state backed by node:sqlite or bun:sqlite.
 */

import { createRequire } from "node:module";

export { Ledger, openLedger, ledgerExists } from "./ledger.js";
export type { LedgerOptions, LedgerFilter, AppendInput, LedgerListener } from "./ledger.js";

export { ApprovalStore } from "./approvals.js";
export type { ApprovalRecord } from "./approvals.js";

export { createActant, buildContextManifest } from "./runtime.js";
export type { ActantOptions, ActantHandle, RunContext } from "./runtime.js";

export { ulid } from "./ulid.js";
export { canonicalJSON, sha256, sha256OfJSON, nextChainHash } from "./hash.js";

const require = createRequire(import.meta.url);
const pkg = require("../package.json") as { version: string };
export const VERSION: string = pkg.version;
