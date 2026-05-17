# Work package: `actant-templates`

## Context

Bundled project templates for `actant new`. Each template lives at `/templates/<name>/` and is embedded in the CLI binary at build time so `actant` is a single-file install.

A template is a working ActantDB project. The CLI renders it with substitution (project name, ports, language choice) and runs post-render hooks (init git, run `actant schema apply`, install SDK).

## Specs to read first

- `/planning/cli-templates.md` — catalog + conventions.
- `/planning/cli-design.md` §"Project layout (scaffolded)" — the expected file tree.
- `/specs/10-alpha-demo.md` — the `coding-agent` template materializes this demo.

## Scope

### Public API

```rust
pub struct Template { pub name: String, pub version: u32, pub languages: Vec<Language>, pub features: Vec<Feature>, pub min_cli_version: Version }

pub struct RenderRequest {
    pub template: String,
    pub destination: PathBuf,
    pub project_name: String,
    pub language: Language,
    pub features: Vec<Feature>,
    pub port: u16,
    pub studio_port: u16,
    pub vars: HashMap<String, String>,
}

pub struct TemplateRegistry { /* loads all bundled templates at compile time via include_dir! */ }

impl TemplateRegistry {
    pub fn list() -> Vec<Template>;
    pub fn get(name: &str) -> Result<Template, TemplateError>;
    pub fn render(req: RenderRequest) -> Result<RenderOutput, TemplateError>;
}

pub struct RenderOutput { pub files_written: Vec<PathBuf>, pub post_hooks: Vec<PostHook> }
```

### Variable substitution

- `{{project_name}}`, `{{port}}`, `{{studio_port}}`, `{{language}}`.
- Language-conditional sections via `# {{#if language == "python"}} ... {{/if}}` (a tiny templating engine; no external crate dependency).

### Bundled templates (Phase 1)

- `minimal` — smallest functional project. Must exist as `/templates/minimal/`.
- `coding-agent` — the alpha demo. Must exist as `/templates/coding-agent/`.

Subsequent phase templates land per `/planning/cli-templates.md` § "Ship order". This work package provides the engine; individual template content is its own work package per template (one per phase).

### Internal modules

```
crates/actant-templates/src/
├── lib.rs
├── registry.rs                  (include_dir! of /templates/)
├── render.rs                    (variable substitution)
├── language.rs                  (Language enum + per-lang post-hooks)
├── feature.rs                   (Feature enum)
├── post_hook.rs                 (RunHook trait; git/install/etc.)
└── error.rs
```

### Tests

- `minimal` renders to a temp dir with expected files; the rendered `actant.yaml` parses.
- `coding-agent` renders; the rendered project's `actant dev --headless` boots clean.
- Substitution: `{{project_name}}` appears in every expected file.
- Refuse: rendering into a non-empty directory without `--force`.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] `minimal` and `coding-agent` are bundled and renderable.
- [ ] Rendered `minimal` passes `actant doctor`.
- [ ] Rendered `coding-agent` runs the alpha demo from `/specs/10-alpha-demo.md`.

## Do NOT

- Do NOT depend on an external templating crate. Keep the substitution engine in-tree.
- Do NOT bundle binary assets > 100 KB in any template.
- Do NOT include secrets in any template. Use `.env.example` only.

## Hand-off

`just ci`, then `actant new --template coding-agent test-project && cd test-project && actant dev`.
