# Codegen project fixtures

The `actant-codegen-project` crate currently exposes a single
`scaffold(root, name) -> io::Result<()>` entry point which writes a
TypeScript/Node project (`package.json` + `README.md`). The per-language
generators described in `/agents/actant-codegen-project.md` (`CommandGen`,
`EffectGen`, `WorkerGen`, `AgentGen`, `WorkflowGen`) are not yet implemented
in this crate.

The fixtures below are project names exercised by
`per_language_generator.rs`. When the real per-language generators land, swap
the placeholders out for real `.actant` input files.
