//! Android host: platform contract types, the integration scene session and the
//! JNI-facing application built on top of them.

pub mod contract;
pub mod integration;
#[cfg(target_os = "android")]
pub mod jni;
pub mod probe;
pub mod sources;

use crate::android::contract::SceneLaunchMode;
use crate::android::sources::cove_android_app::content::scene::HostedSceneContent;
use cove_composition::AnyView;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

static APPLICATION_CONTENT: OnceLock<fn() -> AnyView> = OnceLock::new();
static APPLICATION: OnceLock<fn() -> crate::ApplicationLaunch> = OnceLock::new();
/// Content installed for one launch mode. Storing a function pointer keeps the
/// registry independent of the trait object's thread affiliation: the content
/// itself is built per Activity on the UI thread.
pub type SceneContentFactory = fn() -> Box<dyn HostedSceneContent>;
static SCENE_CONTENT: OnceLock<Mutex<BTreeMap<SceneLaunchMode, SceneContentFactory>>> =
    OnceLock::new();

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

/// Installs content for one launch mode, so a game or application crate can
/// supply its own scene without replacing the shared host seam. The first
/// installation for a mode wins; the mode keeps `UnlinkedContent` until then.
pub fn install_scene_content(mode: SceneLaunchMode, factory: SceneContentFactory) {
    let registry = SCENE_CONTENT.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut registry = registry.lock().unwrap_or_else(|error| error.into_inner());
    registry.entry(mode).or_insert(factory);
}

pub(crate) fn scene_content(mode: SceneLaunchMode) -> Option<SceneContentFactory> {
    let registry = SCENE_CONTENT.get()?;
    let registry = registry.lock().unwrap_or_else(|error| error.into_inner());
    registry.get(&mode).copied()
}
