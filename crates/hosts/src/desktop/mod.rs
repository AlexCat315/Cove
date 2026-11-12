//! Desktop application host: the public `Application` entry point, its window
//! rendering layer, display controls and diagnostics.
//!
//! The SDL3 shell in `cove-shell` owns windows and input; `cove-runtime` owns
//! the frame loop. This crate supplies the UI-specific assembly on top: the
//! composition tree, the text environment, presentation targets and the
//! application-facing control surface.

pub mod application;
pub mod diagnostics;
#[cfg(not(target_arch = "wasm32"))]
pub mod display;
#[cfg(not(target_arch = "wasm32"))]
pub mod rendering;
#[cfg(not(target_arch = "wasm32"))]
pub mod resources_support;
pub mod window;

pub use application::configuration::{
    AppFrameDrivePolicy, AppWindowRendering, AppWindowTitleBarStyle, ApplicationConfiguration,
};
#[cfg(not(target_arch = "wasm32"))]
pub use application::runtime::{Application, ApplicationCallbacks};
#[cfg(not(target_arch = "wasm32"))]
pub use display::{AppAuxiliaryWindowRequest, AppDisplayHandle};
