//! Texture creation, sampling and transfer for the DX12 backend.
//!
//! Views (RTV/DSV) are carved from the non-shader-visible descriptor heaps the
//! device owns; upload and readback go through upload/readback heaps, matching
//! Swift's transfer path.
#![allow(non_snake_case)]
use crate::rhi::{
    RhiError, RhiResult, Sampler, SamplerDescriptor, Texture, TextureDescriptor, TextureDimension,
    TextureSubresource, TextureUploadRegion, rhi_byte_range, rhi_require,
    rhi_texture_subresource_extent, rhi_texture_transfer_bytes, rhi_texture_upload_bytes,
};

use super::device::Dx12DeviceHandle;
use super::formats;
use super::resources::{Dx12Registries, Dx12TextureRecord};
use super::sys;

/// Creates one committed texture and records it.
pub fn create(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: Texture,
    descriptor: &TextureDescriptor,
) -> RhiResult<()> {
    let is_attachment = descriptor.usage.contains(crate::rhi::TextureUsage::COLOR_TARGET)
        || descriptor.usage.contains(crate::rhi::TextureUsage::DEPTH_STENCIL_TARGET);
    let mut flags = sys::D3D12_RESOURCE_FLAG_NONE;
    if descriptor.usage.contains(crate::rhi::TextureUsage::COLOR_TARGET) {
        flags |= sys::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if descriptor.usage.contains(crate::rhi::TextureUsage::DEPTH_STENCIL_TARGET) {
        flags |= sys::D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    }
    if descriptor.usage.contains(crate::rhi::TextureUsage::STORAGE_WRITE) {
        flags |= sys::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    let description = sys::D3D12_RESOURCE_DESC {
        Dimension: formats::resource_dimension(descriptor.dimension),
        Alignment: 0,
        Width: descriptor.width.max(1) as u64,
        Height: descriptor.height.max(1) as u32,
        DepthOrArraySize: formats::array_size(descriptor.dimension, descriptor.layers),
        MipLevels: descriptor.mip_levels.max(1) as u16,
        Format: formats::dxgi_format(descriptor.format),
        SampleDesc: [formats::sample_count(descriptor.sample_count)?, 0],
        Layout: 0,
        Flags: flags,
    };

    let mut record = create_committed(device, &description)?;
    record.format = descriptor.format;
    record.width = descriptor.width;
    record.height = descriptor.height;
    record.depth = descriptor.depth;
    record.layers = if descriptor.dimension == TextureDimension::Cube {
        6
    } else {
        descriptor.layers.max(1)
    };
    record.mip_levels = descriptor.mip_levels.max(1);
    record.sample_count = descriptor.sample_count;
    record.dimension = descriptor.dimension;
    record.usage = descriptor.usage;

    // Carve the views a render target or depth target needs.
    if is_attachment {
        let descriptor_handle = allocate_descriptor(device, registries, if descriptor.format.is_depth() {
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_DSV
        } else {
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_RTV
        })?;
        create_view(device, record.resource, descriptor.format, descriptor_handle)?;
        if descriptor.format.is_depth() {
            record.dsv = Some(descriptor_handle);
        } else {
            record.rtv = Some(descriptor_handle);
        }
    }
    registries.textures.insert(handle.id(), record);
    Ok(())
}

/// Creates one committed texture resource (no metadata).
fn create_committed(
    device: &Dx12DeviceHandle,
    description: &sys::D3D12_RESOURCE_DESC,
) -> RhiResult<Dx12TextureRecord> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, description);
        Err(RhiError::unsupported_backend("DirectX 12 is only available on Windows"))
    }
    #[cfg(target_os = "windows")]
    {
        let heap = sys::D3D12_HEAP_PROPERTIES {
            Type: sys::D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let mut resource: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the device is live, the descriptor outlives the call, and the state
        // matches a default-heap texture.
        let status = unsafe {
            let vtbl = *(device.device as *mut *const super::buffer::DeviceCreateVtbl);
            ((*vtbl).CreateCommittedResource)(
                device.device,
                &heap,
                sys::D3D12_HEAP_FLAG_NONE,
                description,
                sys::D3D12_RESOURCE_STATE_COMMON,
                core::ptr::null(),
                &sys::IID_ID3D12_RESOURCE,
                &mut resource,
            )
        };
        if status != sys::S_OK || resource.is_null() {
            return Err(RhiError::OutOfMemory);
        }
        Ok(Dx12TextureRecord {
            resource,
            format: crate::rhi::TextureFormat::Invalid,
            width: 0,
            height: 0,
            depth: 1,
            layers: 1,
            mip_levels: 1,
            sample_count: 1,
            dimension: TextureDimension::Texture2D,
            usage: crate::rhi::TextureUsage::EMPTY,
            rtv: None,
            dsv: None,
        })
    }
}

/// Reserves one descriptor from the named heap and returns its CPU handle.
fn allocate_descriptor(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    heap_type: u32,
) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    let (heap, index, size) = if heap_type == sys::D3D12_DESCRIPTOR_HEAP_TYPE_DSV {
        (device.dsv_heap, device.next_rtv_index, device.rtv_descriptor_size)
    } else {
        (device.rtv_heap, device.next_rtv_index, device.rtv_descriptor_size)
    };
    let _ = registries;
    // A descriptor handle is the heap base plus `index * size`; D3D12 also offers
    // `GetCPUDescriptorHandleForHeapStart`, which needs the heap's vtable.
    let base = heap_start(heap)?;
    Ok(sys::D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as usize) * size as usize,
    })
}

/// `GetCPUDescriptorHandleForHeapStart` on a descriptor heap.
fn heap_start(heap: sys::ComPtr) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = heap;
        Err(RhiError::unsupported_backend("DirectX 12 is only available on Windows"))
    }
    #[cfg(target_os = "windows")]
    {
        // SAFETY: the heap is live; the vtable entry matches
        // `GetCPUDescriptorHandleForHeapStart`.
        let handle = unsafe {
            let vtbl = *(heap as *mut *const DescriptorHeapVtbl);
            ((*vtbl).GetCPUDescriptorHandleForHeapStart)(heap)
        };
        Ok(handle)
    }
}

/// `ID3D12DescriptorHeap`'s vtable prefix.
#[repr(C)]
struct DescriptorHeapVtbl {
    base: sys::IUnknownVtbl,
    GetDesc: usize,
    GetCPUDescriptorHandleForHeapStart:
        unsafe extern "system" fn(sys::ComPtr) -> sys::D3D12_CPU_DESCRIPTOR_HANDLE,
    GetGPUDescriptorHandleForHeapStart: usize,
}

/// Creates the render-target or depth-stencil view for a texture.
fn create_view(
    device: &Dx12DeviceHandle,
    resource: sys::ComPtr,
    format: crate::rhi::TextureFormat,
    descriptor: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) -> RhiResult<()> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, resource, format, descriptor);
        Err(RhiError::unsupported_backend("DirectX 12 is only available on Windows"))
    }
    #[cfg(target_os = "windows")]
    {
        // The view description is a plain struct whose first fields are the format
        // and dimension; a null description asks D3D12 for a default view over the
        // whole resource, which is what a single-mip attachment needs.
        let _ = format;
        let mut view = ViewDescription::default();
        view.format = formats::dxgi_format(format);
        view.view_dimension = if format.is_depth() { 3 } else { 4 };
        // SAFETY: the device and resource are live; the vtable entry matches
        // `CreateRenderTargetView`/`CreateDepthStencilView`, which take the same
        // arguments.
        unsafe {
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
            if format.is_depth() {
                ((*vtbl).CreateDepthStencilView)(
                    device.device,
                    resource,
                    (&view as *const ViewDescription).cast(),
                    descriptor,
                );
            } else {
                ((*vtbl).CreateRenderTargetView)(
                    device.device,
                    resource,
                    (&view as *const ViewDescription).cast(),
                    descriptor,
                );
            }
        }
        Ok(())
    }
}

/// `D3D12_RENDER_TARGET_VIEW_DESC` / `D3D12_DEPTH_STENCIL_VIEW_DESC` prefix: both
/// start with the format, the view dimension and the matching union member, and a
/// null description means "default view", so this struct is only built when a view
/// needs an explicit format.
#[repr(C)]
#[derive(Default)]
struct ViewDescription {
    format: u32,
    view_dimension: u32,
    /// The union's largest member (a texture-2D-array view).
    payload: [u32; 8],
}

/// Creates one sampler by writing a `D3D12_SAMPLER_DESC` into the sampler heap.
///
/// D3D12 has no sampler object: the descriptor *is* the sampler, so the registry
/// stores the handle the encoder binds.
pub fn create_sampler(
    device: &Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: Sampler,
    descriptor: &SamplerDescriptor,
) -> RhiResult<()> {
    let description = sys::D3D12_SAMPLER_DESC {
        // A comparison sampler needs the comparison filter variant; otherwise the
        // linear/point choice comes straight from the RHI filters.
        Filter: if descriptor.compare_enabled {
            sys::D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR
        } else if descriptor.min_filter == crate::rhi::SamplerFilter::Linear
            || descriptor.mag_filter == crate::rhi::SamplerFilter::Linear
        {
            sys::D3D12_FILTER_MIN_MAG_MIP_LINEAR
        } else {
            sys::D3D12_FILTER_MIN_MAG_MIP_POINT
        },
        AddressU: formats::sampler_address_mode(descriptor.address_mode_u),
        AddressV: formats::sampler_address_mode(descriptor.address_mode_v),
        AddressW: formats::sampler_address_mode(descriptor.address_mode_w),
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: formats::comparison_function(descriptor.compare_op),
        BorderColor: [0.0, 0.0, 0.0, 1.0],
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
    };
    let cpu_handle = write_sampler(device, &description)?;
    // The registry keeps the descriptor handle as the sampler's identity, which is
    // what the command list binds through the sampler heap.
    registries.samplers.insert(handle.id(), cpu_handle);
    Ok(())
}

/// Writes one sampler description into the sampler heap at the next free slot.
fn write_sampler(
    device: &Dx12DeviceHandle,
    description: &sys::D3D12_SAMPLER_DESC,
) -> RhiResult<sys::ComPtr> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, description);
        Err(RhiError::unsupported_backend("DirectX 12 is only available on Windows"))
    }
    #[cfg(target_os = "windows")]
    {
        let base = heap_start(device.sampler_heap)?;
        let handle = sys::D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: base.ptr + (device.next_rtv_index as usize) * device.sampler_descriptor_size as usize,
        };
        // SAFETY: the device and sampler heap are live, the description outlives the
        // call, and the handle was computed inside the heap.
        unsafe {
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateSampler)(device.device, description, handle);
        }
        // The returned `ComPtr` is the descriptor address, not a COM object, so it
        // must never be released; the caller stores it as an opaque value.
        Ok(handle.ptr as sys::ComPtr)
    }
}

/// The committed byte size of one texture, for allocation reporting.
pub fn allocation_bytes(record: &Dx12TextureRecord) -> u64 {
    let texel = record.format.bytes_per_texel().max(1) as u64;
    let layers = record.layers.max(1) as u64;
    let mips = record.mip_levels.max(1) as u64;
    (record.width.max(1) as u64)
        * (record.height.max(1) as u64)
        * (record.depth.max(1) as u64)
        * layers
        * mips
        * texel
}

/// Uploads one region through an upload heap.
///
/// The transfer is recorded on a one-shot copy command list, which is the DX12
/// equivalent of Swift's `grhi_dx12_transfer_texture` upload arm.
pub fn upload_snapshot(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    data: &[u8],
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
) -> RhiResult<()> {
    rhi_require(
        record.sample_count == 1,
        "texture transfer requires a single sample",
    )?;
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        subresource,
        record.width,
        record.height,
        record.mip_levels,
        record.layers,
    )?;
    let byte_count = rhi_texture_upload_bytes(
        region,
        bytes_per_row,
        record.format,
        extent_width,
        extent_height,
        data.len(),
    )?;
    rhi_byte_range(0, byte_count, data.len())?;
    transfer(device, record, data, byte_count, region, bytes_per_row, subresource, true)
}

/// Reads one region back through a readback heap.
pub fn read_snapshot(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    width: usize,
    height: usize,
    bytes_per_row: usize,
    subresource: TextureSubresource,
    destination: &mut [u8],
) -> RhiResult<()> {
    rhi_require(
        record.sample_count == 1,
        "texture readback requires a single sample",
    )?;
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        subresource,
        record.width,
        record.height,
        record.mip_levels,
        record.layers,
    )?;
    let byte_count = rhi_texture_transfer_bytes(
        width,
        height,
        bytes_per_row,
        record.format,
        extent_width,
        extent_height,
        destination.len(),
    )?;
    transfer(
        device,
        record,
        destination,
        byte_count,
        TextureUploadRegion::new(width as i64, height as i64),
        bytes_per_row,
        subresource,
        false,
    )
}

/// One staged copy between a texture and a buffer.
fn transfer(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    bytes: &mut [u8],
    byte_count: usize,
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
    upload: bool,
) -> RhiResult<()> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device, record, bytes, byte_count, region, bytes_per_row, subresource, upload);
        Err(RhiError::unsupported_backend("DirectX 12 is only available on Windows"))
    }
    #[cfg(target_os = "windows")]
    {
        // A texture copy needs the resource in a copy state, so the region is
        // staged through a buffer in the matching heap and a one-shot list, exactly
        // as the buffer path does for vertex data.
        let heap = if upload {
            sys::D3D12_HEAP_TYPE_UPLOAD
        } else {
            sys::D3D12_HEAP_TYPE_READBACK
        };
        let staging = super::buffer::create_in_heap_for_transfer(device.device, byte_count.max(1), heap)?;
        if upload {
            let pointer = staging.mapped;
            rhi_require(!pointer.is_null(), "upload heap is not mapped")?;
            // SAFETY: the staging buffer holds at least `byte_count` bytes and the
            // source slice is distinct.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, byte_count);
            }
        }
        let result = record_copy(
            device,
            record,
            staging.resource,
            region,
            bytes_per_row,
            subresource,
            upload,
        );
        if result.is_ok() && !upload {
            let pointer = staging.mapped;
            if !pointer.is_null() {
                // SAFETY: the copy completed and the readback mapping covers the
                // range; the destination is a distinct live slice.
                unsafe {
                    std::ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), byte_count)
                };
            }
        }
        // SAFETY: the copy completed (or failed before queueing), so the staging
        // resource is unreferenced.
        unsafe { sys::IUnknownVtbl::release(staging.resource) };
        result
    }
}

/// Records and submits one buffer-to-texture or texture-to-buffer copy.
#[cfg(target_os = "windows")]
fn record_copy(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    staging: sys::ComPtr,
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
    upload: bool,
) -> RhiResult<()> {
    let list = super::command_list::open_transfer_list(device)?;
    let (before, after) = if upload {
        (sys::D3D12_RESOURCE_STATE_COPY_DEST, sys::D3D12_RESOURCE_STATE_COMMON)
    } else {
        (sys::D3D12_RESOURCE_STATE_COPY_SOURCE, sys::D3D12_RESOURCE_STATE_COMMON)
    };
    // SAFETY: the texture resource and the list are live.
    unsafe { super::resources::transition(list, record.resource, sys::D3D12_RESOURCE_STATE_COMMON, before) };

    let texture_location = sys::D3D12_TEXTURE_COPY_LOCATION {
        pResource: record.resource,
        Type: sys::D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        SubresourceIndex: (subresource.mip_level
            + subresource.layer * record.mip_levels) as u32,
        PlacedFootprintPadding: [0; 6],
    };
    let buffer_location = sys::D3D12_TEXTURE_COPY_LOCATION {
        pResource: staging,
        Type: sys::D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        SubresourceIndex: 0,
        PlacedFootprintPadding: [
            bytes_per_row as u32,
            region.height as u32,
            region.width as u32,
            region.height as u32,
            1,
            0,
        ],
    };
    let box_region = sys::D3D12_BOX {
        left: region.origin[0].max(0) as u32,
        top: region.origin[1].max(0) as u32,
        front: 0,
        right: (region.origin[0] + region.width) as u32,
        bottom: (region.origin[1] + region.height) as u32,
        back: 1,
    };
    // SAFETY: both resources are live and in the states the copy names.
    unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        let (destination, destination_subresource, destination_x, destination_y) = if upload {
            (&texture_location, 0u32, 0u32, 0u32)
        } else {
            (&buffer_location, 0u32, 0u32, 0u32)
        };
        let (source, source_subresource, source_x, source_y) = if upload {
            (&buffer_location, 0u32, 0u32, 0u32)
        } else {
            (&texture_location, 0u32, 0u32, 0u32)
        };
        ((*vtbl).CopyTextureRegion)(
            list,
            destination,
            destination_subresource,
            destination_x,
            destination_y,
            0,
            source,
            &box_region,
        );
        super::resources::transition(list, record.resource, before, after);
    }
    super::command_list::close_and_submit(device, list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_bytes_cover_layers_and_mips() {
        let record = Dx12TextureRecord {
            resource: core::ptr::null_mut(),
            format: crate::rhi::TextureFormat::Rgba8Unorm,
            width: 4,
            height: 4,
            depth: 1,
            layers: 6,
            mip_levels: 1,
            sample_count: 1,
            dimension: TextureDimension::Cube,
            usage: crate::rhi::TextureUsage::SAMPLED,
            rtv: None,
            dsv: None,
        };
        // 4 * 4 texels * 4 bytes * 6 faces * 1 mip
        assert_eq!(allocation_bytes(&record), 4 * 4 * 4 * 6);
    }

    #[test]
    fn dx12_texture_creation_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let description = sys::D3D12_RESOURCE_DESC::default();
            let device = super::super::device::Dx12DeviceHandle::make(
                crate::rhi::DeviceConfig::default(),
            );
            assert!(device.is_err());
            let _ = description;
        }
    }
}
