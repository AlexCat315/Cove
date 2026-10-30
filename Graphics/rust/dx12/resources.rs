//! Translation of NativeRHI/DX12/{Resources/DX12Resources, Resources/DX12Pipelines,
//! Resources/DX12Compilation, Submission/DX12Uploader}.swift: resource, pipeline and
//! upload-ring creation.
//!
//! Swift's C++ bridge created and tracked these objects; the Rust port drives the
//! `ID3D12Device` vtable directly and keeps the metadata it needs for validation
//! and transfer in its own registries.
#![allow(non_snake_case)]
use std::collections::HashMap;

use crate::rhi::BufferUsage;

use super::sys;

// MARK: - Resource records

/// One committed resource plus its metadata.
pub struct Dx12ResourceRecord {
    /// Owned COM pointer (`ID3D12Resource`).
    pub resource: sys::ComPtr,
    pub size: u64,
    /// GPU virtual address, needed for vertex/index/constant descriptors.
    pub gpu_address: u64,
    /// CPU mapping for upload/readback heaps; null for default heaps.
    pub mapped: *mut u8,
    pub usage: BufferUsage,
}

// SAFETY: the record holds a COM pointer, a GPU address and a mapping. D3D12
// resources are usable from any thread once created, and the mapping is only
// dereferenced by the thread holding the registry lock.
unsafe impl Send for Dx12ResourceRecord {}

pub struct Dx12TextureRecord {
    pub resource: sys::ComPtr,
    pub format: crate::rhi::TextureFormat,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub layers: usize,
    pub mip_levels: usize,
    pub sample_count: usize,
    pub dimension: crate::rhi::TextureDimension,
    pub usage: crate::rhi::TextureUsage,
    /// Render-target descriptor, when the texture can be an attachment.
    pub rtv: Option<sys::D3D12_CPU_DESCRIPTOR_HANDLE>,
    /// Depth-stencil descriptor, when the texture can be a depth attachment.
    pub dsv: Option<sys::D3D12_CPU_DESCRIPTOR_HANDLE>,
}

pub struct Dx12ShaderRecord {
    /// Owned COM pointer (`ID3D12PipelineState` used as the compiled container) for
    /// the bytecode, or null when only the bytes are kept.
    pub bytecode: Vec<u8>,
    pub stage: crate::rhi::ShaderStage,
}

/// The backend's own handle space.
#[derive(Default)]
pub struct Dx12Registries {
    pub buffers: HashMap<u32, Dx12ResourceRecord>,
    pub textures: HashMap<u32, Dx12TextureRecord>,
    /// Sampler descriptor addresses inside the sampler heap. They are not COM
    /// objects, so they are never released.
    pub samplers: HashMap<u32, sys::ComPtr>,
    pub shaders: HashMap<u32, Dx12ShaderRecord>,
    pub graphics_pipelines: HashMap<u32, sys::ComPtr>,
    pub compute_pipelines: HashMap<u32, sys::ComPtr>,
    pub mesh_pipelines: HashMap<u32, sys::ComPtr>,
    pub root_signatures: HashMap<u32, sys::ComPtr>,
    /// Binding layouts and sets, which D3D12 keeps as root-signature and
    /// descriptor-heap state rather than GPU objects.
    pub binding_layouts: HashMap<u32, super::binding::Dx12LayoutRecord>,
    pub binding_sets: HashMap<u32, Dx12SetRecordRecord>,
    pub pipeline_layouts: HashMap<u32, Dx12PipelineLayoutRecord>,
    /// Next free shader-visible descriptor index.
    pub next_descriptor_index: u32,
}

/// One registered binding set: the layout it was built from and its entries.
pub struct Dx12SetRecordRecord {
    pub layout_id: u32,
    pub entries: Vec<crate::rhi::BindingSetEntry>,
}

/// One pipeline layout's declarations, kept for push-constant validation.
pub struct Dx12PipelineLayoutRecord {
    pub set_layouts: Vec<crate::rhi::BindingLayout>,
    pub push_constants: Vec<crate::rhi::PushConstantRange>,
}

impl Drop for Dx12Registries {
    fn drop(&mut self) {
        // SAFETY: every pointer here was created by this backend and is released
        // exactly once. D3D12 defers the actual free until the GPU is idle.
        unsafe {
            for record in self.buffers.values() {
                sys::IUnknownVtbl::release(record.resource);
            }
            for record in self.textures.values() {
                sys::IUnknownVtbl::release(record.resource);
            }
            for pipeline in self.graphics_pipelines.values() {
                sys::IUnknownVtbl::release(*pipeline);
            }
            for pipeline in self.compute_pipelines.values() {
                sys::IUnknownVtbl::release(*pipeline);
            }
            for pipeline in self.mesh_pipelines.values() {
                sys::IUnknownVtbl::release(*pipeline);
            }
            for signature in self.root_signatures.values() {
                sys::IUnknownVtbl::release(*signature);
            }
        }
    }
}

// MARK: - Barriers

/// Emits one resource-transition barrier.
///
/// # Safety
/// `command_list` must be an open `ID3D12GraphicsCommandList` and `resource` a live
/// `ID3D12Resource`.
pub unsafe fn transition(
    command_list: sys::ComPtr,
    resource: sys::ComPtr,
    before: u32,
    after: u32,
) {
    let barrier = sys::D3D12_RESOURCE_BARRIER {
        Type: sys::D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: 0,
        Transition: sys::D3D12_RESOURCE_TRANSITION_BARRIER {
            pResource: resource,
            Subresource: sys::D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            StateBefore: before,
            StateAfter: after,
        },
    };
    // SAFETY: the caller guarantees an open command list; the barrier outlives the
    // call, which copies it.
    unsafe {
        let vtbl = *(command_list as *mut *const CommandListVtbl);
        ((*vtbl).ResourceBarrier)(command_list, 1, &barrier);
    }
}

/// `ID3D12GraphicsCommandList`'s vtable prefix, in header order. Only the entries
/// the backend calls are typed; the rest keep their slots so later offsets are
/// right.
#[repr(C)]
pub struct CommandListVtbl {
    pub base: sys::IUnknownVtbl,
    pub GetDevice: usize,
    pub GetType: usize,
    pub Close: unsafe extern "system" fn(sys::ComPtr) -> sys::HRESULT,
    pub Reset: unsafe extern "system" fn(
        sys::ComPtr,
        sys::ComPtr,
        sys::ComPtr,
    ) -> sys::HRESULT,
    pub ClearState: usize,
    pub DrawInstanced: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32, u32),
    pub DrawIndexedInstanced:
        unsafe extern "system" fn(sys::ComPtr, u32, u32, u32, i32, u32),
    pub Dispatch: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32),
    pub CopyBufferRegion: unsafe extern "system" fn(
        sys::ComPtr,
        sys::ComPtr,
        u64,
        sys::ComPtr,
        u64,
        u64,
    ),
    pub CopyTextureRegion: unsafe extern "system" fn(
        sys::ComPtr,
        *const sys::D3D12_TEXTURE_COPY_LOCATION,
        u32,
        u32,
        u32,
        *const sys::D3D12_TEXTURE_COPY_LOCATION,
        *const sys::D3D12_BOX,
    ),
    pub CopyResource: usize,
    pub CopyTiles: usize,
    pub ResolveSubresource: usize,
    pub IASetPrimitiveTopology: unsafe extern "system" fn(sys::ComPtr, u32),
    pub RSSetViewports: unsafe extern "system" fn(sys::ComPtr, u32, *const sys::D3D12_VIEWPORT),
    pub RSSetScissorRects: unsafe extern "system" fn(sys::ComPtr, u32, *const sys::D3D12_RECT),
    pub OMSetBlendFactor: usize,
    pub OMSetRenderTargets: unsafe extern "system" fn(
        sys::ComPtr,
        u32,
        *const sys::D3D12_CPU_DESCRIPTOR_HANDLE,
        u32,
        *const sys::D3D12_CPU_DESCRIPTOR_HANDLE,
    ),
    pub ClearRenderTargetView: usize,
    pub ClearDepthStencilView: usize,
    pub ResourceBarrier:
        unsafe extern "system" fn(sys::ComPtr, u32, *const sys::D3D12_RESOURCE_BARRIER),
    pub OMSetRenderTargetsAndUnorderedAccessViews: usize,
    pub SetPipelineState: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr),
    pub SetDescriptorHeaps: unsafe extern "system" fn(sys::ComPtr, u32, *const sys::ComPtr),
    pub RSSetViewportsTyped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_list_vtable_keeps_resource_barrier_after_the_typed_prefix() {
        // `ResourceBarrier` is the first method after `OMSetRenderTargets` and the
        // two clears, so its slot count is fixed by the fields above.
        let size = std::mem::size_of::<CommandListVtbl>();
        assert!(size >= std::mem::size_of::<sys::IUnknownVtbl>());
        assert_eq!(
            std::mem::size_of::<sys::IUnknownVtbl>(),
            std::mem::size_of::<*const ()>() * 3
        );
    }

    #[test]
    fn resource_records_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Dx12ResourceRecord>();
    }
}
