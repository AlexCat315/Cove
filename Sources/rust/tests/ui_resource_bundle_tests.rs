//! Translation of `Tests/CoveUITests/PackageResourceBundleTests.swift`.
//!
//! Behaviour checklist:
//! - the window chrome `close` icon ships in the target's packaged resources
//! - the packaged resource locator resolves it to a real, readable file
//!
//! Named differences from the Swift suite:
//! - SwiftPM resolved `Bundle.module` at run time; Rust resolves the same
//!   `{bundle}.bundle` directory beside the executable
//!   (`cove_platform::ResourceBundle`). Under `cargo test` no packaged bundle
//!   exists, so the locator rule is exercised against a bundle staged from the
//!   shipped asset directory. The production counterpart of the Swift
//!   `CoveUIResourceBundle` accessor lives in `Hosts/rust/desktop/
//!   resources_support`, which `cove-ui-addons` does not depend on.

use std::path::{Path, PathBuf};

use cove_platform::ResourceBundle;

/// The SwiftPM resource-bundle id of the `CoveUI` target; the Rust
/// `Hosts/rust/desktop/resources_support` uses the same identifier.
const COVE_UI_BUNDLE_NAME: &str = "CoveUI_CoveUI";

fn repo_resources_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../Hosts/Desktop/Sources/CoveUI/Resources")
}

/// Materialise the bundle a packaged app would carry beside its executable.
fn staged_bundle(temp_root: &Path) -> PathBuf {
    let source = repo_resources_dir().join("WindowChromeIcons");
    let staged = temp_root.join(format!("{COVE_UI_BUNDLE_NAME}.bundle"));
    std::fs::create_dir_all(staged.join("WindowChromeIcons")).expect("stage the bundle");
    std::fs::copy(
        source.join("close.svg"),
        staged.join("WindowChromeIcons/close.svg"),
    )
    .expect("the close icon ships with the target");
    staged
}

#[test]
fn window_chrome_icon_resolves_through_the_packaged_resource_locator() {
    let source = repo_resources_dir().join("WindowChromeIcons/close.svg");
    assert!(
        source.is_file(),
        "the packaged asset must exist at {}",
        source.display()
    );

    let temp_root = std::env::temp_dir().join(format!("cove-ui-resource-bundle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    staged_bundle(&temp_root);

    let bundle = ResourceBundle::locate(&[COVE_UI_BUNDLE_NAME], &[temp_root.clone()])
        .expect("the packaged resource locator finds the bundle");
    let resolved = bundle.root().join("WindowChromeIcons/close.svg");
    assert!(resolved.is_file(), "{COVE_UI_BUNDLE_NAME} resolves close.svg");
    let bytes = bundle
        .read(Path::new("WindowChromeIcons/close.svg"))
        .expect("the resolved icon is readable");
    assert!(
        String::from_utf8_lossy(&bytes).contains("<svg"),
        "the resolved icon is the chrome glyph, got {} bytes",
        bytes.len()
    );

    std::fs::remove_dir_all(&temp_root).expect("clean up the staged bundle");
}
