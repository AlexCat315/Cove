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

/// The Vulkan backend loads the native driver at runtime.
#[cfg(not(target_arch = "wasm32"))]
pub mod vulkan;

/// The DX12 backend reaches the driver through COM vtable dispatch.
#[cfg(not(target_arch = "wasm32"))]
pub mod dx12;

#[cfg(target_arch = "wasm32")]
pub mod webgpu;

pub mod ui;
pub use ui::*;
