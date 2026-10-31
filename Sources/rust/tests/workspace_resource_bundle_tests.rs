//! Translation of `Tests/CoveUIWorkspaceTests/PackageResourceBundleTests.swift`.
//!
//! Behaviour checklist:
//! - the workspace `collapse` icon ships in the target's packaged resources
//! - the packaged resource locator resolves it to a real, readable file
//! - the Rust crate keeps the Swift bundle identifier
//!
//! Named differences from the Swift suite:
//! - SwiftPM resolved `Bundle.module` at run time; Rust resolves the same
//!   `{bundle}.bundle` directory beside the executable
//!   (`cove_platform::ResourceBundle`). Under `cargo test` no packaged bundle
//!   exists, so the locator rule is exercised against a bundle staged from the
//!   shipped `Sources/CoveUIWorkspace/Resources` directory.

use std::path::{Path, PathBuf};

use cove_platform::ResourceBundle;
use cove_ui_addons::workspace::WORKSPACE_RESOURCE_BUNDLE_NAME;

fn repo_resources_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("CoveUIWorkspace/Resources")
}

#[test]
fn workspace_icon_resolves_through_the_packaged_resource_locator() {
    assert_eq!(
        WORKSPACE_RESOURCE_BUNDLE_NAME, "CoveUI_CoveUIWorkspace",
        "the Rust crate ships under the Swift bundle identifier"
    );

    let source = repo_resources_dir().join("WorkspaceIcons/collapse.svg");
    assert!(
        source.is_file(),
        "the packaged asset must exist at {}",
        source.display()
    );

    let temp_root = std::env::temp_dir().join(format!(
        "cove-workspace-resource-bundle-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_root);
    let staged = temp_root.join(format!("{WORKSPACE_RESOURCE_BUNDLE_NAME}.bundle"));
    std::fs::create_dir_all(staged.join("WorkspaceIcons")).expect("stage the bundle");
    std::fs::copy(&source, staged.join("WorkspaceIcons/collapse.svg"))
        .expect("the collapse icon ships with the target");

    let bundle = ResourceBundle::locate(&[WORKSPACE_RESOURCE_BUNDLE_NAME], &[temp_root.clone()])
        .expect("the packaged resource locator finds the bundle");
    assert!(
        bundle
            .root()
            .join("WorkspaceIcons/collapse.svg")
            .is_file(),
        "{WORKSPACE_RESOURCE_BUNDLE_NAME} resolves collapse.svg"
    );
    let bytes = bundle
        .read(Path::new("WorkspaceIcons/collapse.svg"))
        .expect("the resolved icon is readable");
    assert!(
        String::from_utf8_lossy(&bytes).contains("<svg"),
        "the resolved icon is the workspace glyph, got {} bytes",
        bytes.len()
    );

    std::fs::remove_dir_all(&temp_root).expect("clean up the staged bundle");
}
