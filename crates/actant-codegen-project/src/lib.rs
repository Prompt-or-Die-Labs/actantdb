//! actant-codegen-project — write a scaffolded project to disk.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::Path;

/// Write a minimal project under `root`.
pub fn scaffold(root: &Path, name: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("package.json"),
        actant_templates::package_json(name),
    )?;
    std::fs::write(root.join("README.md"), actant_templates::readme(name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_scaffold() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), "alpha").unwrap();
        assert!(dir.path().join("package.json").exists());
        assert!(dir.path().join("README.md").exists());
    }
}
