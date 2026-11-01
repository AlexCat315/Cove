//! Checks the final workspace without deleting its migration history.

use crate::{Result, process};
use serde_json::{Value, json};
use std::path::{Component, Path};

fn check_inventory(root: &Path, document: &Value) -> Result<usize> {
    let entries = document["entries"]
        .as_array()
        .ok_or("inventory has no entries")?;
    if entries.is_empty() {
        return Err("migration inventory is empty".into());
    }
    for entry in entries {
        let source = entry["source"].as_str().ok_or("entry has no source")?;
        if entry["status"] == "pending" || entry["planned"] == true {
            return Err(format!("unmapped source: {source}"));
        }
        let targets = entry["rust"]
            .as_array()
            .ok_or_else(|| format!("no targets: {source}"))?;
        if targets.is_empty() {
            return Err(format!("no targets: {source}"));
        }
        for target in targets {
            let target = target.as_str().ok_or("target must be a path")?;
            let path = Path::new(target);
            if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
                return Err(format!("target escapes workspace: {target}"));
            }
            if !root.join(path).is_file() {
                return Err(format!("missing migration target: {source} -> {target}"));
            }
        }
        if entry["source_sha256"]
            .as_str()
            .is_none_or(|hash| hash.len() != 64)
        {
            return Err(format!("missing source checksum: {source}"));
        }
    }
    Ok(entries.len())
}

fn check_sources(directory: &Path) -> Result<()> {
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(
            name.as_ref(),
            ".git"
                | "target"
                | "third-party"
                | "node_modules"
                | "build"
                | "dist"
                | ".build"
                | ".cache"
                | "Artifacts"
        ) {
            continue;
        }
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            check_sources(&path)?;
        } else if path.extension().is_some_and(|e| e == "swift") || name == "Package.resolved" {
            return Err(format!(
                "legacy Swift source or manifest: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

pub(crate) fn run(as_json: bool) -> Result<()> {
    let metadata: Value = serde_json::from_str(&process::output(process::cargo().args([
        "metadata",
        "--no-deps",
        "--locked",
        "--format-version",
        "1",
    ]))?)
    .map_err(|e| e.to_string())?;
    let root = Path::new(
        metadata["workspace_root"]
            .as_str()
            .ok_or("workspace root missing")?,
    );
    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("Docs/rust-migration-files.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let sources = check_inventory(root, &document)?;
    check_sources(root)?;
    let packages = metadata["packages"].as_array().ok_or("no packages")?;
    let mut tests = 0;
    for package in packages {
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("missing manifest")?,
        );
        let directory = manifest.parent().ok_or("manifest has no parent")?;
        for target in package["targets"].as_array().ok_or("missing targets")? {
            let source = Path::new(target["src_path"].as_str().ok_or("missing target source")?);
            let kind = target["kind"].as_array().ok_or("missing target kind")?;
            if kind.iter().any(|k| k == "test") {
                tests += 1;
                if !source.starts_with(directory.join("tests")) {
                    return Err(format!(
                        "integration test outside tests/: {}",
                        source.display()
                    ));
                }
            } else if kind.iter().any(|k| k == "custom-build") {
                if source != directory.join("build.rs") {
                    return Err(format!(
                        "build script outside crate root: {}",
                        source.display()
                    ));
                }
            } else if !source.starts_with(directory.join("src")) {
                return Err(format!("Rust target outside src/: {}", source.display()));
            }
        }
    }
    if as_json {
        println!(
            "{}",
            json!({"packages": packages.len(), "integration_test_targets": tests, "retired_swift_sources": sources, "missing_targets": 0, "owned_swift_files": 0})
        );
    } else {
        println!(
            "{} packages, {tests} integration test targets, {sources} retained source mappings; no owned Swift files",
            packages.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory(target: &str) -> Value {
        json!({"entries": [{"source": "Original.swift", "source_sha256": "a".repeat(64), "status": "written_unverified", "rust": [target], "planned": false}]})
    }

    #[test]
    fn removing_the_source_preserves_its_mapping_and_missing_targets_fail() {
        let root = tempfile::tempdir().unwrap();
        let document = inventory("lib.rs");
        assert!(check_inventory(root.path(), &document).is_err());
        std::fs::write(root.path().join("lib.rs"), "").unwrap();
        assert_eq!(check_inventory(root.path(), &document).unwrap(), 1);
    }

    #[test]
    fn empty_pending_and_escaping_entries_cannot_pass() {
        let root = tempfile::tempdir().unwrap();
        assert!(check_inventory(root.path(), &json!({"entries": []})).is_err());
        assert!(check_inventory(root.path(), &inventory("../outside.rs")).is_err());
        let mut document = inventory("lib.rs");
        document["entries"][0]["status"] = json!("pending");
        assert!(check_inventory(root.path(), &document).is_err());
    }

    #[test]
    fn vendored_swift_is_excluded_but_owned_swift_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("third-party")).unwrap();
        std::fs::write(root.path().join("third-party/External.swift"), "").unwrap();
        assert!(check_sources(root.path()).is_ok());
        std::fs::write(root.path().join("Owned.swift"), "").unwrap();
        assert!(check_sources(root.path()).is_err());
    }
}
