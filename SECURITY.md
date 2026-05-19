# Security Policy

ActantDB takes security seriously. This document explains how to report
vulnerabilities and what to expect after you do.

## Reporting a vulnerability

Email **security@actantdb.dev** with the details. Please do **not** open
a public GitHub issue, post in Discord, or otherwise discuss the
vulnerability in a public channel until we have had a chance to
investigate and ship a fix.

What to include in the report:

- A short description of the issue.
- The affected component (crate name, npm package, SDK, Studio panel,
  CLI command, deployment recipe).
- The affected version(s). If you are working off `main`, the commit
  SHA is enough.
- Steps to reproduce, ideally with a minimal test case.
- The impact you observed (information disclosure, privilege escalation,
  denial of service, etc.).
- Whether you would like public credit when the advisory is published.

If you want to encrypt the report, the maintainers' PGP key is published
in the repository as `SECURITY-PGP.asc` (placeholder — file is added
alongside the first published advisory). Fingerprint:
`0000 0000 0000 0000 0000  0000 0000 0000 0000 0000` (placeholder; the
real fingerprint will be cross-published on the actantdb.dev website
once an advisory needs encryption).

## Disclosure SLA

We aim for a **90-day coordinated disclosure window**. The clock starts
when we acknowledge your report.

- **Within 3 business days:** initial human acknowledgement.
- **Within 14 days:** triage outcome — confirmed, needs-info, or
  declined-with-reasoning.
- **Within 90 days:** a fix released to `main` and tagged in a point
  release, with an advisory published in the GitHub Security tab.

If a fix is not realistic inside 90 days (cross-crate refactor, upstream
dependency, etc.) we will tell you and agree on an extended window in
writing before the original 90 days expires.

## In scope

The OSS substrate hosted at <https://github.com/Prompt-or-Die-Labs/actantdb>:

- All Rust crates under `crates/` (kernel, server, storage, workers,
  reliability, auth, sync, contracts, etc.).
- All npm packages published under `@actantdb/*`.
- The Python, Swift, and Rust SDKs under `sdks/`.
- Studio (`packages/actant-studio`) and the `actantdb` CLI.
- The deployment recipes under `deploy/` (Docker Compose, Helm chart).
- The HTTP + WebSocket API exposed by `actantdb-server`.
- Reproducible scaffolding in `templates/` and `examples/`.

## Out of scope

- **Third-party dependencies.** Please report these to the relevant
  upstream project directly. We will pick up the fix when we cut a
  release that bumps the dep. If a dep has no upstream remediation
  path, file a normal issue here and we will look at swapping it.
- **Issues that require pre-authenticated workspace owner privileges
  to exploit.** A workspace owner already has full control of their
  own ledger — that is the design. If you find an issue that allows a
  workspace owner to escape into another workspace's data, that is
  in scope.
- **Best-practice findings without a concrete exploit** (e.g. "you
  should add a CSP header" with no working attack). These are welcome
  as normal issues or pull requests.
- **Self-host deployments operated by other people.** We can only
  address vulnerabilities in the code we ship.

## Bug bounty

We do **not** currently run a paid bug bounty program. Reporters who
ask for public credit will be acknowledged in the advisory and in the
release notes for the version that contains the fix.

## Hall of fame

(Empty for now — your name could go here.)
