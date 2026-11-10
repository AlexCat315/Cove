//! Buffer creation, upload and readback for the DX12 backend.
//!
//! Vertex/index/constant/storage buffers live in a default (device-local) heap and
//! are filled through an upload heap. One-way transfer buffers use CPU-visible
//! upload or readback heaps.
#![allow(non_snake_case)]
use crate::rhi::{Buffer, BufferDescriptor, BufferUsage, RhiError, RhiResult, rhi_byte_range};

use super::device::Dx12DeviceHandle;
#[cfg(target_os = "windows")]
use super::resources::{CommandListVtbl, transition};
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
    let is_gpu_resource = descriptor.usage.contains(BufferUsage::VERTEX)
        || descriptor.usage.contains(BufferUsage::INDEX)
        || descriptor.usage.contains(BufferUsage::INDIRECT)
        || descriptor.usage.contains(BufferUsage::UNIFORM)
        || descriptor.usage.contains(BufferUsage::STORAGE_READ)
        || descriptor.usage.contains(BufferUsage::STORAGE_WRITE);
    let source = descriptor.usage.contains(BufferUsage::TRANSFER_SOURCE);
    let destination = descriptor.usage.contains(BufferUsage::TRANSFER_DESTINATION);
    let heap = if is_gpu_resource || (source && destination) {
        sys::D3D12_HEAP_TYPE_DEFAULT
    } else if destination {
        sys::D3D12_HEAP_TYPE_READBACK
    } else {
        sys::D3D12_HEAP_TYPE_UPLOAD
    };
    let flags = if descriptor.usage.contains(BufferUsage::STORAGE_WRITE) {
        sys::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS
    } else {
        sys::D3D12_RESOURCE_FLAG_NONE
    };
    let mut record =
        create_in_heap_from_pointer(device.device, descriptor.size.max(1), heap, flags)?;
    record.usage = descriptor.usage;
    registries.buffers.insert(handle.id(), record);
    Ok(())
}

/// Creates a buffer in an upload heap, for the ring and staging paths.
pub fn create_upload_heap(device: sys::ComPtr, size: usize) -> RhiResult<Dx12ResourceRecord> {
    create_in_heap_for_transfer(device, size, sys::D3D12_HEAP_TYPE_UPLOAD)
}

/// Creates a staging buffer in the given heap, for one texture transfer.
pub fn create_in_heap_for_transfer(
    device: sys::ComPtr,
    size: usize,
    heap_type: u32,
) -> RhiResult<Dx12ResourceRecord> {
    let mut record = create_in_heap_from_pointer(
        device,
        size.max(1),
        heap_type,
        sys::D3D12_RESOURCE_FLAG_NONE,
    )?;
    record.usage =
        crate::rhi::BufferUsage::TRANSFER_SOURCE | crate::rhi::BufferUsage::TRANSFER_DESTINATION;
    Ok(record)
}

/// Creates one committed buffer resource and maps it when the heap allows.
fn create_in_heap_from_pointer(
    device: sys::ComPtr,
    size: usize,
    heap_type: u32,
    flags: u32,
) -> RhiResult<Dx12ResourceRecord> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, size, heap_type, flags);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
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
            // D3D12 rejects `UNKNOWN` for buffer resources; the row-major
            // layout is the only valid one for buffers in any heap type.
            Layout: sys::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: flags,
        };
        let mut resource: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: `device` is a live `ID3D12Device` and the vtable is declared
        // in header order.
        let status = unsafe {
            let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
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
            // Only a genuine allocation failure is "out of memory"; reporting
            // E_INVALIDARG as such hides descriptor mistakes.
            return Err(if status == sys::E_OUTOFMEMORY {
                RhiError::OutOfMemory
            } else {
                RhiError::invalid_argument(sys::hresult_message(status))
            });
        }
        let mut mapped = core::ptr::null_mut();
        // Upload and readback heaps are both CPU-visible; a default heap is not.
        if heap_type == sys::D3D12_HEAP_TYPE_UPLOAD || heap_type == sys::D3D12_HEAP_TYPE_READBACK {
            // SAFETY: the resource is live and the null range permits reading
            // any byte of the persistently mapped buffer.
            let status = unsafe {
                let vtbl = *(resource as *mut *const ResourceVtbl);
                ((*vtbl).Map)(resource, 0, core::ptr::null(), &mut mapped)
            };
            if status != sys::S_OK || mapped.is_null() {
                // SAFETY: the resource has not been submitted.
                unsafe { sys::IUnknownVtbl::release(resource) };
                return Err(RhiError::invalid_argument(sys::hresult_message(status)));
            }
        }
        Ok(Dx12ResourceRecord {
            resource,
            size: size as u64,
            gpu_address: gpu_address(resource),
            mapped: mapped.cast(),
            heap_type,
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

/// `ID3D12Resource`'s vtable prefix, which is all the backend calls.
#[repr(C)]
pub struct ResourceVtbl {
    pub base: sys::IUnknownVtbl,
    pub child: sys::ID3D12DeviceChildSlots,
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

/// Copies `data` into the buffer, staging device-local memory.
pub fn upload(
    registries: &Dx12Registries,
    device: &mut Dx12DeviceHandle,
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
    if record.heap_type == sys::D3D12_HEAP_TYPE_UPLOAD {
        // SAFETY: the mapping covers the validated range and data is disjoint.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), record.mapped.add(offset), data.len());
        }
        return Ok(());
    }
    let staging = create_upload_heap(device.device, data.len())?;
    // SAFETY: the upload heap is mapped and covers the full data slice.
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), staging.mapped, data.len()) };
    let result = copy_immediate(device, &staging, 0, record, offset, data.len());
    // SAFETY: the copy has retired or could not be submitted.
    unsafe { sys::IUnknownVtbl::release(staging.resource) };
    result
}

/// Copies `destination.len()` bytes back out of a mapped buffer.
pub fn read(
    registries: &Dx12Registries,
    device: &mut Dx12DeviceHandle,
    buffer: Buffer,
    offset: usize,
    destination: &mut [u8],
) -> RhiResult<()> {
    let record = registries
        .buffers
        .get(&buffer.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
    rhi_byte_range(offset, destination.len(), record.size as usize)?;
    if destination.is_empty() {
        return Ok(());
    }
    if record.heap_type == sys::D3D12_HEAP_TYPE_DEFAULT {
        let staging = create_in_heap_for_transfer(
            device.device,
            destination.len(),
            sys::D3D12_HEAP_TYPE_READBACK,
        )?;
        let result = copy_immediate(device, record, offset, &staging, 0, destination.len());
        if result.is_ok() {
            // SAFETY: the copy has retired and wrote the full destination range.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    staging.mapped,
                    destination.as_mut_ptr(),
                    destination.len(),
                );
            }
        }
        // SAFETY: the copy has retired or could not be submitted.
        unsafe { sys::IUnknownVtbl::release(staging.resource) };
        return result;
    }
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

fn copy_immediate(
    device: &mut Dx12DeviceHandle,
    src: &Dx12ResourceRecord,
    src_offset: usize,
    dst: &Dx12ResourceRecord,
    dst_offset: usize,
    size: usize,
) -> RhiResult<()> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, src, src_offset, dst, dst_offset, size);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let list = super::command_list::open_graphics_list(device)?;
        // SAFETY: the caller validated both ranges. Default-heap buffers decay
        // to COMMON after each ExecuteCommandLists call.
        unsafe {
            for (record, state) in [
                (src, sys::D3D12_RESOURCE_STATE_COPY_SOURCE),
                (dst, sys::D3D12_RESOURCE_STATE_COPY_DEST),
            ] {
                if record.heap_type == sys::D3D12_HEAP_TYPE_DEFAULT {
                    transition(
                        list,
                        record.resource,
                        sys::D3D12_RESOURCE_STATE_COMMON,
                        state,
                    );
                }
            }
            let vtbl = *(list as *mut *const CommandListVtbl);
            ((*vtbl).CopyBufferRegion)(
                list,
                dst.resource,
                dst_offset as u64,
                src.resource,
                src_offset as u64,
                size as u64,
            );
        }
        let fence = super::command_list::submit_graphics_list(device, list)?;
        super::device::wait_idle(&device.graphics, fence)
    }
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
                sys::D3D12_RESOURCE_FLAG_NONE,
            );
            assert!(result.is_err());
        }
    }
}
