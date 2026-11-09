//! Wasm application entry point.
//!
//! The browser host drives frames through its C ABI rather than entering a
//! blocking loop, so a declaration made with `cove_ui::application!` is published
//! here by a module initializer and pulled by the host when it builds its scene.
//! This mirrors Android, where `JNI_OnLoad` installs the factory the Activity
//! later reads. The first installation wins.

use std::sync::OnceLock;

static APPLICATION: OnceLock<fn() -> crate::ApplicationLaunch> = OnceLock::new();

/// Registers the complete declaration. `cove_ui::application!` calls this from a
/// `.init_array` constructor when its module is initialized.
pub fn install_application(factory: fn() -> crate::ApplicationLaunch) {
    let _ = APPLICATION.set(factory);
}

/// The registered declaration, or `None` when no application module ran yet.
pub fn application() -> Option<fn() -> crate::ApplicationLaunch> {
    APPLICATION.get().copied()
}