//! Native application hosts for desktop, iOS and Android, and the Wasm entry.
#[cfg(not(target_arch = "wasm32"))]
pub mod android;
pub mod application;
pub mod application_services;
pub mod desktop;
pub mod wasm;

pub use application::{
    ApplicationCheckpoint, ApplicationLaunch, DiagnosticsCallbacks, DiagnosticsFactory,
    DiagnosticsSession,
};
pub use application_services::{http::TransportCapabilities, install_platform_services};
pub use cove_platform::WindowBackdrop;
#[cfg(not(target_arch = "wasm32"))]
pub use desktop::AppDisplayHandle;
pub use desktop::application::configuration::{ApplicationConfiguration, TitleBarDivider};
#[cfg(not(target_arch = "wasm32"))]
pub use desktop::application::runtime::{Application, ApplicationCallbacks};
pub use desktop::window::{
    NativeMenu, NativeMenuAction, NativeMenuBar, NativeMenuItem, NativeMenuKeyModifiers,
};
