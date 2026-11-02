mod app_bridge;
mod frame_deadline;
mod frame_drive;
#[cfg(target_os = "ios")]
mod ios;
mod lifecycle;
mod log;
mod native_accessibility;
pub mod platform;
mod platform_host;
mod sdl3_platform_host;
mod surface_factory;
pub use app_bridge::*;
pub use cove_core as core;
pub use cove_core::scene;
pub use cove_graphics as graphics;
pub use cove_imaging as imaging;
pub use cove_shell as shell;
pub use cove_text as text;
pub use frame_deadline::FrameDeadline;
pub use frame_drive::PlatformFrameRateMode;
pub use native_accessibility::NativeAccessibility;
pub use platform_host::PlatformHost;
pub use sdl3_platform_host::{
    FrameOutcome, HostCallbacks, HostWorkSender, PlatformWindowSession, Sdl3PlatformHost,
    SessionCallbacks,
};
pub use surface_factory::SurfaceFactory;
pub struct CoveUIRuntime;
impl CoveUIRuntime {
    pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
}

pub use log::set_runtime_logger;
