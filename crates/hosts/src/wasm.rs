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

/// Installs one browser surface's overlay services before its graph mounts.
/// Call once per input context, not when a frame or surface is recreated.
pub fn install_overlay_services(context: &cove_core::scene::PlatformInputContext) {
    use cove_ui_compose::primitives::overlays::TooltipArbitrationAmbient;
    use cove_ui_compose::primitives::overlays::portal::{PortalStore, PortalStoreAmbient};
    use std::rc::Rc;

    context.add_scoped_ambient(Rc::new(PortalStoreAmbient::new(Rc::new(PortalStore::new()))));
    context.add_scoped_ambient(Rc::new(TooltipArbitrationAmbient::new()));
}
