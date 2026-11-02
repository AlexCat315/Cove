//! Bundled resources. Packaged builds stage `Assets/` into `AppContent.resources`;
//! `cargo run` reads them from the source tree.

use cove_ui::platform::ResourceBundle;
use std::path::Path;
use std::sync::OnceLock;

fn bundle() -> Option<&'static ResourceBundle> {
    static BUNDLE: OnceLock<Option<ResourceBundle>> = OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            ResourceBundle::beside_executable(&["AppContent"])
                .ok()
                .flatten()
        })
        .as_ref()
}

pub fn read(relative: &str) -> std::io::Result<Vec<u8>> {
    let relative = Path::new("Assets").join(relative);
    match bundle() {
        Some(bundle) => bundle.read(&relative),
        None => std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)),
    }
}

pub fn welcome() -> String {
    let bytes = read("welcome.txt").expect("missing Assets/welcome.txt");
    String::from_utf8(bytes)
        .expect("Assets/welcome.txt must be UTF-8")
        .trim()
        .to_string()
}
