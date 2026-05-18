//! Render the bundled `coding-agent` template and assert all expected files
//! exist, that `package.json` parses, and that the agent entry point contains
//! the substituted project name.

use std::collections::HashMap;
use std::fs;

use actant_templates::{RenderRequest, TemplateRegistry};

#[test]
fn renders_coding_agent_into_temp_dir() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dest = tmp.path().join("coding-agent-out");

    let req = RenderRequest {
        template: "coding-agent".to_string(),
        destination: dest.clone(),
        project_name: "my-coding-agent".to_string(),
        port: 8600,
        studio_port: 8601,
        vars: HashMap::new(),
    };

    let out = TemplateRegistry::render(req).expect("render coding-agent");

    let pkg_path = dest.join("package.json");
    let readme_path = dest.join("README.md");
    let agent_path = dest.join("agent.mjs");
    let env_path = dest.join(".env.example");

    for p in [&pkg_path, &readme_path, &agent_path, &env_path] {
        assert!(p.exists(), "expected file missing: {}", p.display());
    }

    assert_eq!(out.files_written.len(), 4, "expected 4 files, got {:?}", out.files_written);

    // package.json parses + contains substituted name.
    let pkg_text = fs::read_to_string(&pkg_path).expect("read package.json");
    let pkg: serde_json::Value = serde_json::from_str(&pkg_text).expect("package.json parses");
    assert_eq!(pkg["name"], "my-coding-agent");
    // All three demo-mode dependencies are present (this template tracks the wedge demo).
    let deps = &pkg["dependencies"];
    for k in ["@actantdb/core", "@actantdb/mastra", "@actantdb/policy"] {
        assert!(deps.get(k).is_some(), "missing dep {k}: {pkg_text}");
    }
    assert!(pkg_text.contains("8601"), "studio_port not substituted");

    // agent.mjs contains the substituted project name and no leftover placeholder.
    let agent = fs::read_to_string(&agent_path).expect("read agent.mjs");
    assert!(agent.contains("my-coding-agent"));
    assert!(!agent.contains("{{project_name}}"));
    // The stub planner imports the three packages the wedge demo uses.
    assert!(agent.contains("@actantdb/core"));
    assert!(agent.contains("@actantdb/mastra"));
    assert!(agent.contains("@actantdb/policy"));
    // The shell.run + file.write tool surface is present.
    assert!(agent.contains("shell.run"));
    assert!(agent.contains("file.write"));

    // README mentions the project name and both ports.
    let readme = fs::read_to_string(&readme_path).expect("read README.md");
    assert!(readme.contains("my-coding-agent"));
    assert!(readme.contains("8600"));
    assert!(readme.contains("8601"));
}

#[test]
fn caller_supplied_vars_override_canonical_fields() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dest = tmp.path().join("override-out");

    let mut vars = HashMap::new();
    vars.insert("project_name".to_string(), "overridden-name".to_string());

    let req = RenderRequest {
        template: "coding-agent".to_string(),
        destination: dest.clone(),
        project_name: "ignored-name".to_string(),
        port: 8600,
        studio_port: 8601,
        vars,
    };

    TemplateRegistry::render(req).expect("render");

    let agent = fs::read_to_string(dest.join("agent.mjs")).expect("read agent.mjs");
    assert!(agent.contains("overridden-name"), "vars did not override canonical project_name");
    assert!(!agent.contains("ignored-name"));
}

#[test]
fn registry_lists_coding_agent() {
    let names: Vec<_> = TemplateRegistry::list()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert!(
        names.contains(&"coding-agent".to_string()),
        "missing coding-agent: {names:?}"
    );
}
