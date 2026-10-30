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

pub mod ui;
pub use ui::*;
