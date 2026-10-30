//! Translation of CoveUIComposeResourceBundle.swift.
use cove_platform::ResourceBundle;
use std::sync::OnceLock;

/// Lazily resolved Compose resources for both development and packaged apps.
pub struct CoveUIComposeResourceBundle;

impl CoveUIComposeResourceBundle {
    pub fn bundle() -> &'static ResourceBundle {
        static BUNDLE: OnceLock<ResourceBundle> = OnceLock::new();
        BUNDLE.get_or_init(|| {
            ResourceBundle::beside_executable(&["CoveUICompose_CoveUICompose", "CoveUICompose"])
                .expect("failed to inspect Compose resource bundle roots")
                .expect("could not locate Compose resource bundle")
        })
    }
}
