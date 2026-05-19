/**
 * @actantdb/policy — Guard verdict builders and the v0.1 policy evaluator.
 *
 * The v0.1 policy supports four mechanisms (see /CHANGELOG.md days 22–35):
 *   - per-tool risk class
 *   - regex deny-list on argument JSON
 *   - sensitivity ceiling (max sensitivity allowed without approval)
 *   - hardcoded default: shell.run requires approval
 *
 * Guard issues one of five verdicts: allow | constrain | require_approval | block | halt.
 */

import { sha256OfJSON } from "@actantdb/core";
import type {
  Policy,
  PolicyVerdict,
  Risk,
  Sensitivity,
  ToolCallRequest,
} from "@actantdb/types";

/** Builder helpers used by tests and Guard. */
export const verdict = {
  allow(reason: string, policySnapshot: string): PolicyVerdict {
    return { decision: "allow", reason, policy_snapshot: policySnapshot };
  },
  constrain(
    reason: string,
    policySnapshot: string,
    constrainedInput: unknown,
    hint: string,
  ): PolicyVerdict {
    return {
      decision: "constrain",
      reason,
      policy_snapshot: policySnapshot,
      constrained_input: constrainedInput,
      hint,
    };
  },
  requireApproval(
    reason: string,
    policySnapshot: string,
    opts: { hint?: string; constrainedInput?: unknown } = {},
  ): PolicyVerdict {
    return {
      decision: "require_approval",
      reason,
      policy_snapshot: policySnapshot,
      ...(opts.hint !== undefined ? { hint: opts.hint } : {}),
      ...(opts.constrainedInput !== undefined ? { constrained_input: opts.constrainedInput } : {}),
    };
  },
  block(reason: string, policySnapshot: string): PolicyVerdict {
    return { decision: "block", reason, policy_snapshot: policySnapshot };
  },
  halt(reason: string, policySnapshot: string): PolicyVerdict {
    return { decision: "halt", reason, policy_snapshot: policySnapshot };
  },
} as const;

/** Compute the snapshot hash of a policy. Used to seal verdicts to a policy. */
export function snapshotHash(p: Policy): string {
  return sha256OfJSON(p);
}

/** Looks up the configured risk for a tool, or returns `low` by default. */
export function riskOf(policy: Policy, tool: string): Risk {
  const entry = policy.tools?.find((t) => t.tool === tool);
  return entry?.risk ?? "low";
}

/** Returns true if a tool has `require_approval` set explicitly. */
export function requiresApproval(policy: Policy, tool: string): boolean {
  const entry = policy.tools?.find((t) => t.tool === tool);
  return entry?.require_approval === true;
}

/**
 * Compare two sensitivity levels.
 * Returns positive if `a` is strictly higher than `b`.
 */
const SENS_ORDER: Record<Sensitivity, number> = {
  public: 0,
  low: 1,
  medium: 2,
  high: 3,
  secret: 4,
};

export function sensitivityExceeds(a: Sensitivity, ceiling: Sensitivity): boolean {
  return SENS_ORDER[a] > SENS_ORDER[ceiling];
}

/** Evaluate a tool call request against the policy. */
export function evaluate(
  policy: Policy,
  req: ToolCallRequest,
  ctx: { argsSensitivity?: Sensitivity } = {},
): PolicyVerdict {
  const snap = snapshotHash(policy);
  const argsJson = JSON.stringify(req.args ?? {});

  // 1. Deny-list (regex) → block.
  for (const rule of policy.deny ?? []) {
    if (rule.tool !== "*" && rule.tool !== req.tool) continue;
    let regex: RegExp;
    try {
      regex = new RegExp(rule.pattern);
    } catch {
      continue;
    }
    if (regex.test(argsJson)) {
      // Killer demo: rm -rf with /dist → suggest the safer variant.
      const constrained = suggestConstrainForShellRm(req);
      if (constrained) {
        return verdict.requireApproval(rule.reason, snap, {
          hint: `drop ${constrained.dropped}`,
          constrainedInput: constrained.args,
        });
      }
      return verdict.block(rule.reason, snap);
    }
  }

  // 2. Sensitivity ceiling.
  if (
    policy.sensitivity_ceiling &&
    ctx.argsSensitivity &&
    sensitivityExceeds(ctx.argsSensitivity, policy.sensitivity_ceiling)
  ) {
    return verdict.requireApproval(
      `args sensitivity ${ctx.argsSensitivity} exceeds ceiling ${policy.sensitivity_ceiling}`,
      snap,
    );
  }

  // 3. Per-tool explicit require_approval.
  if (requiresApproval(policy, req.tool)) {
    return verdict.requireApproval(`tool ${req.tool} is configured require_approval`, snap);
  }

  // 4. Hardcoded v0.1 default: shell.run requires approval.
  if (req.tool === "shell.run") {
    const constrained = suggestConstrainForShellRm(req);
    if (constrained) {
      return verdict.requireApproval(
        `shell.run requires approval (destructive pattern detected)`,
        snap,
        { hint: `drop ${constrained.dropped}`, constrainedInput: constrained.args },
      );
    }
    return verdict.requireApproval(`shell.run requires approval by default`, snap);
  }

  // 5. Risk-based gating: destructive → require approval, high → allow with note.
  const risk = riskOf(policy, req.tool);
  if (risk === "destructive") {
    return verdict.requireApproval(`tool ${req.tool} classified destructive`, snap);
  }

  return verdict.allow(`risk=${risk}`, snap);
}

/** If the proposed shell.run is `rm -rf` with multiple paths and at least one
 * dangerous path (/dist, /, ~), suggest dropping the dangerous path. Returns
 * undefined if no constrain hint applies. */
function suggestConstrainForShellRm(
  req: ToolCallRequest,
): { args: unknown; dropped: string } | undefined {
  if (req.tool !== "shell.run") return undefined;
  const args = req.args as { command?: string } | undefined;
  const cmd = args?.command;
  if (typeof cmd !== "string") return undefined;
  const match = cmd.match(/^\s*rm\s+-rf\s+(.+)$/);
  if (!match) return undefined;
  const tokens = match[1]!.trim().split(/\s+/);
  if (tokens.length < 2) return undefined;
  const dangerous = tokens.filter(
    (t) => t === "/" || t.endsWith("/dist") || t === "dist" || t.startsWith("~"),
  );
  if (dangerous.length === 0) return undefined;
  const dropped = dangerous[0]!;
  const remaining = tokens.filter((t) => t !== dropped);
  if (remaining.length === 0) return undefined;
  const newCmd = `rm -rf ${remaining.join(" ")}`;
  return { args: { ...args, command: newCmd }, dropped };
}

/** A small starter policy that lights up the killer demo. */
export const demoPolicy: Policy = {
  label: "v0.1 demo policy",
  sensitivity_ceiling: "high",
  tools: [
    { tool: "shell.run", risk: "destructive", require_approval: true },
    { tool: "file.write", risk: "medium" },
    { tool: "file.read", risk: "low" },
  ],
  deny: [
    {
      tool: "shell.run",
      pattern: "rm\\s+-rf\\s+(.*\\s)?/?dist",
      reason: "rm -rf includes /dist — release artifacts",
    },
    {
      tool: "shell.run",
      pattern: "rm\\s+-rf\\s+/(?!\\w)",
      reason: "rm -rf on filesystem root",
    },
  ],
};
