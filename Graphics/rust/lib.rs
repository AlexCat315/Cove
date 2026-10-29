//! NativeRHI: backend-neutral GPU abstraction (Metal, Vulkan, DirectX 12).
//!
//! The frontend `Device` owns handle allocation, binding/pipeline caches and
//! the submission planner; a concrete backend owns the GPU objects behind each
//! handle. Every backend failure is reported as a `RhiError` — nothing is
//! silently truncated.
pub mod rhi;

pub use rhi::*;
