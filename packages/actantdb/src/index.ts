// ActantDB — umbrella export. Single import, everything in one place.
//
// Minimal usage:
//
//   import { openLedger, evaluate, withActant } from "actantdb";
//
//   const ledger = openLedger("my-project");
//   const wrapped = withActant(myAgent, { ledger });
//   const verdict = evaluate(myPolicy, toolCall);
//
// If you only need one piece, prefer the individual package — same code,
// smaller install:
//
//   import { openLedger } from "@actantdb/core";
//   import { evaluate }   from "@actantdb/policy";
//   import { withActant } from "@actantdb/mastra";
//
// All exports here are re-exports — no new types, no new behavior.
export * from "@actantdb/core";
export * from "@actantdb/policy";
export * from "@actantdb/mastra";
export * from "@actantdb/replay";
export * from "@actantdb/sdk";
export type * from "@actantdb/types";
