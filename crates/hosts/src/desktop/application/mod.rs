//! Translation of Desktop/Sources/CoveUI/Application.

#[cfg(not(target_arch = "wasm32"))]
pub mod composition;
pub mod configuration;
#[cfg(not(target_arch = "wasm32"))]
pub mod memory;
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime;
#[cfg(not(target_arch = "wasm32"))]
pub mod services;
pub mod typography;

#[cfg(not(target_arch = "wasm32"))]
pub use composition::MainComposition;
pub use configuration::{
    AppFrameDrivePolicy, AppWindowRendering, AppWindowTitleBarStyle, ApplicationConfiguration,
};
#[cfg(not(target_arch = "wasm32"))]
pub use memory::{AppMemorySnapshot, AppWindowMemorySnapshot, AppWindowRendererState};
#[cfg(not(target_arch = "wasm32"))]
pub use runtime::{Application, ApplicationCallbacks};
#[cfg(not(target_arch = "wasm32"))]
pub use services::ApplicationServices;
pub use typography::{AdaptiveApplicationContent, ApplicationTypography};
