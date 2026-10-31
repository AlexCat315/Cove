//! Panel icon resources and the workspace resource bundle.
//!
//! The document stores only the icon key string (it must stay `Codable`); the
//! host registers concrete image resources at startup. Unresolved keys fall
//! back to the text-pill rail button.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// Registry of host-supplied panel icons keyed by `WorkspacePanel.icon_asset_key`.
#[derive(Default)]
pub struct WorkspacePanelIconCatalog;

fn registry() -> &'static Mutex<BTreeMap<String, Vec<u8>>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, Vec<u8>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

impl WorkspacePanelIconCatalog {
    pub fn register(key: impl Into<String>, resource: impl Into<Vec<u8>>) {
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(key.into(), resource.into());
    }

    pub fn resolve(key: Option<&str>) -> Option<Vec<u8>> {
        let key = key?;
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)
            .cloned()
    }

    pub fn is_registered(key: &str) -> bool {
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains_key(key)
    }

    pub fn clear() {
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }
}

/// Bundle name the Swift target loads its resources from. The Rust build keeps
/// the same identifier so a packaged app finds the same directory.
pub const WORKSPACE_RESOURCE_BUNDLE_NAME: &str = "CoveUI_CoveUIWorkspace";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_keys_are_absent_not_panicking() {
        assert!(WorkspacePanelIconCatalog::resolve(None).is_none());
        assert!(WorkspacePanelIconCatalog::resolve(Some("missing")).is_none());
        WorkspacePanelIconCatalog::register("glyph", vec![1u8, 2, 3]);
        assert_eq!(
            WorkspacePanelIconCatalog::resolve(Some("glyph")),
            Some(vec![1, 2, 3])
        );
        WorkspacePanelIconCatalog::clear();
        assert!(!WorkspacePanelIconCatalog::is_registered("glyph"));
    }
}
