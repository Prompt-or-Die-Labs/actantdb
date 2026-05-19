//! `actantdb init` — scaffold a new project from a bundled template.

use std::path::PathBuf;

use actant_templates::{RenderRequest, TemplateRegistry};

/// Print the bundled-template list.
pub fn list() {
    let templates = TemplateRegistry::list();
    if templates.is_empty() {
        println!("(no bundled templates)");
        return;
    }
    println!("Available templates:\n");
    for t in templates {
        println!("  {:<20} v{}  {}", t.name, t.version, t.description);
    }
    println!("\nUse: actantdb init <name> [--name <project>] [--dir <dir>]");
}

/// Render the template into `dir` (defaults to `./<project_name>`).
pub fn run(template: String, name: Option<String>, dir: Option<PathBuf>) -> anyhow::Result<()> {
    // Resolve names.
    let project_name = name.unwrap_or_else(|| template.clone());
    let dir = dir.unwrap_or_else(|| PathBuf::from(format!("./{project_name}")));

    let req = RenderRequest::new(template.clone(), dir.clone(), project_name.clone());
    let out = TemplateRegistry::render(req)
        .map_err(|e| anyhow::anyhow!("render template `{template}`: {e}"))?;

    println!("Scaffolded `{template}` into {}", dir.display());
    println!("  ({} files written)", out.files_written.len());
    println!();
    println!("Next steps:");
    println!("  cd {} && npm install && npm run demo", dir.display());
    Ok(())
}
