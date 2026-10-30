//! NativeRHI: backend-neutral GPU abstraction (Metal, Vulkan, DirectX 12).
//!
//! The frontend `Device` owns handle allocation, binding/pipeline caches and
//! the submission planner; a concrete backend owns the GPU objects behind each
//! handle. Every backend failure is reported as a `RhiError` — nothing is
//! silently truncated.
pub mod rhi;

pub use rhi::*;

/// The Metal backend only exists where the Metal framework does.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod metal;

/// The Vulkan backend reaches the driver through a runtime-loaded loader, so it
/// is compiled on every host: only its registration is limited to the platforms
/// that ship a Vulkan driver.
pub mod vulkan;

pub mod ui;
pub use ui::*;
