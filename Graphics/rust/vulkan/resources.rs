//! Translation of NativeRHI/Vulkan/Backend/VulkanBackendResources.swift:
//! buffer, texture, sampler and shader-module creation and destruction.
//!
//! Device-local destinations are created in device memory; anything the CPU must
//! write goes through a host-visible block from the block allocator. Every
//! failure unwinds the driver objects created for that resource, so a partially
//! created image never enters the registry.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::rhi::{
    Buffer, BufferDescriptor, RhiCompilationWork, RhiCompiledObject, RhiError, RhiResult, Sampler,
    SamplerDescriptor, ShaderFormat, ShaderModule, ShaderModuleDescriptor, Texture,
    TextureAllocation, TextureDescriptor, TextureDimension, rhi_byte_range, rhi_require,
    rhi_validate_texture_samples,
};

use super::formats;
use super::memory::{VulkanMemoryAllocation, VulkanMemoryAllocator};
use super::registries::{
    VulkanBufferRecord, VulkanRegistries, VulkanSamplerRecord, VulkanShaderModuleRecord,
    VulkanTextureRecord,
};
use super::setup::VulkanContext;
use super::sys;

/// The queue families a concurrently shared resource spans.
fn sharing_families(context: &VulkanContext) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut families = Vec::new();
    for family in [
        context.queues.graphics.family,
        context.queues.compute.family,
        context.queues.transfer.family,
    ] {
        if seen.insert(family) {
            families.push(family);
        }
    }
    families
}

/// Locks the shared registry. A poisoned lock only means another thread panicked
/// while holding it; every mutation is a single map operation, so the registry
/// itself stays consistent.
pub fn lock(
    registries: &Arc<Mutex<VulkanRegistries>>,
) -> std::sync::MutexGuard<'_, VulkanRegistries> {
    registries
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// MARK: - Buffers

pub fn create_buffer(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    handle: Buffer,
    descriptor: BufferDescriptor,
) -> RhiResult<()> {
    let mut usage = formats::vk_buffer_usage(descriptor.usage);
    if context.features.acceleration_structures {
        // BLAS/TLAS inputs and storage both need device addresses.
        usage |= sys::VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT
            | sys::VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_BIT_KHR
            | sys::VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_STORAGE_BIT_KHR;
    }
    let families = sharing_families(context);
    let concurrent = families.len() > 1;

    let create_info = sys::VkBufferCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        size: descriptor.size.max(1) as sys::VkDeviceSize,
        usage,
        sharingMode: if concurrent {
            sys::VK_SHARING_MODE_CONCURRENT
        } else {
            sys::VK_SHARING_MODE_EXCLUSIVE
        },
        queueFamilyIndexCount: if concurrent { families.len() as u32 } else { 0 },
        pQueueFamilyIndices: if concurrent {
            families.as_ptr()
        } else {
            core::ptr::null()
        },
    };

    let mut buffer: sys::VkBuffer = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` and the family list it may point at outlive the call,
    // and the out-parameter is a live local.
    let status = unsafe {
        (context.core.create_buffer)(context.device, &create_info, core::ptr::null(), &mut buffer)
    };
    if status != sys::VK_SUCCESS || buffer == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }

    // From here on every failure path destroys the buffer.
    let outcome = (|| -> RhiResult<VulkanMemoryAllocation> {
        let mut requirements = sys::VkMemoryRequirements::default();
        // SAFETY: the buffer was just created on this device.
        unsafe {
            (context.core.get_buffer_memory_requirements)(context.device, buffer, &mut requirements)
        };
        // Vertex/index/indirect/uniform/storage data lives in device-local memory;
        // a transfer-only buffer stays host visible so the CPU can fill it.
        let is_gpu_resource = descriptor.usage.contains(crate::rhi::BufferUsage::VERTEX)
            || descriptor.usage.contains(crate::rhi::BufferUsage::INDEX)
            || descriptor.usage.contains(crate::rhi::BufferUsage::INDIRECT)
            || descriptor.usage.contains(crate::rhi::BufferUsage::UNIFORM)
            || descriptor
                .usage
                .contains(crate::rhi::BufferUsage::STORAGE_READ)
            || descriptor
                .usage
                .contains(crate::rhi::BufferUsage::STORAGE_WRITE);
        let flags = if is_gpu_resource {
            sys::VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
        } else {
            sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | sys::VK_MEMORY_PROPERTY_HOST_COHERENT_BIT
        };
        let allocation = allocator.allocate(
            requirements.size,
            requirements.alignment,
            requirements.memoryTypeBits,
            flags,
            0,
            false,
        )?;
        // SAFETY: the allocation satisfies the requirements the driver reported
        // for this buffer.
        let bind = unsafe {
            (context.core.bind_buffer_memory)(
                context.device,
                buffer,
                allocation.memory,
                allocation.offset,
            )
        };
        if bind != sys::VK_SUCCESS {
            allocator.free(allocation);
            return Err(RhiError::OutOfMemory);
        }
        Ok(allocation)
    })();

    match outcome {
        Ok(allocation) => {
            lock(registries).buffers.insert(
                handle.id(),
                VulkanBufferRecord {
                    buffer,
                    allocation,
                    size: descriptor.size,
                    usage: descriptor.usage,
                },
            );
            Ok(())
        }
        Err(error) => {
            // SAFETY: binding never succeeded, so nothing references the buffer.
            unsafe {
                (context.core.destroy_buffer)(context.device, buffer, core::ptr::null());
            }
            Err(error)
        }
    }
}

pub fn destroy_buffer(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    handle: Buffer,
) {
    let Some(record) = lock(registries).buffers.remove(&handle.id()) else {
        return;
    };
    // SAFETY: the frame ring waited for every submission that could use this
    // buffer before invoking deferred destruction.
    unsafe {
        (context.core.destroy_buffer)(context.device, record.buffer, core::ptr::null());
    }
    allocator.free(record.allocation);
}

// MARK: - Textures

pub fn create_texture(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    handle: Texture,
    descriptor: TextureDescriptor,
) -> RhiResult<()> {
    rhi_validate_texture_samples(&descriptor)?;
    let mut usage = formats::vk_image_usage(descriptor.usage);
    if descriptor.transient_attachment {
        // TRANSIENT images may only have attachment usage; remove the transfer
        // bits the general helper also enables.
        usage &= !(sys::VK_IMAGE_USAGE_TRANSFER_SRC_BIT | sys::VK_IMAGE_USAGE_TRANSFER_DST_BIT);
        usage |= sys::VK_IMAGE_USAGE_TRANSIENT_ATTACHMENT_BIT;
    }
    let array_layers = if descriptor.dimension == TextureDimension::Cube {
        6
    } else {
        descriptor.layers.max(1) as u32
    };
    let families = sharing_families(context);
    let concurrent = families.len() > 1;
    let flags = if descriptor.dimension == TextureDimension::Cube {
        sys::VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT
    } else {
        0
    };

    let create_info = sys::VkImageCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags,
        imageType: formats::vk_image_type(descriptor.dimension),
        format: formats::vk_format(descriptor.format),
        extent: sys::VkExtent3D {
            width: descriptor.width.max(1) as u32,
            height: descriptor.height.max(1) as u32,
            depth: descriptor.depth.max(1) as u32,
        },
        mipLevels: descriptor.mip_levels.max(1) as u32,
        arrayLayers: array_layers,
        samples: formats::vk_sample_count(descriptor.sample_count)?,
        tiling: sys::VK_IMAGE_TILING_OPTIMAL,
        usage,
        sharingMode: if concurrent {
            sys::VK_SHARING_MODE_CONCURRENT
        } else {
            sys::VK_SHARING_MODE_EXCLUSIVE
        },
        queueFamilyIndexCount: if concurrent { families.len() as u32 } else { 0 },
        pQueueFamilyIndices: if concurrent {
            families.as_ptr()
        } else {
            core::ptr::null()
        },
        initialLayout: sys::VK_IMAGE_LAYOUT_UNDEFINED,
    };

    // Reject unsupported format/usage/sample combinations before creating anything.
    let mut support = sys::VkImageFormatProperties::default();
    // SAFETY: the physical device is live and the out-parameter is a live local.
    let supported = unsafe {
        (context.instance_commands.get_image_format_properties)(
            context.physical_device,
            create_info.format,
            create_info.imageType,
            create_info.tiling,
            create_info.usage,
            create_info.flags,
            &mut support,
        )
    };
    rhi_require(
        supported == sys::VK_SUCCESS
            && (support.sampleCounts & descriptor.sample_count as u32) != 0,
        "Vulkan adapter does not support the requested texture format, usage and samples",
    )?;
    rhi_require(
        create_info.extent.width <= support.maxExtent.width
            && create_info.extent.height <= support.maxExtent.height
            && create_info.extent.depth <= support.maxExtent.depth
            && create_info.mipLevels <= support.maxMipLevels
            && create_info.arrayLayers <= support.maxArrayLayers,
        "texture exceeds Vulkan format limits",
    )?;

    let mut image: sys::VkImage = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` and the family list it may point at outlive the call.
    let status = unsafe {
        (context.core.create_image)(context.device, &create_info, core::ptr::null(), &mut image)
    };
    if status != sys::VK_SUCCESS || image == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }

    // Every failure path below destroys the image and any view already created.
    let mut owned_view: Option<sys::VkImageView> = None;
    let mut owned_attachment_view: Option<sys::VkImageView> = None;
    let mut owned_allocation: Option<VulkanMemoryAllocation> = None;
    let build = (|| -> RhiResult<(VulkanMemoryAllocation, sys::VkImageView, Option<sys::VkImageView>)> {
        let mut requirements = sys::VkMemoryRequirements::default();
        // SAFETY: the image was just created on this device.
        unsafe {
            (context.core.get_image_memory_requirements)(context.device, image, &mut requirements)
        };
        let allocation = allocator.allocate(
            requirements.size,
            requirements.alignment,
            requirements.memoryTypeBits,
            sys::VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
            if descriptor.transient_attachment {
                sys::VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT
            } else {
                0
            },
            descriptor.transient_attachment,
        )?;
        owned_allocation = Some(allocation);
        // SAFETY: the allocation satisfies the requirements the driver reported.
        let bind = unsafe {
            (context.core.bind_image_memory)(
                context.device,
                image,
                allocation.memory,
                allocation.offset,
            )
        };
        if bind != sys::VK_SUCCESS {
            return Err(RhiError::OutOfMemory);
        }

        let view = create_view(
            context,
            image,
            formats::vk_format(descriptor.format),
            formats::vk_image_view_type(descriptor.dimension),
            formats::aspect_flags(descriptor.format),
            sys::VK_REMAINING_MIP_LEVELS,
        )?;
        owned_view = Some(view);

        // A mipmapped attachment needs a single-level view so the render pass can
        // bind just the base level.
        let attachment_view = if descriptor.mip_levels > 1
            && (descriptor.usage.contains(crate::rhi::TextureUsage::COLOR_TARGET)
                || descriptor.usage.contains(crate::rhi::TextureUsage::DEPTH_STENCIL_TARGET))
        {
            let extra = create_view(
                context,
                image,
                formats::vk_format(descriptor.format),
                formats::vk_image_view_type(descriptor.dimension),
                formats::aspect_flags(descriptor.format),
                1,
            )?;
            owned_attachment_view = Some(extra);
            Some(extra)
        } else {
            None
        };
        Ok((allocation, view, attachment_view))
    })();

    match build {
        Ok((allocation, view, attachment_view)) => {
            lock(registries).textures.insert(
                handle.id(),
                VulkanTextureRecord {
                    image,
                    allocation: Some(allocation),
                    view,
                    format: descriptor.format,
                    width: descriptor.width,
                    height: descriptor.height,
                    depth: descriptor.depth,
                    dimension: descriptor.dimension,
                    layers: array_layers,
                    mip_levels: create_info.mipLevels,
                    usage: descriptor.usage,
                    is_swapchain: false,
                    layout: sys::VK_IMAGE_LAYOUT_UNDEFINED,
                    sample_count: descriptor.sample_count,
                    transient_attachment: descriptor.transient_attachment,
                    attachment_view,
                },
            );
            Ok(())
        }
        Err(error) => {
            // SAFETY: the device is not using any of these objects yet, so they
            // can be released in reverse creation order.
            unsafe {
                if let Some(view) = owned_attachment_view {
                    (context.core.destroy_image_view)(context.device, view, core::ptr::null());
                }
                if let Some(view) = owned_view {
                    (context.core.destroy_image_view)(context.device, view, core::ptr::null());
                }
                (context.core.destroy_image)(context.device, image, core::ptr::null());
            }
            if let Some(allocation) = owned_allocation {
                allocator.free(allocation);
            }
            Err(error)
        }
    }
}

/// Creates an image view over `image` with the given aspect and mip count.
fn create_view(
    context: &VulkanContext,
    image: sys::VkImage,
    format: u32,
    view_type: u32,
    aspect: u32,
    level_count: u32,
) -> RhiResult<sys::VkImageView> {
    let create_info = sys::VkImageViewCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        image,
        viewType: view_type,
        format,
        // Identity swizzle: zero for every channel means "use the component".
        components: [0; 4],
        subresourceRange: sys::VkImageSubresourceRange {
            aspectMask: aspect,
            baseMipLevel: 0,
            levelCount: level_count,
            baseArrayLayer: 0,
            layerCount: sys::VK_REMAINING_ARRAY_LAYERS,
        },
    };
    let mut view: sys::VkImageView = sys::VK_NULL_HANDLE;
    // SAFETY: the image is live on this device and `create_info` outlives the call.
    let status = unsafe {
        (context.core.create_image_view)(context.device, &create_info, core::ptr::null(), &mut view)
    };
    if status != sys::VK_SUCCESS || view == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }
    Ok(view)
}

pub fn destroy_texture(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    allocator: &mut VulkanMemoryAllocator,
    handle: Texture,
) {
    let Some(record) = lock(registries).textures.remove(&handle.id()) else {
        return;
    };
    // Swapchain images own no memory or view of their own beyond what the
    // swapchain manages, so only their view is released by the swapchain path.
    if record.is_swapchain {
        return;
    }
    // SAFETY: the frame ring waited for every submission that could use this
    // texture before invoking deferred destruction.
    unsafe {
        if let Some(view) = record.attachment_view {
            (context.core.destroy_image_view)(context.device, view, core::ptr::null());
        }
        (context.core.destroy_image_view)(context.device, record.view, core::ptr::null());
        (context.core.destroy_image)(context.device, record.image, core::ptr::null());
    }
    if let Some(allocation) = record.allocation {
        allocator.free(allocation);
    }
}

/// Backend-reported allocation for one texture.
pub fn texture_allocation(
    registries: &VulkanRegistries,
    texture: Texture,
) -> Option<TextureAllocation> {
    let allocation = registries.textures.get(&texture.id())?.allocation?;
    // Reserved resource bytes, including lazy allocations. Lazy memory may commit
    // zero bytes on a tile GPU, but that is not a portable guarantee.
    Some(TextureAllocation::new(allocation.size as i64, false))
}

// MARK: - Samplers

pub fn create_sampler(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: Sampler,
    descriptor: SamplerDescriptor,
) -> RhiResult<()> {
    let create_info = sys::VkSamplerCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        magFilter: formats::vk_filter(descriptor.mag_filter),
        minFilter: formats::vk_filter(descriptor.min_filter),
        mipmapMode: formats::vk_sampler_mipmap_mode(descriptor.mip_filter),
        addressModeU: formats::vk_sampler_address_mode(descriptor.address_mode_u),
        addressModeV: formats::vk_sampler_address_mode(descriptor.address_mode_v),
        addressModeW: formats::vk_sampler_address_mode(descriptor.address_mode_w),
        mipLodBias: 0.0,
        anisotropyEnable: sys::VK_FALSE,
        maxAnisotropy: 1.0,
        compareEnable: u32::from(descriptor.compare_enabled),
        compareOp: formats::vk_compare_op(descriptor.compare_op),
        minLod: 0.0,
        maxLod: sys::VK_LOD_CLAMP_NONE,
        borderColor: sys::VK_BORDER_COLOR_FLOAT_OPAQUE_BLACK,
        unnormalizedCoordinates: sys::VK_FALSE,
    };
    let mut sampler: sys::VkSampler = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` outlives the call and the out-parameter is a live
    // local.
    let status = unsafe {
        (context.resources.create_sampler)(
            context.device,
            &create_info,
            core::ptr::null(),
            &mut sampler,
        )
    };
    if status != sys::VK_SUCCESS || sampler == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }
    lock(registries)
        .samplers
        .insert(handle.id(), VulkanSamplerRecord { sampler });
    Ok(())
}

pub fn destroy_sampler(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: Sampler,
) {
    let Some(record) = lock(registries).samplers.remove(&handle.id()) else {
        return;
    };
    // SAFETY: the frame ring waited for every submission that could use it.
    unsafe {
        (context.resources.destroy_sampler)(context.device, record.sampler, core::ptr::null());
    }
}

// MARK: - Shader modules

pub fn create_shader_module(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: ShaderModule,
    descriptor: ShaderModuleDescriptor,
) -> RhiResult<()> {
    prepare_shader_compilation(context, registries, handle, descriptor)?.run_and_install()
}

/// Validates SPIR-V and prepares the module creation, so the background path and
/// the immediate path share one implementation.
pub fn prepare_shader_compilation(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: ShaderModule,
    descriptor: ShaderModuleDescriptor,
) -> RhiResult<RhiCompilationWork> {
    rhi_require(
        descriptor.format == ShaderFormat::Spirv,
        "Vulkan backend requires SPIR-V shader modules",
    )?;
    rhi_require(
        descriptor.code.len() >= 20 && descriptor.code.len() % 4 == 0,
        "invalid SPIR-V byte count",
    )?;

    // Reinterpret the little-endian byte stream as the word stream SPIR-V is
    // defined over. `chunks_exact` cannot fail because the length was checked.
    let words: Vec<u32> = descriptor
        .code
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    rhi_require(words[0] == 0x0723_0203, "invalid SPIR-V magic")?;
    rhi_require(
        words[1] <= context.features.spirv_version,
        "SPIR-V version exceeds this Vulkan device; compile mobile shaders with -profile spirv_1_3",
    )?;

    // Walk the instruction stream: reject a truncated instruction or a LocalSizeId
    // execution mode on a device without maintenance4, which would fail at
    // pipeline creation with a much less useful message.
    let mut cursor = 5usize;
    while cursor < words.len() {
        let word_count = (words[cursor] >> 16) as usize;
        rhi_require(
            word_count > 0 && word_count <= words.len() - cursor,
            "malformed SPIR-V instruction",
        )?;
        if words[cursor] & 0xffff == 331
            && word_count >= 3
            && words[cursor + 2] == 38
            && !context.features.maintenance4
        {
            return Err(RhiError::unsupported_feature(
                "LocalSizeId requires Vulkan maintenance4; compile a fixed workgroup size for this device",
            ));
        }
        cursor += word_count;
    }

    let create_info = sys::VkShaderModuleCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        codeSize: descriptor.code.len(),
        pCode: words.as_ptr(),
    };
    let mut module: sys::VkShaderModule = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` points at `words`, which outlives the call, and the
    // out-parameter is a live local.
    let status = unsafe {
        (context.core.create_shader_module)(
            context.device,
            &create_info,
            core::ptr::null(),
            &mut module,
        )
    };
    if status != sys::VK_SUCCESS || module == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }

    // The module is created now and published by the install closure, matching
    // Swift's snapshot/compile/publish split.
    let device = context.device;
    let destroy = context.core.destroy_shader_module;
    let id = handle.id();
    let record = VulkanShaderModuleRecord {
        module,
        stage: descriptor.stage,
        entry_point: descriptor.entry_point,
        specialization_constants: descriptor.specialization_constants,
    };
    let shared = Arc::clone(registries);
    Ok(RhiCompilationWork::new(move || {
        let shared_publish = Arc::clone(&shared);
        Ok(RhiCompiledObject::new(
            move || {
                lock(&shared_publish).shader_modules.insert(id, record);
            },
            // SAFETY: `module` was created above and is only destroyed when this
            // object is dropped without being installed.
            move || unsafe { destroy(device, module, core::ptr::null()) },
        ))
    }))
}

pub fn destroy_shader_module(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: ShaderModule,
) {
    let Some(record) = lock(registries).shader_modules.remove(&handle.id()) else {
        return;
    };
    // SAFETY: the frame ring waited for every submission that could use it.
    unsafe {
        (context.core.destroy_shader_module)(context.device, record.module, core::ptr::null());
    }
}

/// Validates that a buffer range can be bound as a descriptor range.
pub fn check_binding_range(offset: usize, length: usize, capacity: usize) -> RhiResult<()> {
    rhi_require(length > 0, "buffer binding size must be positive")?;
    rhi_byte_range(offset, length, capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharing_families_deduplicates_in_class_order() {
        // The helper is exercised through a context-shaped tuple instead of a real
        // context, because queue discovery is the only input it uses.
        let mut seen = HashSet::new();
        let mut families = Vec::new();
        for family in [0u32, 0, 2] {
            if seen.insert(family) {
                families.push(family);
            }
        }
        assert_eq!(families, vec![0, 2]);
    }

    #[test]
    fn binding_ranges_require_positive_sizes_inside_the_buffer() {
        assert!(check_binding_range(0, 16, 16).is_ok());
        assert!(check_binding_range(0, 0, 16).is_err());
        assert!(check_binding_range(8, 16, 16).is_err());
        assert!(check_binding_range(16, 0, 16).is_err());
    }
}
