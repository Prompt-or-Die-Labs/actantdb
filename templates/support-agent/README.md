# {{project_name}}

Customer-support refund agent. Demonstrates ActantDB's approval flow:

- Small refund (`ord_small`, $25) → policy-bot approves → executes.
- Large refund (`ord_big`, $500) → policy-bot denies for being over $100.
- Test order (`test_oops`) → policy `deny` rule fires before any tool runs.

Run `npm install && npm run demo`, then `npm run studio` to inspect the three
runs side-by-side.
