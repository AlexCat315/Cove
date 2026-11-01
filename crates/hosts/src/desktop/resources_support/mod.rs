//! Translation of Desktop/Sources/CoveUI/ResourcesSupport/Bundle.swift and
//! ResourcesSupport/Lookup.swift.
//!
//! SwiftPM resolved resources through `Bundle.module`; the Rust tree resolves
//! the same directories with [`cove_platform::ResourceBundle`], using the
//! identical rules on desktop and Android.

use cove_platform::ResourceBundle;
use std::io;

/// SwiftPM names the CoveUI target's resource bundle `CoveUI_CoveUI`.
pub const RESOURCE_BUNDLE_NAME: &str = "CoveUI_CoveUI";

/// Resolves SwiftPM resources using the same rules on desktop and Android.
pub struct ApplicationResources;

impl ApplicationResources {
    /// The bundle for an application-declared resource target, or the
    /// `Missing resource bundle` error the Swift `required` helper raised.
    pub fn required(named: &str) -> io::Result<ResourceBundle> {
        ResourceBundle::beside_executable(&[named])?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing resource bundle {named}"),
            )
        })
    }
}

/// The bundle CoveUI's own window-chrome icons ship in.
pub fn resource_bundle() -> io::Result<ResourceBundle> {
    ApplicationResources::required(RESOURCE_BUNDLE_NAME)
}

/// Path of one window-chrome icon inside [`RESOURCE_BUNDLE_NAME`], e.g.
/// `minimize`.
pub fn window_chrome_icon(name: &str) -> io::Result<std::path::PathBuf> {
    let bundle = resource_bundle()?;
    Ok(bundle
        .root()
        .join("WindowChromeIcons")
        .join(format!("{name}.svg")))
}
