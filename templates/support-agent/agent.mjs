#!/usr/bin/env node
// {{project_name}} — customer-support agent template.
//
// Tools: lookup_order, issue_refund. The refund tool has
// require_approval=true in the policy; this template shows the full
// approval flow including a custom resolver that denies large refunds.

import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { withActant } from "@actantdb/mastra";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "{{project_name}}";

const policy = {
  tools: [
    { tool: "issue_refund", require_approval: true },
    { tool: "lookup_order", risk: "low" },
  ],
  deny: [{ tool: "issue_refund", pattern: '"order_id":"test_', reason: "no refunds for test orders" }],
};

const agent = {
  name: "support",
  tools: {
    lookup_order: {
      id: "lookup_order",
      execute: async ({ order_id }) => ({
        order_id,
        amount: order_id === "ord_small" ? 25 : 500,
        status: "fulfilled",
      }),
    },
    issue_refund: {
      id: "issue_refund",
      execute: async ({ order_id, amount }) => ({ ok: true, refunded: amount, order_id }),
    },
  },
  generate: async ({ order_id }) => {
    const order = await agent.tools.lookup_order.execute({ order_id });
    return await agent.tools.issue_refund.execute({ order_id, amount: order.amount });
  },
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy,
  resolveApproval: async (req) => {
    const amt = req.args?.amount ?? 0;
    return amt > 100
      ? { decision: "deny", approver: "policy-bot", reason: `amount ${amt} > 100` }
      : { decision: "approve", approver: "policy-bot", scope: "once" };
  },
});

for (const order_id of ["ord_small", "ord_big", "test_oops"]) {
  const r = await wrapped.run({ message: `refund ${order_id}`, input: { order_id } });
  process.stdout.write(`${order_id} => ${JSON.stringify(r.result)}\n`);
}
process.stdout.write(`Studio: npx actantdb studio --project ${PROJECT} --store-dir ${STORE_DIR} --port {{studio_port}}\n`);
