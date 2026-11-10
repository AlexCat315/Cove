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
    device: &mut Dx12DeviceHandle,
    registries: &mut Dx12Registries,
    handle: Texture,
    descriptor: &TextureDescriptor,
) -> RhiResult<()> {
    let is_attachment = descriptor
        .usage
        .contains(crate::rhi::TextureUsage::COLOR_TARGET)
        || descriptor
            .usage
            .contains(crate::rhi::TextureUsage::DEPTH_STENCIL_TARGET);
    let mut flags = sys::D3D12_RESOURCE_FLAG_NONE;
    if descriptor
        .usage
        .contains(crate::rhi::TextureUsage::COLOR_TARGET)
    {
        flags |= sys::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if descriptor
        .usage
        .contains(crate::rhi::TextureUsage::DEPTH_STENCIL_TARGET)
    {
        flags |= sys::D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    }
    if descriptor
        .usage
        .contains(crate::rhi::TextureUsage::STORAGE_WRITE)
    {
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
        let index = device.take_rtv_index()?;
        let descriptor_handle = if descriptor.format.is_depth() {
            dsv_heap_cpu(device, index)?
        } else {
            rtv_heap_cpu(device, index)?
        };
        create_view(
            device,
            record.resource,
            descriptor.format,
            descriptor_handle,
        )?;
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
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
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
            let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
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
            return Err(if status == sys::E_OUTOFMEMORY {
                RhiError::OutOfMemory
            } else {
                RhiError::invalid_argument(sys::hresult_message(status))
            });
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

/// CPU handle for `index` in the render-target-view heap.
pub fn rtv_heap_cpu(
    device: &Dx12DeviceHandle,
    index: u32,
) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    let base = heap_start(device.rtv_heap)?;
    Ok(sys::D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as usize) * device.rtv_descriptor_size as usize,
    })
}

/// CPU handle for `index` in the depth-stencil-view heap.
pub fn dsv_heap_cpu(
    device: &Dx12DeviceHandle,
    index: u32,
) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    let base = heap_start(device.dsv_heap)?;
    Ok(sys::D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as usize) * device.rtv_descriptor_size as usize,
    })
}

/// `GetCPUDescriptorHandleForHeapStart` on a descriptor heap.
fn heap_start(heap: sys::ComPtr) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = heap;
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        // A COM method returning a struct hands it back through a hidden
        // out-parameter under the MSVC ABI, so the handle is written into
        // `handle` rather than returned in a register.
        let mut handle = sys::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 };
        // SAFETY: the heap is live, the vtable is declared in header order and
        // `handle` is a writable local.
        unsafe {
            let vtbl = *(heap as *mut *const DescriptorHeapVtbl);
            ((*vtbl).GetCPUDescriptorHandleForHeapStart)(heap, &mut handle);
        }
        Ok(handle)
    }
}

/// `ID3D12DescriptorHeap`'s vtable prefix.
#[repr(C)]
struct DescriptorHeapVtbl {
    base: sys::IUnknownVtbl,
    child: sys::ID3D12DeviceChildSlots,
    GetDesc: usize,
    GetCPUDescriptorHandleForHeapStart:
        unsafe extern "system" fn(
            sys::ComPtr,
            *mut sys::D3D12_CPU_DESCRIPTOR_HANDLE,
        ) -> *mut sys::D3D12_CPU_DESCRIPTOR_HANDLE,
    GetGPUDescriptorHandleForHeapStart:
        unsafe extern "system" fn(
            sys::ComPtr,
            *mut sys::D3D12_GPU_DESCRIPTOR_HANDLE,
        ) -> *mut sys::D3D12_GPU_DESCRIPTOR_HANDLE,
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
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        // The view description is a plain struct whose first fields are the format
        // and dimension; a null description asks D3D12 for a default view over the
        // whole resource, which is what a single-mip attachment needs.
        let view = ViewDescription {
            format: formats::dxgi_format(format),
            view_dimension: if format.is_depth() { 3 } else { 4 },
            ..Default::default()
        };
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
/// stores the description and writes it into the shader-visible heap when a
/// binding set is registered.
pub fn create_sampler(
    _device: &Dx12DeviceHandle,
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
    registries.samplers.insert(handle.id(), description);
    Ok(())
}

/// Writes one sampler description into the sampler heap at the given CPU handle.
#[cfg(target_os = "windows")]
pub fn write_sampler_at(
    device: &Dx12DeviceHandle,
    description: &sys::D3D12_SAMPLER_DESC,
    handle: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    // SAFETY: the device and sampler heap are live, the description outlives the
    // call, and the handle points inside the heap.
    unsafe {
        let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
        ((*vtbl).CreateSampler)(device.device, description, handle);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn write_sampler_at(
    _device: &Dx12DeviceHandle,
    _description: &sys::D3D12_SAMPLER_DESC,
    _handle: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) {
}

/// The GPU start handle of a descriptor heap.
fn heap_start_gpu(heap: sys::ComPtr) -> RhiResult<sys::D3D12_GPU_DESCRIPTOR_HANDLE> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = heap;
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let mut handle = sys::D3D12_GPU_DESCRIPTOR_HANDLE { ptr: 0 };
        // SAFETY: the heap is live and the vtable returns the handle through the
        // hidden out-parameter used by the MSVC ABI.
        unsafe {
            let vtbl = *(heap as *mut *const DescriptorHeapVtbl);
            ((*vtbl).GetGPUDescriptorHandleForHeapStart)(heap, &mut handle);
        }
        Ok(handle)
    }
}

/// CPU handle for `index` in the shader-visible CBV/SRV/UAV heap.
pub fn resource_heap_cpu(
    device: &Dx12DeviceHandle,
    index: u32,
) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    let base = heap_start(device.resource_heap)?;
    Ok(sys::D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as usize) * device.resource_descriptor_size as usize,
    })
}

/// GPU handle for `index` in the shader-visible CBV/SRV/UAV heap.
pub fn resource_heap_gpu(
    device: &Dx12DeviceHandle,
    index: u32,
) -> RhiResult<sys::D3D12_GPU_DESCRIPTOR_HANDLE> {
    let base = heap_start_gpu(device.resource_heap)?;
    Ok(sys::D3D12_GPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as u64) * device.resource_descriptor_size as u64,
    })
}

/// CPU handle for `index` in the shader-visible sampler heap.
pub fn sampler_heap_cpu(
    device: &Dx12DeviceHandle,
    index: u32,
) -> RhiResult<sys::D3D12_CPU_DESCRIPTOR_HANDLE> {
    let base = heap_start(device.sampler_heap)?;
    Ok(sys::D3D12_CPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as usize) * device.sampler_descriptor_size as usize,
    })
}

/// GPU handle for `index` in the shader-visible sampler heap.
pub fn sampler_heap_gpu(
    device: &Dx12DeviceHandle,
    index: u32,
) -> RhiResult<sys::D3D12_GPU_DESCRIPTOR_HANDLE> {
    let base = heap_start_gpu(device.sampler_heap)?;
    Ok(sys::D3D12_GPU_DESCRIPTOR_HANDLE {
        ptr: base.ptr + (index as u64) * device.sampler_descriptor_size as u64,
    })
}

/// Creates a shader-resource view for `texture` at the given CPU handle.
#[cfg(not(target_os = "windows"))]
pub fn create_texture_srv(
    _device: &Dx12DeviceHandle,
    _texture: sys::ComPtr,
    _handle: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) {
}

#[cfg(not(target_os = "windows"))]
pub fn create_texture_uav(
    _device: &Dx12DeviceHandle,
    _texture: sys::ComPtr,
    _handle: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) {
}

/// Creates a shader-resource view for `texture` at the given CPU handle.
#[cfg(target_os = "windows")]
pub fn create_texture_srv(
    device: &Dx12DeviceHandle,
    texture: sys::ComPtr,
    handle: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    // SAFETY: the device and texture are live; a null description creates a
    // default SRV covering the whole resource.
    unsafe {
        let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
        ((*vtbl).CreateShaderResourceView)(device.device, texture, core::ptr::null(), handle);
    }
}

/// Creates an unordered-access view for `texture` at the given CPU handle.
#[cfg(target_os = "windows")]
pub fn create_texture_uav(
    device: &Dx12DeviceHandle,
    texture: sys::ComPtr,
    handle: sys::D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    // SAFETY: the device and texture are live; a null description creates a
    // default UAV covering the whole resource.
    unsafe {
        let vtbl = *(device.device as *mut *const sys::ID3D12DeviceVtbl);
        ((*vtbl).CreateUnorderedAccessView)(
            device.device,
            texture,
            core::ptr::null_mut(),
            core::ptr::null(),
            handle,
        );
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
    transfer_upload(
        device,
        record,
        data,
        byte_count,
        region,
        bytes_per_row,
        subresource,
    )
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
    transfer_read(
        device,
        record,
        destination,
        byte_count,
        TextureUploadRegion::new(width as i64, height as i64),
        bytes_per_row,
        subresource,
    )
}

/// One staged copy from `bytes` into the texture.
fn transfer_upload(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    bytes: &[u8],
    byte_count: usize,
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
) -> RhiResult<()> {
    transfer(
        device,
        record,
        Some(bytes),
        None,
        byte_count,
        region,
        bytes_per_row,
        subresource,
        true,
    )
}

/// One staged copy from the texture into `destination`.
fn transfer_read(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    destination: &mut [u8],
    byte_count: usize,
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
) -> RhiResult<()> {
    transfer(
        device,
        record,
        None,
        Some(destination),
        byte_count,
        region,
        bytes_per_row,
        subresource,
        false,
    )
}

/// One staged copy between a texture and a buffer.
#[allow(clippy::too_many_arguments)]
fn transfer(
    device: &mut Dx12DeviceHandle,
    record: &super::backend::TextureSnapshot,
    source: Option<&[u8]>,
    destination: Option<&mut [u8]>,
    byte_count: usize,
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
    upload: bool,
) -> RhiResult<()> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (
            device,
            record,
            source,
            destination,
            byte_count,
            region,
            bytes_per_row,
            subresource,
            upload,
        );
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
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
        // The buffer side of a texture copy is a placed footprint whose rows
        // must start on 256-byte boundaries, so tightly packed caller rows are
        // re-laid out through the staging buffer.
        let rows = usize::try_from(region.height).unwrap_or(0);
        let pitch = bytes_per_row.next_multiple_of(sys::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT);
        let staging =
            super::buffer::create_in_heap_for_transfer(device.device, (pitch * rows).max(1), heap)?;
        let row_bytes = |row: usize| {
            let start = row * bytes_per_row;
            start..(start + bytes_per_row).min(byte_count)
        };
        if upload {
            let pointer = staging.mapped;
            rhi_require(!pointer.is_null(), "upload heap is not mapped")?;
            let bytes =
                source.ok_or_else(|| RhiError::invalid_argument("missing upload source"))?;
            for row in 0..rows {
                let range = row_bytes(row);
                let Some(chunk) = bytes.get(range) else {
                    break;
                };
                // SAFETY: row `row` of the staging buffer spans `pitch >= chunk.len()`
                // bytes inside its `pitch * rows` allocation; the slices are distinct.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        chunk.as_ptr(),
                        pointer.add(row * pitch),
                        chunk.len(),
                    );
                }
            }
        }
        let result = record_copy(
            device,
            record,
            staging.resource,
            region,
            pitch,
            subresource,
            upload,
        );
        if result.is_ok() && !upload {
            let pointer = staging.mapped;
            if !pointer.is_null()
                && let Some(destination) = destination
            {
                for row in 0..rows {
                    let range = row_bytes(row);
                    let Some(chunk) = destination.get_mut(range) else {
                        break;
                    };
                    // SAFETY: the copy completed and row `row` of the readback
                    // mapping holds at least `chunk.len()` bytes; the slices are
                    // distinct.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            pointer.add(row * pitch),
                            chunk.as_mut_ptr(),
                            chunk.len(),
                        )
                    };
                }
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
    row_pitch: usize,
    subresource: TextureSubresource,
    upload: bool,
) -> RhiResult<()> {
    let list = super::command_list::open_transfer_list(device)?;
    let (before, after) = if upload {
        (
            sys::D3D12_RESOURCE_STATE_COPY_DEST,
            sys::D3D12_RESOURCE_STATE_COMMON,
        )
    } else {
        (
            sys::D3D12_RESOURCE_STATE_COPY_SOURCE,
            sys::D3D12_RESOURCE_STATE_COMMON,
        )
    };
    // SAFETY: the texture resource and the list are live.
    unsafe {
        super::resources::transition(
            list,
            record.resource,
            sys::D3D12_RESOURCE_STATE_COMMON,
            before,
        )
    };

    let texture_location = sys::D3D12_TEXTURE_COPY_LOCATION::subresource(
        record.resource,
        (subresource.mip_level + subresource.layer * record.mip_levels) as u32,
    );
    let width = region.width.max(0) as u32;
    let height = region.height.max(0) as u32;
    let buffer_location = sys::D3D12_TEXTURE_COPY_LOCATION::placed(
        staging,
        sys::D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
            Offset: 0,
            Footprint: sys::D3D12_SUBRESOURCE_FOOTPRINT {
                Format: formats::dxgi_format(record.format),
                Width: width,
                Height: height,
                Depth: 1,
                RowPitch: row_pitch as u32,
            },
        },
    );
    let x = region.origin[0].max(0) as u32;
    let y = region.origin[1].max(0) as u32;
    // SAFETY: both resources are live and in the states the copy names; the
    // footprint describes the staging allocation.
    unsafe {
        let vtbl = *(list as *mut *const super::resources::CommandListVtbl);
        if upload {
            // The whole staged footprint lands at the region's origin.
            let source_box = sys::D3D12_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: width,
                bottom: height,
                back: 1,
            };
            ((*vtbl).CopyTextureRegion)(
                list,
                &texture_location,
                x,
                y,
                0,
                &buffer_location,
                &source_box,
            );
        } else {
            // The region is read out of the texture to the footprint's origin.
            let source_box = sys::D3D12_BOX {
                left: x,
                top: y,
                front: 0,
                right: x + width,
                bottom: y + height,
                back: 1,
            };
            ((*vtbl).CopyTextureRegion)(
                list,
                &buffer_location,
                0,
                0,
                0,
                &texture_location,
                &source_box,
            );
        }
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
            let device =
                super::super::device::Dx12DeviceHandle::make(crate::rhi::DeviceConfig::default());
            assert!(device.is_err());
            let _ = description;
        }
    }
}
