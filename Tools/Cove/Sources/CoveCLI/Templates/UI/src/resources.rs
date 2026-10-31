//! Bundled resource lookup, matching `AppResources.swift`.
//!
//! The same lookup serves desktop, Android debug and Android release builds, so
//! the keys never change across packaging modes.

use cove_ui::platform::ResourceBundle;
use std::sync::OnceLock;

const BUNDLE_NAMES: [&str; 2] = ["CoveApplication_AppContent", "AppContent"];

fn bundle() -> &'static ResourceBundle {
    static BUNDLE: OnceLock<ResourceBundle> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        ResourceBundle::beside_executable(&BUNDLE_NAMES)
            .expect("failed to inspect the application resource bundle")
            .expect("could not locate the application resource bundle")
    })
}

/// The bundled welcome line, trimmed the way the Swift template trims it.
pub fn welcome() -> String {
    let bytes = bundle()
        .read(std::path::Path::new("Assets/welcome.txt"))
        .expect("missing bundled Assets/welcome.txt");
    String::from_utf8(bytes)
        .expect("Assets/welcome.txt must be UTF-8")
        .trim()
        .to_string()
}

pub fn format_count(count: i64) -> String {
    format!("Count: {count}")
}
