//! Desktop application host: the public `Application` entry point, its window
//! rendering layer, display controls and diagnostics.
//!
//! The SDL3 shell in `cove-shell` owns windows and input; `cove-runtime` owns
//! the frame loop. This crate supplies the UI-specific assembly on top: the
//! composition tree, the text environment, presentation targets and the
//! application-facing control surface.

pub mod application;
pub mod diagnostics;
pub mod display;
pub mod rendering;
pub mod resources_support;
pub mod window;

pub use application::configuration::{
    AppFrameDrivePolicy, AppWindowRendering, AppWindowTitleBarStyle, ApplicationConfiguration,
};
pub use application::runtime::{Application, ApplicationCallbacks};
pub use display::{AppAuxiliaryWindowRequest, AppDisplayHandle};
