//! Translation of NativeRHI/Metal/Device/MetalDeviceTransfer.swift: binding
//! sets, immediate data transfer, the swapchain, the per-frame uploader and
//! idle synchronization.
//!
//! Every entry point mirrors one Swift method on the device extension. The
//! immediate transfer paths are explicitly synchronous (they are not the
//! per-submit hot path), so they may wait for their blit to complete.
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLDevice, MTLOrigin, MTLResourceOptions, MTLSize, MTLTexture,
};

use crate::rhi::{
    BindingLayout, BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSet,
    BindingSetEntry, Buffer, PipelineLayout, PipelineLayoutDescriptor, RhiCompilationWork,
    RhiError, RhiResult, ShaderVisibility, Swapchain, SwapchainImage, Texture, TextureSubresource,
    TextureUploadRegion, rhi_byte_range, rhi_require, rhi_texture_subresource_extent,
    rhi_texture_transfer_bytes, rhi_texture_upload_bytes,
};

use super::device::{MetalDeviceHandle, MetalWindowSwapchain, VERTEX_BUFFER_BASE_INDEX};
use super::formats;
use super::registries::{MetalBindingSet, MetalBoundEntry, MetalBoundResource, MetalBoundValue};
use super::resources::lock;
use super::upload_ring::{MetalFrameUploader, MetalUploadChunk};

/// Chunk size for one slot's upload ring, matching Swift's 4 MiB.
pub const UPLOAD_CHUNK_SIZE: usize = 4 * 1024 * 1024;

// MARK: - Binding layouts

pub fn register_binding_layout(
    device: &mut MetalDeviceHandle,
    handle: BindingLayout,
    descriptor: BindingLayoutDescriptor,
) -> RhiResult<()> {
    device.interfaces.bindings.insert(handle.id(), descriptor);
    Ok(())
}

pub fn register_pipeline_layout(
    device: &mut MetalDeviceHandle,
    handle: PipelineLayout,
    descriptor: PipelineLayoutDescriptor,
) -> RhiResult<()> {
    rhi_require(
        descriptor.set_layouts.len() <= 1,
        "Metal direct bindings support only set 0",
    )?;
    for range in &descriptor.push_constants {
        rhi_require(
            range.slot < VERTEX_BUFFER_BASE_INDEX,
            "push constant buffer slot is reserved",
        )?;
        let visibility = ShaderVisibility::from(range.stage);
        let entries: Vec<BindingLayoutEntry> = descriptor
            .set_layouts
            .iter()
            .filter_map(|layout| device.interfaces.bindings.get(&layout.id()))
            .flat_map(|layout| layout.entries.iter().copied())
            .collect();
        rhi_require(
            !entries.iter().any(|entry| {
                entry.slot == range.slot
                    && entry.visibility.contains(visibility)
                    && matches!(
                        entry.binding_type,
                        crate::rhi::BindingType::UniformBuffer
                            | crate::rhi::BindingType::StorageBuffer
                            | crate::rhi::BindingType::AccelerationStructure
                    )
            }),
            "push constants overlap a buffer binding",
        )?;
    }
    device.interfaces.pipelines.insert(handle.id(), descriptor);
    Ok(())
}

/// Resolves one binding-set resource into its native object. Called once at set
/// registration, so the encoder never resolves a handle per draw.
fn resolve_binding_resource(
    device: &MetalDeviceHandle,
    resource: BindingResource,
) -> RhiResult<MetalBoundResource> {
    let registries = lock(&device.registries);
    match resource {
        BindingResource::UniformBuffer {
            buffer,
            offset,
            size,
        } => {
            let native = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                RhiError::invalid_argument("binding references an unknown buffer or invalid offset")
            })?;
            let capacity = native.length();
            rhi_require(
                offset < capacity,
                "binding references an unknown buffer or invalid offset",
            )?;
            if let Some(size) = size {
                rhi_require(size > 0, "uniform binding size must be positive")?;
                rhi_byte_range(offset, size, capacity)?;
            }
            Ok(MetalBoundResource {
                id: buffer.id(),
                value: MetalBoundValue::Buffer {
                    buffer: native.clone(),
                    offset,
                },
            })
        }
        BindingResource::StorageBuffer { buffer, offset } => {
            let native = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                RhiError::invalid_argument("binding references an unknown buffer or invalid offset")
            })?;
            let capacity = native.length();
            rhi_require(
                offset < capacity,
                "binding references an unknown buffer or invalid offset",
            )?;
            Ok(MetalBoundResource {
                id: buffer.id(),
                value: MetalBoundValue::Buffer {
                    buffer: native.clone(),
                    offset,
                },
            })
        }
        BindingResource::Texture(texture) | BindingResource::StorageTexture(texture) => {
            let native = registries.textures.get(&texture.id()).ok_or_else(|| {
                RhiError::invalid_argument("binding references an unknown texture")
            })?;
            // Swapchain drawables are framebuffer-only and can change their
            // native object under a frame-slot handle, so they cannot be cached
            // as sampled or storage bindings.
            let required = if matches!(resource, BindingResource::StorageTexture(_)) {
                objc2_metal::MTLTextureUsage::ShaderRead | objc2_metal::MTLTextureUsage::ShaderWrite
            } else {
                objc2_metal::MTLTextureUsage::ShaderRead
            };
            rhi_require(
                native.usage().intersects(required),
                "texture lacks its binding's shader usage",
            )?;
            Ok(MetalBoundResource {
                id: texture.id(),
                value: MetalBoundValue::Texture(native.clone()),
            })
        }
        BindingResource::Sampler(sampler) => {
            let native = registries.samplers.get(&sampler.id()).ok_or_else(|| {
                RhiError::invalid_argument("binding references an unknown sampler")
            })?;
            Ok(MetalBoundResource {
                id: sampler.id(),
                value: MetalBoundValue::Sampler(native.clone()),
            })
        }
        BindingResource::AccelerationStructure(structure) => {
            let native = registries
                .acceleration_structures
                .get(&structure.id())
                .ok_or_else(|| {
                    RhiError::invalid_argument(
                        "binding references an unknown acceleration structure",
                    )
                })?;
            Ok(MetalBoundResource {
                id: structure.id(),
                value: MetalBoundValue::AccelerationStructure(std::sync::Arc::clone(native)),
            })
        }
    }
}

pub fn register_binding_set(
    device: &mut MetalDeviceHandle,
    handle: BindingSet,
    _layout: BindingLayout,
    layout_entries: Vec<BindingLayoutEntry>,
    set_entries: Vec<BindingSetEntry>,
) -> RhiResult<()> {
    // Resolve immutable bindings once; encoding reuses the resolved resources.
    for entry in &layout_entries {
        if matches!(
            entry.binding_type,
            crate::rhi::BindingType::UniformBuffer
                | crate::rhi::BindingType::StorageBuffer
                | crate::rhi::BindingType::AccelerationStructure
        ) {
            rhi_require(
                entry.slot < VERTEX_BUFFER_BASE_INDEX,
                "Metal buffer binding slots 24...31 are reserved for vertex inputs",
            )?;
        }
    }

    // Dynamic offset positions follow the ascending dynamic binding slot.
    let mut dynamic_slots: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    let mut dynamic = layout_entries
        .iter()
        .filter(|entry| entry.buffer.has_dynamic_offset)
        .collect::<Vec<_>>();
    dynamic.sort_by_key(|entry| entry.slot);
    for (index, entry) in dynamic.into_iter().enumerate() {
        dynamic_slots.insert(entry.slot, index);
    }

    let mut resolved: Vec<MetalBoundEntry> = Vec::with_capacity(set_entries.len());
    for set_entry in set_entries {
        let layout_entry = layout_entries
            .iter()
            .find(|entry| entry.slot == set_entry.slot)
            .ok_or_else(|| RhiError::layout_mismatch("binding is missing its layout entry"))?;
        resolved.push(MetalBoundEntry {
            slot: set_entry.slot,
            visibility: layout_entry.visibility,
            resource: resolve_binding_resource(device, set_entry.resource)?,
        });
    }
    lock(&device.registries)
        .binding_sets
        .insert(handle.id(), MetalBindingSet::new(resolved, &dynamic_slots));
    Ok(())
}

pub fn unregister_binding_set(device: &mut MetalDeviceHandle, handle: BindingSet) {
    lock(&device.registries).binding_sets.remove(handle.id());
}

// MARK: - Immediate data transfer

pub fn read_buffer_data(
    device: &mut MetalDeviceHandle,
    buffer: Buffer,
    offset: usize,
    destination: &mut [u8],
) -> RhiResult<()> {
    wait_until_idle(device)?;
    let registries = lock(&device.registries);
    let source = registries
        .buffers
        .get(&buffer.id())
        .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
    let capacity = source.length();
    rhi_byte_range(offset, destination.len(), capacity)?;
    if destination.is_empty() {
        return Ok(());
    }
    // SAFETY: the range was validated inside the buffer and the destination is a
    // distinct live slice, so the two regions cannot overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(
            source.contents().as_ptr().cast::<u8>().add(offset),
            destination.as_mut_ptr(),
            destination.len(),
        );
    }
    Ok(())
}

pub fn upload_buffer_data(
    device: &mut MetalDeviceHandle,
    buffer: Buffer,
    offset: usize,
    data: &[u8],
) -> RhiResult<()> {
    let registries = lock(&device.registries);
    let target = registries.buffers.get(&buffer.id()).ok_or_else(|| {
        RhiError::invalid_argument(format!("uploadBufferData: unknown buffer {}", buffer.id()))
    })?;
    let capacity = target.length();
    rhi_byte_range(offset, data.len(), capacity)?;
    if data.is_empty() {
        return Ok(());
    }
    // Buffers are shared-storage (CPU/GPU coherent on Apple Silicon), so a
    // direct memcpy into contents needs no blit synchronization.
    // SAFETY: the range was validated inside the buffer, and `data` is a
    // distinct live slice.
    unsafe {
        std::ptr::copy_nonoverlapping(
            data.as_ptr(),
            target.contents().as_ptr().cast::<u8>().add(offset),
            data.len(),
        );
    }
    Ok(())
}

/// Lazily-grown shared staging buffer reused for immediate transfers.
fn shared_staging_buffer(
    device: &mut MetalDeviceHandle,
    minimum_size: usize,
) -> RhiResult<Retained<ProtocolObject<dyn MTLBuffer>>> {
    if let Some(existing) = &device.staging_buffer
        && existing.length() >= minimum_size
    {
        return Ok(existing.clone());
    }
    let buffer = device
        .device
        .newBufferWithLength_options(minimum_size.max(1), MTLResourceOptions::StorageModeShared)
        .ok_or(RhiError::OutOfMemory)?;
    device.staging_buffer = Some(buffer.clone());
    Ok(buffer)
}

/// Runs one blit and waits for it. The immediate transfer APIs are synchronous
/// by contract, so blocking here is correct; the submit path never does this.
fn run_blit(
    device: &MetalDeviceHandle,
    encode: impl FnOnce(&ProtocolObject<dyn MTLBlitCommandEncoder>) -> RhiResult<()>,
) -> RhiResult<()> {
    let command_buffer = device
        .graphics_queue
        .commandBuffer()
        .ok_or(RhiError::OutOfMemory)?;
    let blit = command_buffer
        .blitCommandEncoder()
        .ok_or_else(|| RhiError::submit_failed("cannot create blit encoder"))?;
    encode(&blit)?;
    blit.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();
    if let Some(error) = command_buffer.error() {
        return Err(RhiError::submit_failed(
            error.localizedDescription().to_string(),
        ));
    }
    Ok(())
}

pub fn upload_texture_data(
    device: &mut MetalDeviceHandle,
    texture: Texture,
    data: &[u8],
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
) -> RhiResult<()> {
    let (native, format, layers) = {
        let registries = lock(&device.registries);
        let native = registries
            .textures
            .get(&texture.id())
            .ok_or_else(|| {
                RhiError::invalid_argument(format!(
                    "uploadTextureData: unknown texture {}",
                    texture.id()
                ))
            })?
            .clone();
        let layers = if native.textureType() == objc2_metal::MTLTextureType::TypeCube {
            6
        } else {
            native.arrayLength()
        };
        let format = formats::color_format(native.pixelFormat());
        (native, format, layers)
    };

    rhi_require(
        native.sampleCount() == 1,
        "texture transfer requires a single sample",
    )?;
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        subresource,
        native.width(),
        native.height(),
        native.mipmapLevelCount(),
        layers,
    )?;
    let needed = rhi_texture_upload_bytes(
        region,
        bytes_per_row,
        format,
        extent_width,
        extent_height,
        data.len(),
    )?;
    let staging = shared_staging_buffer(device, needed)?;
    // SAFETY: the staging buffer holds at least `needed` bytes and `data` has at
    // least that many, both live and distinct.
    unsafe {
        std::ptr::copy_nonoverlapping(
            data.as_ptr(),
            staging.contents().as_ptr().cast::<u8>(),
            needed,
        );
    }

    // Private (device-local) texture: blit staging -> texture on a one-shot
    // command buffer.
    let width = region.width as usize;
    let height = region.height as usize;
    let destination = native.clone();
    run_blit(device, |blit| {
        // SAFETY: the region and row pitch were validated against the mip
        // extent and the staging buffer length.
        unsafe {
            blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &staging,
                0,
                bytes_per_row,
                bytes_per_row * height,
                MTLSize { width, height, depth: 1 },
                &destination,
                subresource.layer,
                subresource.mip_level,
                MTLOrigin {
                    x: region.origin[0] as usize,
                    y: region.origin[1] as usize,
                    z: 0,
                },
            )
        };
        Ok(())
    })
}

pub fn read_texture_data(
    device: &mut MetalDeviceHandle,
    texture: Texture,
    width: usize,
    height: usize,
    bytes_per_row: usize,
    subresource: TextureSubresource,
    destination: &mut [u8],
) -> RhiResult<()> {
    let (native, format, layers) = {
        let registries = lock(&device.registries);
        let native = registries
            .textures
            .get(&texture.id())
            .ok_or_else(|| {
                RhiError::invalid_argument(format!(
                    "readTextureData: unknown texture {}",
                    texture.id()
                ))
            })?
            .clone();
        let layers = if native.textureType() == objc2_metal::MTLTextureType::TypeCube {
            6
        } else {
            native.arrayLength()
        };
        let format = formats::color_format(native.pixelFormat());
        (native, format, layers)
    };

    rhi_require(
        native.sampleCount() == 1,
        "texture transfer requires a single sample",
    )?;
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        subresource,
        native.width(),
        native.height(),
        native.mipmapLevelCount(),
        layers,
    )?;
    let needed = rhi_texture_transfer_bytes(
        width,
        height,
        bytes_per_row,
        format,
        extent_width,
        extent_height,
        destination.len(),
    )?;
    rhi_require(
        destination.len() >= needed,
        "readTextureData: destination buffer too small",
    )?;
    let staging = shared_staging_buffer(device, needed)?;

    run_blit(device, |blit| {
        // SAFETY: the copy extent and destination pitch were validated against
        // the mip extent and the destination length.
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                &native,
                subresource.layer,
                subresource.mip_level,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width, height, depth: 1 },
                &staging,
                0,
                bytes_per_row,
                needed,
            )
        };
        Ok(())
    })?;

    // SAFETY: the staging buffer holds at least `needed` bytes and `destination`
    // is a distinct live slice of at least that length.
    unsafe {
        std::ptr::copy_nonoverlapping(
            staging.contents().as_ptr().cast::<u8>(),
            destination.as_mut_ptr(),
            needed,
        );
    }
    Ok(())
}

// MARK: - Swapchain

pub fn configure_swapchain(
    device: &mut MetalDeviceHandle,
    handle: Swapchain,
    descriptor: crate::rhi::SurfaceDescriptor,
) -> RhiResult<()> {
    rhi_require(
        descriptor.kind == crate::rhi::NativeSurfaceKind::MetalLayer,
        "Metal requires a CAMetalLayer surface",
    )?;
    rhi_require(descriptor.native_handle != 0, "surface nativeHandle is nil")?;
    rhi_require(
        matches!(
            descriptor.color_format,
            crate::rhi::TextureFormat::Bgra8Unorm
                | crate::rhi::TextureFormat::Bgra8UnormSrgb
                | crate::rhi::TextureFormat::Rgba16Float
        ),
        "CAMetalLayer requires BGRA8, BGRA8 sRGB or RGBA16 float",
    )?;
    // SAFETY: the frontend hands over an unmanaged `CAMetalLayer` that the
    // caller keeps alive until the device releases the surface.
    let layer = unsafe {
        Retained::retain(descriptor.native_handle as *mut objc2_quartz_core::CAMetalLayer)
            .ok_or_else(|| {
                RhiError::invalid_argument("surface nativeHandle is not a CAMetalLayer")
            })?
    };
    let layer_pointer = Retained::as_ptr(&layer);
    rhi_require(
        !device.swapchains.iter().any(|(id, window)| {
            *id != handle.id() && Retained::as_ptr(&window.layer) == layer_pointer
        }),
        "CAMetalLayer already has a swapchain",
    )?;
    rhi_require(
        device
            .swapchains
            .get(&handle.id())
            .is_none_or(|window| window.current_drawable.is_none()),
        "present an acquired drawable before resizing",
    )?;
    let generation = device
        .swapchains
        .get(&handle.id())
        .map_or(0, |window| window.generation)
        + 1;
    destroy_swapchain(device, handle);

    layer.setDevice(Some(&device.device));
    layer.setPixelFormat(formats::pixel_format(descriptor.color_format));
    layer::set_colorspace(&layer, descriptor.color_format);
    layer.setFramebufferOnly(true);
    // `NSSize` is the Foundation spelling of `CGSize`, so it needs no extra
    // core-foundation dependency for this one field.
    layer.setDrawableSize(objc2_foundation::NSSize {
        width: descriptor.width as f64,
        height: descriptor.height as f64,
    });
    // Display sync exists on macOS only; iOS always presents via the compositor.
    #[cfg(target_os = "macos")]
    layer.setDisplaySyncEnabled(descriptor.vsync_enabled);
    #[cfg(not(target_os = "macos"))]
    let _ = descriptor.vsync_enabled;

    device
        .swapchains
        .insert(handle.id(), MetalWindowSwapchain::new(layer, generation));
    Ok(())
}

pub fn presentation_mode(
    device: &MetalDeviceHandle,
    handle: Swapchain,
) -> RhiResult<crate::rhi::PresentationMode> {
    let window = device
        .swapchains
        .get(&handle.id())
        .ok_or_else(|| RhiError::invalid_argument("query a live swapchain"))?;
    #[cfg(target_os = "macos")]
    {
        Ok(if window.layer.displaySyncEnabled() {
            crate::rhi::PresentationMode::Fifo
        } else {
            crate::rhi::PresentationMode::Immediate
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        // iOS presents through its own display link, so the cadence is fixed.
        let _ = window;
        Ok(crate::rhi::PresentationMode::Fifo)
    }
}

pub fn destroy_swapchain(device: &mut MetalDeviceHandle, handle: Swapchain) {
    let Some(window) = device.swapchains.remove(&handle.id()) else {
        return;
    };
    // Drawable textures are backend-internal handles; drop their records too.
    let ids: Vec<u32> = window.texture_ids.values().copied().collect();
    {
        let mut registries = lock(&device.registries);
        for id in ids {
            registries.textures.remove(&id);
        }
    }
    drop(window);
}

pub fn acquire_swapchain_image(
    device: &mut MetalDeviceHandle,
    handle: Swapchain,
) -> RhiResult<SwapchainImage> {
    let active_slot = device.active_slot;
    // One ID per frame slot keeps both resource and planner history bounded.
    let internal_id = {
        let mut registries = lock(&device.registries);
        registries.next_internal_id()
    };
    let registries_handle = std::sync::Arc::clone(&device.registries);
    let Some(window) = device.swapchains.get_mut(&handle.id()) else {
        return Err(RhiError::SwapchainAcquireFailed(
            "configure a swapchain and return its acquired image first".into(),
        ));
    };
    if window.current_drawable.is_some() {
        return Err(RhiError::SwapchainAcquireFailed(
            "configure a swapchain and return its acquired image first".into(),
        ));
    }
    let drawable = window
        .layer
        .nextDrawable()
        .ok_or_else(|| RhiError::SwapchainAcquireFailed("nextDrawable returned nil".into()))?;
    // The frontend has already waited for this slot's render and present work.
    let id = *window.texture_ids.entry(active_slot).or_insert(internal_id);
    let texture = objc2_quartz_core::CAMetalDrawable::texture(&*drawable);
    let (width, height) = (texture.width(), texture.height());
    lock(&registries_handle).textures.insert(id, texture);
    window.current_texture_id = Some(id);
    window.current_drawable = Some(drawable);
    Ok(SwapchainImage::new(
        handle,
        window.generation,
        Texture::new(id),
        width,
        height,
    ))
}

pub fn present(
    device: &mut MetalDeviceHandle,
    image: SwapchainImage,
    completion: crate::rhi::Completion,
) -> RhiResult<()> {
    let Some(window) = device.swapchains.get_mut(&image.swapchain.id()) else {
        return Err(RhiError::present_failed(
            "image was not acquired from this swapchain",
        ));
    };
    let Some(drawable) = window.current_drawable.take() else {
        return Err(RhiError::present_failed(
            "image was not acquired from this swapchain",
        ));
    };
    if window.current_texture_id != Some(image.texture.id()) {
        window.current_drawable = Some(drawable);
        return Err(RhiError::present_failed(
            "image was not acquired from this swapchain",
        ));
    }
    let commands = device
        .graphics_queue
        .commandBuffer()
        .ok_or(RhiError::OutOfMemory)?;
    add_completion_handler(
        &commands,
        std::sync::Arc::clone(&device.submission_status),
        completion,
    );
    // Queue order places presentation after all preceding scene and UI work.
    commands.presentDrawable(as_drawable(&drawable));
    commands.commit();
    window.current_drawable = None;
    Ok(())
}

// MARK: - Frame uploader

pub fn make_frame_uploader(
    device: &mut MetalDeviceHandle,
    slot: usize,
) -> RhiResult<Box<dyn crate::rhi::FrameUploader>> {
    device.active_slot = slot;
    // Swift vends the same uploader object for a slot for the slot's lifetime,
    // but the frontend owns it exclusively, so this returns a fresh handle onto
    // the same ring state and resets it for the new frame.
    let uploader = if let Some(existing) = device.uploaders.get(&slot) {
        let mut guard = existing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::rhi::FrameUploader::reset(&mut *guard);
        std::sync::Arc::clone(existing)
    } else {
        let device_handle = device.device.clone();
        let registries_handle = std::sync::Arc::clone(&device.registries);
        let uploader = MetalFrameUploader::new(UPLOAD_CHUNK_SIZE, move |capacity| {
            let buffer = device_handle
                .newBufferWithLength_options(capacity, MTLResourceOptions::StorageModeShared)
                .ok_or(RhiError::OutOfMemory)?;
            let handle = Buffer::new(lock(&registries_handle).next_internal_id());
            lock(&registries_handle)
                .buffers
                .insert(handle.id(), buffer.clone());
            Ok(MetalUploadChunk::new(buffer, handle))
        });
        let uploader = std::sync::Arc::new(std::sync::Mutex::new(uploader));
        device
            .uploaders
            .insert(slot, std::sync::Arc::clone(&uploader));
        uploader
    };
    Ok(Box::new(SharedFrameUploader { uploader }))
}

/// Presents the slot's shared uploader through the exclusive `FrameUploader`
/// handle the frontend owns.
pub struct SharedFrameUploader {
    uploader: std::sync::Arc<std::sync::Mutex<MetalFrameUploader>>,
}

impl crate::rhi::FrameUploader for SharedFrameUploader {
    fn reset(&mut self) {
        let mut guard = self
            .uploader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::rhi::FrameUploader::reset(&mut *guard);
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<crate::rhi::UploadLocation> {
        let mut guard = self
            .uploader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::rhi::FrameUploader::write(&mut *guard, bytes, alignment)
    }
}

// MARK: - Idle

pub fn wait_until_idle(device: &mut MetalDeviceHandle) -> RhiResult<()> {
    // Command queues execute FIFO, so committing a no-op buffer and waiting on it
    // guarantees every previously queued buffer finished.
    if let Some(graphics) = device.graphics_queue.commandBuffer() {
        graphics.commit();
        graphics.waitUntilCompleted();
    }
    let graphics_pointer = Retained::as_ptr(&device.graphics_queue);
    if Retained::as_ptr(&device.compute_queue) != graphics_pointer
        && let Some(compute) = device.compute_queue.commandBuffer()
    {
        commit_and_wait(&compute);
    }
    device.submission_status.check()
}

fn commit_and_wait(buffer: &ProtocolObject<dyn MTLCommandBuffer>) {
    buffer.commit();
    buffer.waitUntilCompleted();
}

/// Compilation entry point shared by the immediate and background paths.
pub fn prepare_compilation(
    device: &MetalDeviceHandle,
    request: crate::rhi::RhiCompilationRequest,
) -> RhiResult<RhiCompilationWork> {
    use crate::rhi::RhiCompilationRequest;
    match request {
        RhiCompilationRequest::Shader(handle, descriptor) => {
            super::resources::prepare_shader_compilation(device, handle, *descriptor)
        }
        RhiCompilationRequest::Graphics(handle, descriptor) => {
            super::resources::prepare_graphics_compilation(device, handle, *descriptor)
        }
        RhiCompilationRequest::Compute(handle, descriptor) => {
            super::resources::prepare_compute_compilation(device, handle, *descriptor)
        }
        RhiCompilationRequest::Mesh(handle, descriptor) => {
            super::resources::prepare_mesh_compilation(device, handle, *descriptor)
        }
    }
}

/// Registers `completion` to run once Metal retires `command_buffer`.
///
/// The callback also records the command buffer's error on the submission
/// status, so a GPU failure reaches the API thread at `wait_until_idle` instead
/// of being lost. Metal copies the block when it is added, so the local
/// `RcBlock` can be dropped as soon as the call returns.
pub fn add_completion_handler(
    command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    status: std::sync::Arc<super::submission_status::MetalSubmissionStatus>,
    completion: crate::rhi::Completion,
) {
    // The handler is a `Fn`, so the one-shot completion is taken out of an
    // owner slot on first (and only) invocation, mirroring Swift's
    // `MetalCompletionCallback`.
    let owned = std::sync::Arc::new(std::sync::Mutex::new(Some(completion)));
    let block = block2::RcBlock::new(
        move |buffer: core::ptr::NonNull<ProtocolObject<dyn MTLCommandBuffer>>| {
            // SAFETY: Metal passes a live command buffer for the duration of the
            // callback.
            let buffer = unsafe { buffer.as_ref() };
            if let Some(error) = buffer.error() {
                status.record(error.localizedDescription().to_string());
            }
            let taken = owned
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(completion) = taken {
                completion();
            }
        },
    );
    // SAFETY: `as_ptr` yields the `*mut DynBlock` shape `MTLCommandBufferHandler`
    // declares, and `addCompletedHandler:` copies the block, so the pointer never
    // outlives this statement as far as the caller's `block` is concerned.
    unsafe {
        command_buffer.addCompletedHandler(block2::RcBlock::as_ptr(&block) as *mut _);
    }
}

/// Re-types a `CAMetalDrawable` reference as its parent `MTLDrawable` protocol,
/// which is the type `presentDrawable:` accepts.
pub fn as_drawable(
    drawable: &ProtocolObject<dyn objc2_quartz_core::CAMetalDrawable>,
) -> &ProtocolObject<dyn objc2_metal::MTLDrawable> {
    // SAFETY: `CAMetalDrawable` refines `MTLDrawable`, so the same object
    // pointer is a valid `MTLDrawable` object and only the static type changes.
    unsafe {
        &*(drawable as *const ProtocolObject<dyn objc2_quartz_core::CAMetalDrawable>
            as *const ProtocolObject<dyn objc2_metal::MTLDrawable>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_upload_chunk_size_matches_the_swift_default() {
        assert_eq!(UPLOAD_CHUNK_SIZE, 4 * 1024 * 1024);
    }
}

/// The compositor reads a layer through its colour space. Eight-bit layers hold
/// sRGB-encoded values; a float layer holds the frame's linear premultiplied
/// colour, which only an extended linear space reads back unchanged — reading
/// it as encoded sRGB would brighten every translucent pixel.
mod layer {
    use objc2::encode::{Encoding, RefEncode};
    use objc2::msg_send;
    use objc2_quartz_core::CAMetalLayer;
    use std::ffi::c_void;

    #[repr(C)]
    pub struct CGColorSpace {
        _private: [u8; 0],
    }
    // SAFETY: `CGColorSpaceRef` is `struct CGColorSpace *`.
    unsafe impl RefEncode for CGColorSpace {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGColorSpace", &[]));
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        static kCGColorSpaceSRGB: *const c_void;
        static kCGColorSpaceExtendedLinearSRGB: *const c_void;
        fn CGColorSpaceCreateWithName(name: *const c_void) -> *mut CGColorSpace;
        fn CGColorSpaceRelease(space: *mut CGColorSpace);
    }

    pub fn set_colorspace(layer: &CAMetalLayer, format: crate::rhi::TextureFormat) {
        // SAFETY: the names are CoreGraphics constants; the created space is
        // retained by the layer and released here once.
        unsafe {
            let name = if format == crate::rhi::TextureFormat::Rgba16Float {
                kCGColorSpaceExtendedLinearSRGB
            } else {
                kCGColorSpaceSRGB
            };
            let space = CGColorSpaceCreateWithName(name);
            if space.is_null() {
                return;
            }
            let _: () = msg_send![layer, setColorspace: space];
            CGColorSpaceRelease(space);
        }
    }
}
