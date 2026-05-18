//! Renderer must refuse to overwrite a non-empty destination directory.
//!
//! `RenderRequest` has no `force` flag (see work package; deferred), so the
//! correct behaviour is always to error out. Existing-but-empty dirs are OK.

use std::collections::HashMap;
use std::fs;

use actant_templates::{RenderRequest, TemplateError, TemplateRegistry};

fn req_into(dest: std::path::PathBuf) -> RenderRequest {
    RenderRequest {
        template: "minimal".to_string(),
        destination: dest,
        project_name: "demo".to_string(),
        port: 8400,
        studio_port: 8401,
        vars: HashMap::new(),
    }
}

#[test]
fn refuses_non_empty_destination() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dest = tmp.path().join("not-empty");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("stray.txt"), b"already here").unwrap();

    let err = TemplateRegistry::render(req_into(dest.clone())).unwrap_err();
    match err {
        TemplateError::DestinationNotEmpty(p) => assert_eq!(p, dest),
        other => panic!("expected DestinationNotEmpty, got {other:?}"),
    }

    // The stray file must still be there — we didn't partially write.
    assert!(dest.join("stray.txt").exists());
}

#[test]
fn allows_existing_but_empty_destination() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dest = tmp.path().join("empty-dir");
    fs::create_dir_all(&dest).unwrap();

    TemplateRegistry::render(req_into(dest.clone())).expect("render into empty dir");
    assert!(dest.join("package.json").exists());
}

#[test]
fn allows_missing_destination() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dest = tmp.path().join("does/not/yet/exist");

    TemplateRegistry::render(req_into(dest.clone())).expect("render creates path");
    assert!(dest.join("package.json").exists());
}
