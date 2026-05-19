# {{project_name}}

Fan-out template — spawns `N` concurrent agent runs (default 20). Useful for:

- Stress-testing the ledger's hash-chain integrity under concurrency.
- Confirming Studio renders many runs in the timeline.
- Benchmarking captures-per-second on your hardware.

Tweak `N` via env: `N=100 npm run demo`.
