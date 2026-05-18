//! Render the bundled `minimal` template into a temp dir and assert the result.

use std::collections::HashMap;
use std::fs;

use actant_templates::{RenderRequest, TemplateRegistry};

#[test]
fn renders_minimal_into_temp_dir() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dest = tmp.path().join("minimal-out");

    let req = RenderRequest {
        template: "minimal".to_string(),
        destination: dest.clone(),
        project_name: "my-minimal-app".to_string(),
        port: 8500,
        studio_port: 8501,
        vars: HashMap::new(),
    };

    let out = TemplateRegistry::render(req).expect("render minimal");

    // Expected files exist.
    let pkg_path = dest.join("package.json");
    let readme_path = dest.join("README.md");
    let entry_path = dest.join("index.mjs");
    let env_path = dest.join(".env.example");

    for p in [&pkg_path, &readme_path, &entry_path, &env_path] {
        assert!(p.exists(), "expected file missing: {}", p.display());
    }

    // File count matches.
    assert_eq!(
        out.files_written.len(),
        4,
        "expected 4 files, got {:?}",
        out.files_written
    );

    // package.json parses as JSON and contains the substituted name.
    let pkg_text = fs::read_to_string(&pkg_path).expect("read package.json");
    let pkg: serde_json::Value = serde_json::from_str(&pkg_text).expect("package.json parses");
    assert_eq!(pkg["name"], "my-minimal-app");
    assert!(pkg_text.contains("my-minimal-app"));
    assert!(
        !pkg_text.contains("{{project_name}}"),
        "placeholder leaked: {pkg_text}"
    );
    // studio_port substituted inside the scripts.studio command.
    assert!(pkg_text.contains("8501"), "studio_port not substituted");

    // index.mjs contains the substituted project name.
    let entry = fs::read_to_string(&entry_path).expect("read index.mjs");
    assert!(entry.contains("my-minimal-app"));
    assert!(!entry.contains("{{project_name}}"));

    // README contains the project name and the studio port.
    let readme = fs::read_to_string(&readme_path).expect("read README.md");
    assert!(readme.contains("my-minimal-app"));
    assert!(readme.contains("8501"));
    assert!(readme.contains("8500"));
}

#[test]
fn registry_lists_minimal() {
    let names: Vec<_> = TemplateRegistry::list()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert!(
        names.contains(&"minimal".to_string()),
        "missing minimal: {names:?}"
    );
}

#[test]
fn registry_get_minimal_succeeds() {
    let t = TemplateRegistry::get("minimal").expect("get minimal");
    assert_eq!(t.name, "minimal");
    assert!(t.version >= 1);
}

#[test]
fn registry_get_unknown_fails() {
    let err = TemplateRegistry::get("nope-not-real").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown template"), "unexpected error: {msg}");
}
