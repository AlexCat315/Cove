//! Translation of NativeRHI/DX12: the concrete Direct3D 12 backend.
//!
//! `sys` is a self-contained COM FFI layer (vtables plus the D3D12/DXGI structs
//! the backend needs), so no `windows` crate is required. The module compiles on
//! every host; only the loader and surface creation are Windows-gated, which keeps
//! the crate type-checking on macOS while the real path stays Windows-only.
// Off Windows the backend is never registered, so the Windows-only call paths
// leave most of the module unreachable.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) mod backend;
pub(crate) mod binding;
pub(crate) mod buffer;
pub(crate) mod command_list;
pub(crate) mod device;
pub(crate) mod formats;
pub(crate) mod pipeline;
pub(crate) mod resources;
pub(crate) mod swapchain;
pub(crate) mod sys;
pub(crate) mod texture;

pub use backend::{Dx12Backend, make_backend};
