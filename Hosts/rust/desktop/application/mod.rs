//! Translation of Desktop/Sources/CoveUI/Application.

pub mod composition;
pub mod configuration;
pub mod memory;
pub mod runtime;
pub mod services;
pub mod typography;

pub use composition::MainComposition;
pub use configuration::{
    AppFrameDrivePolicy, AppWindowRendering, AppWindowTitleBarStyle, ApplicationConfiguration,
};
pub use memory::{AppMemorySnapshot, AppWindowMemorySnapshot, AppWindowRendererState};
pub use runtime::{Application, ApplicationCallbacks};
pub use services::ApplicationServices;
pub use typography::{AdaptiveApplicationContent, ApplicationTypography};
