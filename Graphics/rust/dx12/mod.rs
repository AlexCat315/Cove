//! Translation of NativeRHI/DX12: the concrete Direct3D 12 backend.
//!
//! `sys` is a self-contained COM FFI layer (vtables plus the D3D12/DXGI structs
//! the backend needs), so no `windows` crate is required. The module compiles on
//! every host; only the loader and surface creation are Windows-gated, which keeps
//! the crate type-checking on macOS while the real path stays Windows-only.
pub(crate) mod sys;
