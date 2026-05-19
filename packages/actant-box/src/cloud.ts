/**
 * @actantdb/box — cloud mode stub.
 *
 * `Box.create({ mode: "cloud" })` resolves to a Box whose every method
 * throws `cloud_unsupported`. The contract is in place so consumer code
 * is portable the day Phase 2 lands.
 *
 * See /docs/CLOUD_ROADMAP.md Phase 2 for the control-plane design.
 */

import { BoxError } from "./errors.js";

export function cloudNotImplemented(method: string): never {
  throw new BoxError(
    "cloud_unsupported",
    `${method}: cloud control plane is in development — see docs/CLOUD_ROADMAP.md Phase 2.`,
  );
}
