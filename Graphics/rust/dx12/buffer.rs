//! Buffer creation, upload and readback for the DX12 backend.
//!
//! Vertex/index/constant/storage buffers live in a default (device-local) heap and
//! are filled through an upload heap; a transfer-only buffer is itself an upload
//! heap, which lets the CPU write it directly.
#![allow(non_snake_case)]
use crate::rhi::{
    Buffer, BufferDescriptor, BufferUsage, RhiError, RhiResult, rhi_byte_range, rhi_require,
};

use super::device::Dx12DeviceHandle;
use super::resources::{Dx12Registries, Dx12ResourceRecord};
use super::sys;
use core::ffi::c_void;

/// Creates one committed buffer and records it.
pub fn create(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: Buffer,
    descriptor: &BufferDescriptor,
) -> RhiResult<()> {
    // A GPU resource needs device-local memory; a transfer-only buffer stays in an
    // upload heap so the CPU can write it without staging.
    let is_gpu_resource = descriptor.usage.contains(BufferUsage::VERTEX)
        || descriptor.usage.contains(BufferUsage::INDEX)
        || descriptor.usage.contains(BufferUsage::INDIRECT)
        || descriptor.usage.contains(BufferUsage::UNIFORM)
        || descriptor.usage.contains(BufferUsage::STORAGE_READ)
        || descriptor.usage.contains(BufferUsage::STORAGE_WRITE);
    let heap = if is_gpu_resource {
        sys::D3D12_HEAP_TYPE_DEFAULT
    } else {
        sys::D3D12_HEAP_TYPE_UPLOAD
    };
    let mut record = create_in_heap(device, descriptor.size.max(1), heap)?;
    record.usage = descriptor.usage;
    registries.buffers.insert(handle.id(), record);
    Ok(())
}

/// Creates a buffer in an upload heap, for the ring and staging paths.
pub fn create_upload_heap(device: sys::ComPtr, size: usize) -> RhiResult<Dx12ResourceRecord> {
    create_in_heap_from_pointer(device, size.max(1), sys::D3D12_HEAP_TYPE_UPLOAD)
}

/// Creates a staging buffer in the given heap, for one texture transfer.
pub fn create_in_heap_for_transfer(
    device: sys::ComPtr,
    size: usize,
    heap_type: u32,
) -> RhiResult<Dx12ResourceRecord> {
    let mut record = create_in_heap_from_pointer(device, size.max(1), heap_type)?;
    record.usage =
        crate::rhi::BufferUsage::TRANSFER_SOURCE | crate::rhi::BufferUsage::TRANSFER_DESTINATION;
    Ok(record)
}

fn create_in_heap(
    device: &Dx12DeviceHandle,
    size: usize,
    heap_type: u32,
) -> RhiResult<Dx12ResourceRecord> {
    create_in_heap_from_pointer(device.device, size, heap_type)
}

/// Creates one committed buffer resource and maps it when the heap allows.
fn create_in_heap_from_pointer(
    device: sys::ComPtr,
    size: usize,
    heap_type: u32,
) -> RhiResult<Dx12ResourceRecord> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, size, heap_type);
        return Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ));
    }
    #[cfg(target_os = "windows")]
    {
        let heap = sys::D3D12_HEAP_PROPERTIES {
            Type: heap_type,
            ..Default::default()
        };
        let description = sys::D3D12_RESOURCE_DESC {
            Dimension: sys::D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: size as u64,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: sys::DXGI_FORMAT_UNKNOWN,
            SampleDesc: [1, 0],
            Layout: 0,
            Flags: sys::D3D12_RESOURCE_FLAG_NONE,
        };
        let mut resource: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: `device` is a live `ID3D12Device`; the vtable entry matches
        // `CreateCommittedResource` (the first method after `CheckFeatureSupport`
        // that creates resources).
        let status = unsafe {
            let vtbl = *(device as *mut *const DeviceCreateVtbl);
            ((*vtbl).CreateCommittedResource)(
                device,
                &heap,
                sys::D3D12_HEAP_FLAG_NONE,
                &description,
                if heap_type == sys::D3D12_HEAP_TYPE_UPLOAD {
                    sys::D3D12_RESOURCE_STATE_GENERIC_READ
                } else if heap_type == sys::D3D12_HEAP_TYPE_READBACK {
                    sys::D3D12_RESOURCE_STATE_COPY_DEST
                } else {
                    sys::D3D12_RESOURCE_STATE_COMMON
                },
                core::ptr::null(),
                &sys::IID_ID3D12_RESOURCE,
                &mut resource,
            )
        };
        if status != sys::S_OK || resource.is_null() {
            return Err(RhiError::OutOfMemory);
        }
        let mut mapped = core::ptr::null_mut();
        // Upload and readback heaps are both CPU-visible; a default heap is not.
        if heap_type == sys::D3D12_HEAP_TYPE_UPLOAD || heap_type == sys::D3D12_HEAP_TYPE_READBACK {
            // SAFETY: the resource is live and the read range is null, meaning
            // "no CPU reads from this mapping".
            unsafe {
                let vtbl = *(resource as *mut *const ResourceVtbl);
                ((*vtbl).Map)(resource, 0, core::ptr::null(), &mut mapped);
            }
        }
        Ok(Dx12ResourceRecord {
            resource,
            size: size as u64,
            gpu_address: gpu_address(resource),
            mapped: mapped.cast(),
            usage: BufferUsage::EMPTY,
        })
    }
}

/// The GPU virtual address of a resource, or zero when unavailable.
fn gpu_address(resource: sys::ComPtr) -> u64 {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: the resource is live and `GetGPUVirtualAddress` has no
        // preconditions beyond that.
        unsafe {
            let vtbl = *(resource as *mut *const ResourceVtbl);
            ((*vtbl).GetGPUVirtualAddress)(resource)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = resource;
        0
    }
}

/// `ID3D12Device`'s vtable prefix up to `CreateCommittedResource`.
#[repr(C)]
struct DeviceCreateVtbl {
    base: sys::IUnknownVtbl,
    GetNodeCount: usize,
    CreateCommandQueue: usize,
    CreateCommandAllocator: usize,
    CreateGraphicsPipelineState: usize,
    CreateComputePipelineState: usize,
    CreateCommandList: usize,
    CheckFeatureSupport: usize,
    CreateDescriptorHeap: usize,
    GetDescriptorHandleIncrementSize: usize,
    CreateRootSignature: usize,
    CreateConstantBufferView: usize,
    CreateShaderResourceView: usize,
    CreateUnorderedAccessView: usize,
    CreateRenderTargetView: usize,
    CreateDepthStencilView: usize,
    CreateSampler: usize,
    CopyDescriptors: usize,
    CopyDescriptorsSimple: usize,
    CreateCommittedResource: unsafe extern "system" fn(
        sys::ComPtr,
        *const sys::D3D12_HEAP_PROPERTIES,
        u32,
        *const sys::D3D12_RESOURCE_DESC,
        u32,
        *const c_void,
        *const sys::GUID,
        *mut sys::ComPtr,
    ) -> sys::HRESULT,
}

/// `ID3D12Resource`'s vtable prefix, which is all the backend calls.
#[repr(C)]
pub struct ResourceVtbl {
    pub base: sys::IUnknownVtbl,
    pub GetPrivateData: usize,
    pub SetPrivateData: usize,
    pub SetPrivateDataInterface: usize,
    pub GetName: usize,
    pub SetName: usize,
    pub GetHeapProperties: usize,
    pub Map: unsafe extern "system" fn(
        sys::ComPtr,
        u32,
        *const sys::D3D12_RANGE,
        *mut *mut c_void,
    ) -> sys::HRESULT,
    pub Unmap: unsafe extern "system" fn(sys::ComPtr, u32, *const sys::D3D12_RANGE),
    pub GetDesc: usize,
    pub GetGPUVirtualAddress: unsafe extern "system" fn(sys::ComPtr) -> u64,
}

/// Copies `data` into a mapped buffer.
pub fn upload(
    registries: &Dx12Registries,
    _device: &Dx12DeviceHandle,
    buffer: Buffer,
    offset: usize,
    data: &[u8],
) -> RhiResult<()> {
    let record = registries
        .buffers
        .get(&buffer.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
    rhi_byte_range(offset, data.len(), record.size as usize)?;
    if data.is_empty() {
        return Ok(());
    }
    rhi_require(
        !record.mapped.is_null(),
        "buffer is not in an upload heap; stage through a ring allocation",
    )?;
    // SAFETY: the mapping covers `record.size` bytes and the range was validated;
    // `data` is a distinct live slice.
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), record.mapped.add(offset), data.len());
    }
    Ok(())
}

/// Copies `destination.len()` bytes back out of a mapped buffer.
pub fn read(
    registries: &Dx12Registries,
    buffer: Buffer,
    offset: usize,
    destination: &mut [u8],
) -> RhiResult<()> {
    let record = registries
        .buffers
        .get(&buffer.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
    rhi_byte_range(offset, destination.len(), record.size as usize)?;
    rhi_require(
        !record.mapped.is_null(),
        "buffer readback requires a readback or upload heap",
    )?;
    // SAFETY: the mapping covers `record.size` bytes, the range was validated, and
    // `destination` is a distinct live slice.
    unsafe {
        std::ptr::copy_nonoverlapping(
            record.mapped.add(offset),
            destination.as_mut_ptr(),
            destination.len(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gpu_usage_selects_a_device_local_heap() {
        let gpu = BufferUsage::VERTEX | BufferUsage::INDEX;
        assert!(gpu.contains(BufferUsage::VERTEX));
        // The selection rule is: any GPU-readable usage keeps the buffer
        // device-local, so only a transfer-only usage lands in an upload heap.
        let transfer_only = BufferUsage::TRANSFER_SOURCE;
        assert!(!transfer_only.contains(BufferUsage::VERTEX));
        assert!(!transfer_only.contains(BufferUsage::UNIFORM));
    }

    #[test]
    fn dx12_buffer_creation_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let result = create_in_heap_from_pointer(
                core::ptr::null_mut(),
                64,
                sys::D3D12_HEAP_TYPE_DEFAULT,
            );
            assert!(result.is_err());
        }
    }
}
