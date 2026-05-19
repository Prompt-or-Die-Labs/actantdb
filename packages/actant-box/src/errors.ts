/**
 * @actantdb/box — error class.
 *
 * Box has a single public error type so consumers can `instanceof BoxError`
 * and react to typed `code` values. Anything thrown from the public API
 * surface ends up wrapped in (or already is) a BoxError.
 */

export type BoxErrorCode =
  | "not_found"
  | "already_exists"
  | "io_error"
  | "exec_failed"
  | "git_failed"
  | "schedule_not_found"
  | "snapshot_not_found"
  | "invalid_argument"
  | "cloud_unsupported"
  | "deleted"
  | "harness_cli_missing"
  | "harness_timeout"
  | "unknown_harness";

export class BoxError extends Error {
  readonly code: BoxErrorCode;
  override readonly cause?: unknown;

  constructor(code: BoxErrorCode, message: string, cause?: unknown) {
    super(message);
    this.name = "BoxError";
    this.code = code;
    if (cause !== undefined) this.cause = cause;
  }
}
