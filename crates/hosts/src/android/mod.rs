//! Android host: platform contract types, the integration scene session and the
//! JNI-facing application built on top of them.

pub mod contract;
pub mod integration;
#[cfg(target_os = "android")]
pub mod jni;
pub mod probe;
pub mod sources;

use cove_composition::AnyView;
use std::sync::OnceLock;

static APPLICATION_CONTENT: OnceLock<fn() -> AnyView> = OnceLock::new();
static APPLICATION: OnceLock<fn() -> crate::ApplicationLaunch> = OnceLock::new();

/// Registers the complete declaration so an Activity receives the same
/// configuration and checkpoint hooks as the desktop/iOS entry point.
pub fn install_application(factory: fn() -> crate::ApplicationLaunch) {
    let _ = APPLICATION.set(factory);
}

pub(crate) fn application() -> Option<fn() -> crate::ApplicationLaunch> {
    APPLICATION.get().copied()
}

/// Installs the root view that `SceneLaunchMode::Application` hosts. The first
/// installation wins; `cove_ui::application!` calls this from `JNI_OnLoad`.
pub fn install_application_content(factory: fn() -> AnyView) {
    let _ = APPLICATION_CONTENT.set(factory);
}

pub(crate) fn application_content() -> Option<fn() -> AnyView> {
    APPLICATION_CONTENT.get().copied()
}
