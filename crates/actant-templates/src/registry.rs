//! Bundled-template registry.
//!
//! Templates live at `/templates/<name>/` in the repo and are embedded into the
//! crate at compile time via [`include_dir!`]. The registry exposes [`list`],
//! [`get`], and [`render`] over that bundle.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir, DirEntry};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::render::substitute;

/// All bundled templates, embedded at compile time.
///
/// Path is anchored to `$CARGO_MANIFEST_DIR` so it resolves correctly regardless
/// of the working directory at build time.
static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../templates");

/// Default port the substrate server binds to when a template is rendered
/// without an explicit override.
pub const DEFAULT_PORT: u16 = 8400;

/// Default port Studio binds to when a template is rendered without an
/// explicit override.
pub const DEFAULT_STUDIO_PORT: u16 = 8401;

/// Metadata describing one bundled template.
///
/// Surfaced via [`TemplateRegistry::list`] and [`TemplateRegistry::get`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    /// Stable identifier — also the directory name under `/templates/`.
    pub name: String,
    /// Template-content version. Bumped when the bundled files change shape.
    pub version: u32,
    /// One-line description shown by `actant new --list`.
    pub description: String,
}

/// Inputs for [`TemplateRegistry::render`].
///
/// Variable precedence: `vars` is layered on top of the canonical fields
/// (`project_name`, `port`, `studio_port`), so an explicit entry in `vars`
/// wins over the canonical field. This lets callers override a derived value
/// without having to recompute the canonical map.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    /// Template name (must match a [`Template::name`] returned by `list()`).
    pub template: String,
    /// Destination directory. Must either not exist or be empty.
    pub destination: PathBuf,
    /// Substituted into `{{project_name}}`.
    pub project_name: String,
    /// Substituted into `{{port}}` (default substrate port).
    pub port: u16,
    /// Substituted into `{{studio_port}}` (default Studio port).
    pub studio_port: u16,
    /// Additional variables. Override canonical fields by key.
    pub vars: HashMap<String, String>,
}

impl RenderRequest {
    /// Construct a request with default ports filled in.
    pub fn new(
        template: impl Into<String>,
        destination: PathBuf,
        project_name: impl Into<String>,
    ) -> Self {
        Self {
            template: template.into(),
            destination,
            project_name: project_name.into(),
            port: DEFAULT_PORT,
            studio_port: DEFAULT_STUDIO_PORT,
            vars: HashMap::new(),
        }
    }
}

/// Result of a successful render.
#[derive(Debug, Clone, Default)]
pub struct RenderOutput {
    /// Files written, in deterministic (sorted) order.
    pub files_written: Vec<PathBuf>,
}

/// Errors returned by [`TemplateRegistry`] operations.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// The requested template name is not bundled in this build.
    #[error("unknown template: {0}")]
    UnknownTemplate(String),
    /// The destination directory exists but is not empty (and `--force` was not set).
    #[error("destination is not empty: {0}")]
    DestinationNotEmpty(PathBuf),
    /// A filesystem error while writing the rendered project.
    #[error("io error at {path}: {source}")]
    Io {
        /// Path the IO error happened at.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
}

/// Compile-time bundle of project templates exposed as a registry.
///
/// All operations are zero-arg (stateless); we keep the type so we can attach
/// methods and so the public surface matches the work-package spec.
#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateRegistry;

impl TemplateRegistry {
    /// List all templates bundled in this build.
    pub fn list() -> Vec<Template> {
        let mut out: Vec<Template> = TEMPLATES
            .dirs()
            .filter_map(|d| {
                let name = d.path().file_name()?.to_str()?.to_string();
                Some(builtin_template(&name))
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Look up metadata for one template.
    pub fn get(name: &str) -> Result<Template, TemplateError> {
        if TEMPLATES.get_dir(name).is_none() {
            return Err(TemplateError::UnknownTemplate(name.to_string()));
        }
        Ok(builtin_template(name))
    }

    /// Render `req.template` into `req.destination`, substituting `{{...}}`
    /// placeholders in every file body and in every relative file path.
    pub fn render(req: RenderRequest) -> Result<RenderOutput, TemplateError> {
        let dir = TEMPLATES
            .get_dir(req.template.as_str())
            .ok_or_else(|| TemplateError::UnknownTemplate(req.template.clone()))?;

        ensure_empty(&req.destination)?;
        mkdir_p(&req.destination)?;

        let vars = build_vars(&req);

        let mut files_written = Vec::new();
        write_dir(
            dir,
            &req.template,
            &req.destination,
            &vars,
            &mut files_written,
        )?;
        files_written.sort();
        Ok(RenderOutput { files_written })
    }
}

fn builtin_template(name: &str) -> Template {
    match name {
        "minimal" => Template {
            name: "minimal".to_string(),
            version: 1,
            description: "Smallest functional ActantDB project — embedded ledger + a no-op agent."
                .to_string(),
        },
        "coding-agent" => Template {
            name: "coding-agent".to_string(),
            version: 1,
            description:
                "Coding agent with shell.run + file.write tools, wrapped through @actantdb/mastra."
                    .to_string(),
        },
        other => Template {
            name: other.to_string(),
            version: 1,
            description: format!("Bundled template: {other}"),
        },
    }
}

fn build_vars(req: &RenderRequest) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("project_name".to_string(), req.project_name.clone());
    vars.insert("port".to_string(), req.port.to_string());
    vars.insert("studio_port".to_string(), req.studio_port.to_string());
    // Caller-supplied vars layered on top.
    for (k, v) in &req.vars {
        vars.insert(k.clone(), v.clone());
    }
    vars
}

fn ensure_empty(dest: &Path) -> Result<(), TemplateError> {
    match fs::read_dir(dest) {
        Ok(mut iter) => {
            if iter.next().is_some() {
                return Err(TemplateError::DestinationNotEmpty(dest.to_path_buf()));
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(TemplateError::Io {
            path: dest.to_path_buf(),
            source,
        }),
    }
}

fn mkdir_p(path: &Path) -> Result<(), TemplateError> {
    fs::create_dir_all(path).map_err(|source| TemplateError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_dir(
    dir: &Dir<'_>,
    template_root: &str,
    dest_root: &Path,
    vars: &HashMap<String, String>,
    files_written: &mut Vec<PathBuf>,
) -> Result<(), TemplateError> {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => write_dir(sub, template_root, dest_root, vars, files_written)?,
            DirEntry::File(file) => {
                let rel = file
                    .path()
                    .strip_prefix(template_root)
                    .unwrap_or(file.path());
                // Substitute placeholders into the relative path too, so a file
                // like `{{project_name}}.config.json` works if a template ever
                // needs it.
                let rel_str = rel.to_string_lossy().to_string();
                let substituted_rel = substitute(&rel_str, vars);
                let target = dest_root.join(substituted_rel);
                if let Some(parent) = target.parent() {
                    mkdir_p(parent)?;
                }
                let body = match std::str::from_utf8(file.contents()) {
                    Ok(text) => substitute(text, vars).into_bytes(),
                    Err(_) => file.contents().to_vec(),
                };
                fs::write(&target, &body).map_err(|source| TemplateError::Io {
                    path: target.clone(),
                    source,
                })?;
                files_written.push(target);
            }
        }
    }
    Ok(())
}
