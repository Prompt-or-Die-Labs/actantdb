/**
 * Predicate evaluator — JS port of `crates/actant-subscribe/src/predicate.rs`.
 *
 * Predicates are tagged objects: `{ "op": "eq", "field": "...", "value": ... }`.
 * Logical ops: `{ "op": "and", "args": [...] }`, `or`, `not`.
 * Comparators: eq, ne, lt, le, gt, ge, exists.
 * Special: `{ "op": "true" }` / `{ "op": "false" }`.
 *
 * Dotted paths with numeric segments index into arrays (`items.0.id`).
 *
 * No new public type — predicate JSON is accepted at the tool boundary
 * and parsed here. Source of truth for the shape lives in the Rust crate.
 */

export type JSONValue =
  | null
  | boolean
  | number
  | string
  | JSONValue[]
  | { [k: string]: JSONValue };

export type Predicate =
  | { op: "true" }
  | { op: "false" }
  | { op: "eq"; field: string; value: JSONValue }
  | { op: "ne"; field: string; value: JSONValue }
  | { op: "lt"; field: string; value: JSONValue }
  | { op: "le"; field: string; value: JSONValue }
  | { op: "gt"; field: string; value: JSONValue }
  | { op: "ge"; field: string; value: JSONValue }
  | { op: "exists"; field: string }
  | { op: "and"; args: Predicate[] }
  | { op: "or"; args: Predicate[] }
  | { op: "not"; arg: Predicate };

export function evaluatePredicate(pred: unknown, root: JSONValue): boolean {
  if (!isObject(pred)) return false;
  const op = (pred as { op?: unknown }).op;
  switch (op) {
    case "true":
      return true;
    case "false":
      return false;
    case "exists":
      return resolve(root, (pred as { field: string }).field) !== undefined;
    case "eq": {
      const v = resolve(root, (pred as { field: string }).field);
      if (v === undefined) return false;
      return deepEq(v, (pred as { value: JSONValue }).value);
    }
    case "ne": {
      const v = resolve(root, (pred as { field: string }).field);
      // Documented exception in the Rust predicate: missing => true for `ne`.
      if (v === undefined) return true;
      return !deepEq(v, (pred as { value: JSONValue }).value);
    }
    case "lt":
      return cmp(pred, root, (o) => o < 0);
    case "le":
      return cmp(pred, root, (o) => o <= 0);
    case "gt":
      return cmp(pred, root, (o) => o > 0);
    case "ge":
      return cmp(pred, root, (o) => o >= 0);
    case "and": {
      const xs = (pred as { args?: unknown }).args;
      if (!Array.isArray(xs)) return false;
      return xs.every((p) => evaluatePredicate(p, root));
    }
    case "or": {
      const xs = (pred as { args?: unknown }).args;
      if (!Array.isArray(xs)) return false;
      return xs.some((p) => evaluatePredicate(p, root));
    }
    case "not":
      return !evaluatePredicate((pred as { arg: Predicate }).arg, root);
    default:
      return false;
  }
}

function cmp(pred: unknown, root: JSONValue, ok: (n: number) => boolean): boolean {
  const field = (pred as { field?: string }).field;
  const lit = (pred as { value?: JSONValue }).value;
  if (typeof field !== "string" || lit === undefined) return false;
  const got = resolve(root, field);
  if (got === undefined) return false;
  const ord = order(got, lit);
  if (ord === undefined) return false;
  return ok(ord);
}

function resolve(root: JSONValue, path: string): JSONValue | undefined {
  if (path.length === 0) return undefined;
  let cur: JSONValue | undefined = root;
  for (const seg of path.split(".")) {
    if (seg.length === 0) return undefined;
    if (cur === null || cur === undefined) return undefined;
    if (Array.isArray(cur)) {
      const idx = Number(seg);
      if (!Number.isInteger(idx) || idx < 0) return undefined;
      cur = cur[idx];
    } else if (typeof cur === "object") {
      cur = (cur as { [k: string]: JSONValue })[seg];
    } else {
      return undefined;
    }
  }
  return cur;
}

function order(a: JSONValue, b: JSONValue): number | undefined {
  if (typeof a === "number" && typeof b === "number") {
    return a - b;
  }
  if (typeof a === "string" && typeof b === "string") {
    return a < b ? -1 : a > b ? 1 : 0;
  }
  if (typeof a === "boolean" && typeof b === "boolean") {
    return Number(a) - Number(b);
  }
  if (a === null && b === null) return 0;
  return undefined;
}

function deepEq(a: JSONValue, b: JSONValue): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      const av = a[i];
      const bv = b[i];
      if (av === undefined || bv === undefined) return false;
      if (!deepEq(av, bv)) return false;
    }
    return true;
  }
  if (typeof a === "object" && typeof b === "object") {
    const ak = Object.keys(a as object);
    const bk = Object.keys(b as object);
    if (ak.length !== bk.length) return false;
    for (const k of ak) {
      const av = (a as { [k: string]: JSONValue })[k];
      const bv = (b as { [k: string]: JSONValue })[k];
      if (av === undefined || bv === undefined) return false;
      if (!deepEq(av, bv)) return false;
    }
    return true;
  }
  return false;
}

function isObject(x: unknown): x is Record<string, unknown> {
  return typeof x === "object" && x !== null && !Array.isArray(x);
}
