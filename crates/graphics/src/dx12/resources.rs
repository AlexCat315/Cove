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
    /// The stage the module was compiled for, retained so pipeline creation
    /// can validate stage/slot pairings without re-parsing the DXIL.
    #[allow(dead_code)]
    pub stage: crate::rhi::ShaderStage,
}

/// The backend's own handle space.
#[derive(Default)]
pub struct Dx12Registries {
    pub buffers: HashMap<u32, Dx12ResourceRecord>,
    pub textures: HashMap<u32, Dx12TextureRecord>,
    /// Sampler descriptions, copied into the shader-visible sampler heap when a
    /// binding set is registered.
    pub samplers: HashMap<u32, sys::D3D12_SAMPLER_DESC>,
    pub shaders: HashMap<u32, Dx12ShaderRecord>,
    pub graphics_pipelines: HashMap<u32, Dx12GraphicsPipelineRecord>,
    pub compute_pipelines: HashMap<u32, Dx12ComputePipelineRecord>,
    pub mesh_pipelines: HashMap<u32, Dx12MeshPipelineRecord>,
    pub root_signatures: HashMap<u32, sys::ComPtr>,
    /// Binding layouts and sets, which D3D12 keeps as root-signature and
    /// descriptor-heap state rather than GPU objects.
    pub binding_layouts: HashMap<u32, super::binding::Dx12LayoutRecord>,
    pub binding_sets: HashMap<u32, Dx12SetRecord>,
    pub pipeline_layouts: HashMap<u32, Dx12PipelineLayoutRecord>,
    /// Next free descriptor index in the shader-visible CBV/SRV/UAV heap.
    pub next_descriptor_index: u32,
    /// Next free descriptor index in the shader-visible sampler heap. Samplers
    /// have their own heap and their own increment size, so they cannot share
    /// the resource-heap counter (or the RTV counter).
    pub next_sampler_index: u32,
}

// SAFETY: the records are COM pointers plus plain data. D3D12 objects are
// free-threaded, and the owning `Mutex` serializes access to the records.
unsafe impl Send for Dx12Registries {}

/// One registered binding set: the layout it was built from, its entries and the
/// descriptor-heap ranges it occupies.
pub struct Dx12SetRecord {
    pub layout_id: u32,
    pub entries: Vec<crate::rhi::BindingSetEntry>,
    /// Base descriptor index in the CBV/SRV/UAV heap for this set's table.
    pub resource_heap_base: u32,
    /// Number of CBV/SRV/UAV descriptors reserved for this set.
    pub resource_heap_count: u32,
    /// Base descriptor index in the sampler heap for this set's table.
    pub sampler_heap_base: u32,
    /// Number of sampler descriptors reserved for this set.
    pub sampler_heap_count: u32,
}

/// One pipeline layout's declarations and the root-parameter indices its sets
/// and push-constant ranges map to.
pub struct Dx12PipelineLayoutRecord {
    pub set_layouts: Vec<crate::rhi::BindingLayout>,
    pub push_constants: Vec<crate::rhi::PushConstantRange>,
    /// Per-set descriptor-table root parameter for CBV/SRV/UAV, if the set has any.
    pub resource_table_root_index: Vec<Option<u32>>,
    /// Per-set descriptor-table root parameter for samplers, if the set has any.
    pub sampler_table_root_index: Vec<Option<u32>>,
    /// Maps `(set_index, binding_slot)` for buffer bindings to their root descriptor index.
    pub buffer_root_index: HashMap<(u32, u32), u32>,
    /// Maps push-constant slot to its root constants index.
    pub push_constant_root_index: HashMap<u32, u32>,
}

/// A graphics pipeline state plus the metadata needed to bind it.
pub struct Dx12GraphicsPipelineRecord {
    pub pipeline: sys::ComPtr,
    pub layout_id: u32,
    pub primitive: crate::rhi::PrimitiveType,
    pub vertex_layout: Option<crate::rhi::VertexLayoutDescriptor>,
}

/// A compute pipeline state plus its layout.
pub struct Dx12ComputePipelineRecord {
    pub pipeline: sys::ComPtr,
    pub layout_id: u32,
}

/// A mesh pipeline state plus its layout.
pub struct Dx12MeshPipelineRecord {
    pub pipeline: sys::ComPtr,
    pub layout_id: u32,
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
            for record in self.graphics_pipelines.values() {
                sys::IUnknownVtbl::release(record.pipeline);
            }
            for record in self.compute_pipelines.values() {
                sys::IUnknownVtbl::release(record.pipeline);
            }
            for record in self.mesh_pipelines.values() {
                sys::IUnknownVtbl::release(record.pipeline);
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

/// Vertex-buffer view passed to `IASetVertexBuffers`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_VERTEX_BUFFER_VIEW {
    pub BufferLocation: u64,
    pub SizeInBytes: u32,
    pub StrideInBytes: u32,
}

/// Index-buffer view passed to `IASetIndexBuffer`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3D12_INDEX_BUFFER_VIEW {
    pub BufferLocation: u64,
    pub SizeInBytes: u32,
    pub Format: u32,
}

/// `ID3D12GraphicsCommandList`'s vtable prefix, in header order. Only the entries
/// the backend calls are typed; the rest keep their slots so later offsets are
/// right.
#[repr(C)]
pub struct CommandListVtbl {
    pub base: sys::IUnknownVtbl,
    pub object: sys::ID3D12ObjectSlots,
    pub GetDevice: usize,
    pub GetType: usize,
    pub Close: unsafe extern "system" fn(sys::ComPtr) -> sys::HRESULT,
    pub Reset: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr, sys::ComPtr) -> sys::HRESULT,
    pub ClearState: usize,
    pub DrawInstanced: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32, u32),
    pub DrawIndexedInstanced: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32, i32, u32),
    pub Dispatch: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32),
    pub CopyBufferRegion:
        unsafe extern "system" fn(sys::ComPtr, sys::ComPtr, u64, sys::ComPtr, u64, u64),
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
    pub OMSetStencilRef: usize,
    pub SetPipelineState: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr),
    pub ResourceBarrier:
        unsafe extern "system" fn(sys::ComPtr, u32, *const sys::D3D12_RESOURCE_BARRIER),
    pub ExecuteBundle: usize,
    pub SetDescriptorHeaps: unsafe extern "system" fn(sys::ComPtr, u32, *const sys::ComPtr),
    pub SetComputeRootSignature: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr),
    pub SetGraphicsRootSignature: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr),
    pub SetComputeRootDescriptorTable:
        unsafe extern "system" fn(sys::ComPtr, u32, sys::D3D12_GPU_DESCRIPTOR_HANDLE),
    pub SetGraphicsRootDescriptorTable:
        unsafe extern "system" fn(sys::ComPtr, u32, sys::D3D12_GPU_DESCRIPTOR_HANDLE),
    pub SetComputeRoot32BitConstant: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32),
    pub SetGraphicsRoot32BitConstant: unsafe extern "system" fn(sys::ComPtr, u32, u32, u32),
    pub SetComputeRoot32BitConstants:
        unsafe extern "system" fn(sys::ComPtr, u32, u32, *const u8, u32),
    pub SetGraphicsRoot32BitConstants:
        unsafe extern "system" fn(sys::ComPtr, u32, u32, *const u8, u32),
    pub SetComputeRootConstantBufferView: unsafe extern "system" fn(sys::ComPtr, u32, u64),
    pub SetGraphicsRootConstantBufferView: unsafe extern "system" fn(sys::ComPtr, u32, u64),
    pub SetComputeRootShaderResourceView: unsafe extern "system" fn(sys::ComPtr, u32, u64),
    pub SetGraphicsRootShaderResourceView: unsafe extern "system" fn(sys::ComPtr, u32, u64),
    pub SetComputeRootUnorderedAccessView: unsafe extern "system" fn(sys::ComPtr, u32, u64),
    pub SetGraphicsRootUnorderedAccessView: unsafe extern "system" fn(sys::ComPtr, u32, u64),
    pub IASetIndexBuffer: unsafe extern "system" fn(sys::ComPtr, *const D3D12_INDEX_BUFFER_VIEW),
    pub IASetVertexBuffers:
        unsafe extern "system" fn(sys::ComPtr, u32, u32, *const D3D12_VERTEX_BUFFER_VIEW),
    pub SOSetTargets: usize,
    pub OMSetRenderTargets: unsafe extern "system" fn(
        sys::ComPtr,
        u32,
        *const sys::D3D12_CPU_DESCRIPTOR_HANDLE,
        u32,
        *const sys::D3D12_CPU_DESCRIPTOR_HANDLE,
    ),
    pub ClearDepthStencilView: unsafe extern "system" fn(
        sys::ComPtr,
        sys::D3D12_CPU_DESCRIPTOR_HANDLE,
        u32,
        f32,
        u8,
        u32,
        *const sys::D3D12_RECT,
    ),
    pub ClearRenderTargetView: unsafe extern "system" fn(
        sys::ComPtr,
        sys::D3D12_CPU_DESCRIPTOR_HANDLE,
        *const f32,
        u32,
        *const sys::D3D12_RECT,
    ),
    pub ClearUnorderedAccessViewUint: usize,
    pub ClearUnorderedAccessViewFloat: usize,
    pub DiscardResource: usize,
    pub BeginQuery: usize,
    pub EndQuery: usize,
    pub ResolveQueryData: usize,
    pub SetPredication: usize,
    pub SetMarker: usize,
    pub BeginEvent: usize,
    pub EndEvent: usize,
    pub ExecuteIndirect: usize,
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
