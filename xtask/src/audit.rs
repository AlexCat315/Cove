//! Checks the Rust workspace layout.

use crate::{Result, process};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) fn run(as_json: bool) -> Result<()> {
    let metadata: Value = serde_json::from_str(&process::output(process::cargo().args([
        "metadata",
        "--no-deps",
        "--locked",
        "--format-version",
        "1",
    ]))?)
    .map_err(|e| e.to_string())?;
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
            json!({"packages": packages.len(), "integration_test_targets": tests})
        );
    } else {
        println!(
            "{} packages, {tests} integration test targets",
            packages.len()
        );
    }
    Ok(())
}
