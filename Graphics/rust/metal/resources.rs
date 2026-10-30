//! Translation of NativeRHI/Metal/Device/MetalDeviceResources.swift and
//! Metal/Pipelines/MetalMeshPipeline.swift: resource, shader and pipeline
//! creation plus deferred destruction.
//!
//! Compilation goes through [`RhiCompilationWork`] so the background path and
//! the immediate path share one implementation: the driver call happens inside
//! the closure, and publishing into the registries only happens once it
//! succeeded. The registries are shared by reference (as Swift shares its
//! `MetalRegistries` class), which is what lets an install closure publish
//! without borrowing the device.
use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLAllocation, MTLComputePipelineState, MTLDataType, MTLDepthStencilDescriptor,
    MTLDevice, MTLFunctionConstantValues, MTLLibrary, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResource, MTLResourceOptions, MTLSamplerDescriptor, MTLStorageMode,
    MTLTextureDescriptor, MTLVertexDescriptor,
};
use objc2_foundation::NSString;

use crate::rhi::{
    Buffer, BufferDescriptor, ComputePipeline, ComputePipelineDescriptor, GraphicsPipeline,
    GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor, RhiCompiledObject,
    RhiCompilationWork, RhiError, RhiResult, Sampler, SamplerDescriptor, ShaderConstantType,
    ShaderModule, ShaderModuleDescriptor, ShaderStage, Texture, TextureAllocation, TextureDescriptor,
    TextureDimension, ThreadgroupSize, VertexLayoutDescriptor, rhi_require,
    rhi_validate_sample_count, rhi_validate_texture_samples,
};

use super::device::{MetalDeviceHandle, VERTEX_BUFFER_BASE_INDEX, label};
use super::formats;
use super::registries::{MetalGraphicsPipeline, MetalMeshPipeline, MetalRegistries, MetalShaderRecord};

/// Locks the shared registry. A poisoned lock only means another thread
/// panicked while holding it; the registry itself stays consistent because
/// every mutation is a single map operation.
pub fn lock(registries: &Arc<Mutex<MetalRegistries>>) -> std::sync::MutexGuard<'_, MetalRegistries> {
    registries.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// MARK: - Buffers

pub fn create_buffer(
    registries: &Arc<Mutex<MetalRegistries>>,
    device: &ProtocolObject<dyn objc2_metal::MTLDevice>,
    handle: Buffer,
    descriptor: BufferDescriptor,
) -> RhiResult<()> {
    rhi_require(descriptor.size > 0, "buffer size must be > 0")?;
    // Shared storage is CPU-writable and GPU/CPU coherent (Apple Silicon
    // uniform memory), so immediate uploads can memcpy into contents.
    let buffer = device
        .newBufferWithLength_options(descriptor.size, MTLResourceOptions::StorageModeShared)
        .ok_or(RhiError::OutOfMemory)?;
    if let Some(text) = label(descriptor.label) {
        buffer.setLabel(Some(&text));
    }
    lock(registries).buffers.insert(handle.id(), buffer);
    Ok(())
}

// MARK: - Textures

pub fn create_texture(
    registries: &Arc<Mutex<MetalRegistries>>,
    device: &ProtocolObject<dyn objc2_metal::MTLDevice>,
    handle: Texture,
    descriptor: TextureDescriptor,
) -> RhiResult<()> {
    rhi_validate_texture_samples(&descriptor)?;
    rhi_require(
        device.supportsTextureSampleCount(descriptor.sample_count),
        "Metal adapter does not support the requested texture sample count",
    )?;
    let native =  { MTLTextureDescriptor::new() };
    native.setPixelFormat(formats::pixel_format(descriptor.format));
    // SAFETY: every setter below takes an unsigned size the descriptor stores;
    // the frontend already validated the semantic combinations.
    unsafe { native.setWidth(descriptor.width.max(1)) };
    unsafe { native.setHeight(descriptor.height.max(1)) };
    unsafe { native.setDepth(descriptor.depth.max(1)) };
    native.setTextureType(if descriptor.sample_count > 1 {
        objc2_metal::MTLTextureType::Type2DMultisample
    } else {
        formats::texture_type(descriptor.dimension)
    });
    if descriptor.dimension == TextureDimension::Texture2DArray {
        unsafe { native.setArrayLength(descriptor.layers.max(1)) };
    }
    unsafe { native.setMipmapLevelCount(descriptor.mip_levels.max(1)) };
    unsafe { native.setSampleCount(descriptor.sample_count.max(1)) };
    native.setUsage(formats::texture_usage(descriptor.usage));
    // Ordinary textures use device-local storage; transient samples may remain
    // tile-only on Apple GPUs without changing their sample count.
    native.setStorageMode(
        if descriptor.transient_attachment
            && device.supportsFamily(objc2_metal::MTLGPUFamily::Apple1)
        {
            objc2_metal::MTLStorageMode::Memoryless
        } else {
            objc2_metal::MTLStorageMode::Private
        },
    );
    let texture = device.newTextureWithDescriptor(&native).ok_or(RhiError::OutOfMemory)?;
    if let Some(text) = label(descriptor.label) {
        texture.setLabel(Some(&text));
    }
    lock(registries).textures.insert(handle.id(), texture);
    Ok(())
}

// MARK: - Samplers

pub fn create_sampler(
    registries: &Arc<Mutex<MetalRegistries>>,
    device: &ProtocolObject<dyn objc2_metal::MTLDevice>,
    handle: Sampler,
    descriptor: SamplerDescriptor,
) -> RhiResult<()> {
    let native =  { MTLSamplerDescriptor::new() };
    native.setMinFilter(formats::sampler_min_mag_filter(descriptor.min_filter));
    native.setMagFilter(formats::sampler_min_mag_filter(descriptor.mag_filter));
    native.setMipFilter(formats::sampler_mip_filter(descriptor.mip_filter));
    native.setSAddressMode(formats::sampler_address_mode(descriptor.address_mode_u));
    native.setTAddressMode(formats::sampler_address_mode(descriptor.address_mode_v));
    native.setRAddressMode(formats::sampler_address_mode(descriptor.address_mode_w));
    if descriptor.compare_enabled {
        native.setCompareFunction(formats::compare_function(descriptor.compare_op));
    }
    let sampler = device.newSamplerStateWithDescriptor(&native).ok_or(RhiError::OutOfMemory)?;
    lock(registries).samplers.insert(handle.id(), sampler);
    Ok(())
}

// MARK: - Shader modules

pub fn create_shader_module(
    device: &MetalDeviceHandle,
    handle: ShaderModule,
    descriptor: ShaderModuleDescriptor,
) -> RhiResult<()> {
    prepare_shader_compilation(device, handle, descriptor)?.run_and_install()
}

pub fn prepare_shader_compilation(
    device: &MetalDeviceHandle,
    handle: ShaderModule,
    descriptor: ShaderModuleDescriptor,
) -> RhiResult<RhiCompilationWork> {
    // Compile hourly off the render lock, exactly as Swift does: the cache is
    // consulted and populated before the work object is returned.
    let library = device.library_cache.library(&descriptor)?;
    let entry_point = descriptor.entry_point.clone();
    let stage = descriptor.stage;
    let threadgroup_size = descriptor.threadgroup_size;
    let constants = descriptor.specialization_constants.clone();
    let registries = Arc::clone(&device.registries);
    Ok(RhiCompilationWork::new(move || {
        let name = NSString::from_str(&entry_point);
        let function = if constants.is_empty() {
            library.newFunctionWithName(&name)
        } else {
            let values =  { MTLFunctionConstantValues::new() };
            for constant in &constants {
                let data_type = match constant.value.constant_type() {
                    ShaderConstantType::Uint32 => MTLDataType::UInt,
                    ShaderConstantType::Int32 => MTLDataType::Int,
                    ShaderConstantType::Float32 => MTLDataType::Float,
                    ShaderConstantType::Bool => MTLDataType::Bool,
                };
                let bits = constant.value.bits();
                // SAFETY: `bits` is a live local holding exactly the 32-bit
                // representation the declared type consumes, and Metal copies
                // it during the call.
                let value = NonNull::new((&bits as *const u32).cast_mut().cast::<c_void>())
                    .expect("a local scalar is never null");
                unsafe {
                    values.setConstantValue_type_atIndex(value, data_type, constant.id as usize)
                };
            }
            library
                .newFunctionWithName_constantValues_error(&name, &values)
                .ok()
        };
        let function = function.ok_or_else(|| {
            RhiError::invalid_argument(format!(
                "entry point '{entry_point}' not found in the compiled library"
            ))
        })?;
        let record = MetalShaderRecord { function, library, stage, threadgroup_size };
        Ok(RhiCompiledObject::new(
            move || {
                lock(&registries).shaders.insert(handle.id(), record);
            },
            || {},
        ))
    }))
}

// MARK: - Vertex layout

/// Builds the native vertex descriptor for a backend-neutral layout. The
/// caller enforces Metal's attribute and buffer-layout limits first.
pub fn vertex_descriptor(layout: &VertexLayoutDescriptor) -> Retained<MTLVertexDescriptor> {
    let descriptor =  { MTLVertexDescriptor::new() };
    let attributes = descriptor.attributes();
    for attribute in layout.attributes.iter().take(32) {
        // SAFETY: the loop bound keeps the index inside the 31-entry array
        // Metal documents for vertex attributes.
        let native = unsafe { attributes.objectAtIndexedSubscript(attribute.location as usize) };
        native.setFormat(formats::vertex_format(attribute.format));
        // SAFETY: the offset and buffer index were validated against the
        // declared layout, and Metal bounds-checks the bind at draw time.
        unsafe {
            native.setOffset(attribute.offset);
            native.setBufferIndex(
                VERTEX_BUFFER_BASE_INDEX as usize + attribute.buffer_index as usize,
            );
        }
    }
    let layouts = descriptor.layouts();
    for (index, buffer_layout) in layout.buffer_layouts.iter().take(8).enumerate() {
        // SAFETY: `index` stays inside the eight-layout array Metal documents.
        let native = unsafe {
            layouts.objectAtIndexedSubscript(VERTEX_BUFFER_BASE_INDEX as usize + index)
        };
        // SAFETY: the stride is a validated positive byte count and a step rate
        // of one is the value Swift passes.
        unsafe {
            native.setStride(buffer_layout.stride);
            native.setStepRate(1);
        }
        native.setStepFunction(formats::vertex_step_function(buffer_layout.step_rate));
    }
    descriptor
}

// MARK: - Graphics pipelines

pub fn create_graphics_pipeline(
    device: &MetalDeviceHandle,
    handle: GraphicsPipeline,
    descriptor: GraphicsPipelineDescriptor,
) -> RhiResult<()> {
    prepare_graphics_compilation(device, handle, descriptor)?.run_and_install()
}

pub fn prepare_graphics_compilation(
    device: &MetalDeviceHandle,
    handle: GraphicsPipeline,
    descriptor: GraphicsPipelineDescriptor,
) -> RhiResult<RhiCompilationWork> {
    let vertex_function = {
        let registries = lock(&device.registries);
        registries
            .shaders
            .get(&descriptor.vertex.id())
            .map(|record| record.function.clone())
            .ok_or_else(|| {
                RhiError::invalid_argument("graphics pipeline: vertex shader module not found")
            })?
    };
    let fragment_function = match descriptor.fragment {
        Some(fragment) => {
            let registries = lock(&device.registries);
            Some(
                registries
                    .shaders
                    .get(&fragment.id())
                    .map(|record| record.function.clone())
                    .ok_or_else(|| {
                        RhiError::invalid_argument(
                            "graphics pipeline: fragment shader module not found",
                        )
                    })?,
            )
        }
        None => None,
    };

    rhi_validate_sample_count(descriptor.sample_count)?;
    rhi_require(
        device.device.supportsTextureSampleCount(descriptor.sample_count),
        "Metal adapter does not support the requested pipeline sample count",
    )?;

    let native =  { MTLRenderPipelineDescriptor::new() };
    native.setRasterSampleCount(descriptor.sample_count);
    native.setVertexFunction(Some(&vertex_function));
    native.setFragmentFunction(fragment_function.as_deref());

    // MRT: one color attachment per descriptor entry.
    let color_attachments = native.colorAttachments();
    for (index, attachment) in descriptor.color_attachments.iter().enumerate() {
        // SAFETY: Metal exposes eight color attachment slots and the frontend
        // limits a graphics pipeline to the same count.
        let native_attachment = unsafe { color_attachments.objectAtIndexedSubscript(index) };
        native_attachment.setPixelFormat(formats::pixel_format(attachment.format));
        let blend = attachment.blend;
        if blend.enabled {
            native_attachment.setBlendingEnabled(true);
            native_attachment
                .setSourceRGBBlendFactor(formats::blend_factor(blend.source_color_blend_factor));
            native_attachment.setDestinationRGBBlendFactor(formats::blend_factor(
                blend.destination_color_blend_factor,
            ));
            native_attachment
                .setRgbBlendOperation(formats::blend_operation(blend.color_blend_operation));
            native_attachment
                .setSourceAlphaBlendFactor(formats::blend_factor(blend.source_alpha_blend_factor));
            native_attachment.setDestinationAlphaBlendFactor(formats::blend_factor(
                blend.destination_alpha_blend_factor,
            ));
            native_attachment
                .setAlphaBlendOperation(formats::blend_operation(blend.alpha_blend_operation));
        }
    }

    if let Some(depth_format) = descriptor.depth_format {
        native.setDepthAttachmentPixelFormat(formats::pixel_format(depth_format));
    }
    if let Some(stencil_format) = descriptor.stencil_format {
        native.setStencilAttachmentPixelFormat(formats::pixel_format(stencil_format));
    }
    if let Some(text) = label(descriptor.label) {
        native.setLabel(Some(&text));
    }

    // Rasterization itself (cull/winding/fill) is encoder state in Metal; the
    // descriptor is stashed so the render encoder can apply it on bind.
    if let Some(layout) = &descriptor.vertex_layout {
        rhi_require(
            layout.attributes.len() <= 32,
            "graphics pipeline has more than 32 vertex attributes; limit is 32",
        )?;
        rhi_require(
            layout.buffer_layouts.len() <= 8,
            "graphics pipeline has more than 8 buffer layouts; limit is 8",
        )?;
        native.setVertexDescriptor(Some(&vertex_descriptor(layout)));
    }

    let native_device = device.device.clone();
    let registries = Arc::clone(&device.registries);
    Ok(RhiCompilationWork::new(move || {
        let state = native_device
            .newRenderPipelineStateWithDescriptor_error(&native)
            .map_err(|error| {
                RhiError::invalid_argument(format!(
                    "graphics pipeline failed: {}",
                    error.localizedDescription()
                ))
            })?;
        let depth_state = descriptor.depth_stencil.and_then(|depth_stencil| {
            let native_state =  { MTLDepthStencilDescriptor::new() };
            native_state.setDepthCompareFunction(formats::compare_function(
                depth_stencil.depth_compare,
            ));
            native_state.setDepthWriteEnabled(depth_stencil.depth_write_enabled);
            native_device.newDepthStencilStateWithDescriptor(&native_state)
        });
        let primitive = formats::primitive_type(descriptor.primitive);
        let rasterization = descriptor.rasterization;
        let depth_format_present = descriptor.depth_format.is_some();
        let id = handle.id();
        Ok(RhiCompiledObject::new(
            move || {
                let mut registries = lock(&registries);
                registries.render_pipelines.insert(
                    id,
                    MetalGraphicsPipeline { state, descriptor },
                );
                registries.pipeline_raster_states.insert(id, rasterization);
                registries.pipeline_primitives.insert(id, primitive);
                if let Some(depth_state) = depth_state {
                    registries.depth_states.insert(id, depth_state);
                }
            },
            move || {
                let _ = depth_format_present;
            },
        ))
    }))
}

// MARK: - Mesh pipelines

pub fn create_mesh_pipeline(
    device: &MetalDeviceHandle,
    handle: MeshPipeline,
    descriptor: MeshPipelineDescriptor,
) -> RhiResult<()> {
    prepare_mesh_compilation(device, handle, descriptor)?.run_and_install()
}

pub fn prepare_mesh_compilation(
    device: &MetalDeviceHandle,
    handle: MeshPipeline,
    descriptor: MeshPipelineDescriptor,
) -> RhiResult<RhiCompilationWork> {
    rhi_require(
        device.adapter_capabilities().mesh_shading,
        "adapter has no mesh shader support",
    )?;
    let (mesh_function, mesh_size) = {
        let registries = lock(&device.registries);
        let record = registries
            .shaders
            .get(&descriptor.mesh.id())
            .filter(|record| record.stage == ShaderStage::Mesh)
            .ok_or_else(|| RhiError::invalid_argument("mesh pipeline requires a mesh shader"))?;
        (record.function.clone(), record.threadgroup_size)
    };

    let native =  { objc2_metal::MTLMeshRenderPipelineDescriptor::new() };
    // SAFETY: the mesh function comes from this device's registry and outlives
    // the descriptor, which Metal reads only during pipeline creation.
    unsafe { native.setMeshFunction(Some(&mesh_function)) };
    if let Some(text) = label(descriptor.label) {
        native.setLabel(Some(&text));
    }

    let task_size = match descriptor.task {
        Some(task) => {
            rhi_require(
                device.capabilities.mesh_shading.task,
                "task shaders are not enabled in the current mesh implementation",
            )?;
            let registries = lock(&device.registries);
            let record = registries
                .shaders
                .get(&task.id())
                .filter(|record| record.stage == ShaderStage::Task)
                .ok_or_else(|| RhiError::invalid_argument("task shader stage mismatch"))?;
            let function = record.function.clone();
            let size = record.threadgroup_size;
            // SAFETY: the task function comes from this device's registry and
            // outlives the descriptor.
            unsafe { native.setObjectFunction(Some(&function)) };
            size
        }
        None => ThreadgroupSize::default(),
    };

    if let Some(fragment) = descriptor.fragment {
        let registries = lock(&device.registries);
        let record = registries
            .shaders
            .get(&fragment.id())
            .filter(|record| record.stage == ShaderStage::Fragment)
            .ok_or_else(|| RhiError::invalid_argument("fragment shader stage mismatch"))?;
        let function = record.function.clone();
        // SAFETY: the fragment function comes from this device's registry and
        // outlives the descriptor.
        unsafe { native.setFragmentFunction(Some(&function)) };
    }

    rhi_require(descriptor.color_attachments.len() <= 8, "too many mesh color attachments")?;
    let color_attachments = native.colorAttachments();
    for (index, attachment) in descriptor.color_attachments.iter().enumerate() {
        // SAFETY: the count was capped at the eight slots Metal exposes.
        let target = unsafe { color_attachments.objectAtIndexedSubscript(index) };
        target.setPixelFormat(formats::pixel_format(attachment.format));
        let blend = attachment.blend;
        // Swift sets every factor unconditionally here, so a disabled
        // attachment keeps the factors the descriptor was given.
        target.setBlendingEnabled(blend.enabled);
        target.setSourceRGBBlendFactor(formats::blend_factor(blend.source_color_blend_factor));
        target.setDestinationRGBBlendFactor(formats::blend_factor(
            blend.destination_color_blend_factor,
        ));
        target.setRgbBlendOperation(formats::blend_operation(blend.color_blend_operation));
        target.setSourceAlphaBlendFactor(formats::blend_factor(blend.source_alpha_blend_factor));
        target.setDestinationAlphaBlendFactor(formats::blend_factor(
            blend.destination_alpha_blend_factor,
        ));
        target.setAlphaBlendOperation(formats::blend_operation(blend.alpha_blend_operation));
    }
    if let Some(depth) = descriptor.depth_format {
        native.setDepthAttachmentPixelFormat(formats::pixel_format(depth));
    }
    // Depth state requires its own encoder binding. Reject it until this
    // pipeline has a depth-state implementation rather than dropping it.
    rhi_require(
        descriptor.depth_stencil.is_none(),
        "mesh depth/stencil state is not implemented",
    )?;

    let native_device = device.device.clone();
    let registries = Arc::clone(&device.registries);
    let rasterization = descriptor.rasterization;
    Ok(RhiCompilationWork::new(move || {
        let state =  {
            native_device.newRenderPipelineStateWithMeshDescriptor_options_reflection_error(
                &native,
                objc2_metal::MTLPipelineOption::empty(),
                None,
            )
        }
        .map_err(|error| {
            RhiError::invalid_argument(format!(
                "mesh pipeline failed: {}",
                error.localizedDescription()
            ))
        })?;
        let total = |size: ThreadgroupSize| size.x * size.y * size.z;
        let mesh_limit =  { state.maxTotalThreadsPerMeshThreadgroup() } as i64;
        let object_limit =  { state.maxTotalThreadsPerObjectThreadgroup() } as i64;
        rhi_require(total(mesh_size) <= mesh_limit, "mesh workgroup exceeds pipeline limit")?;
        if descriptor.task.is_some() {
            rhi_require(
                total(task_size) <= object_limit,
                "task workgroup exceeds pipeline limit",
            )?;
        }
        let id = handle.id();
        Ok(RhiCompiledObject::new(
            move || {
                lock(&registries).mesh_pipelines.insert(
                    id,
                    MetalMeshPipeline {
                        descriptor,
                        state,
                        mesh_size,
                        task_size,
                        rasterization,
                    },
                );
            },
            || {},
        ))
    }))
}

// MARK: - Compute pipelines

pub fn create_compute_pipeline(
    device: &MetalDeviceHandle,
    handle: ComputePipeline,
    descriptor: ComputePipelineDescriptor,
) -> RhiResult<()> {
    prepare_compute_compilation(device, handle, descriptor)?.run_and_install()
}

pub fn prepare_compute_compilation(
    device: &MetalDeviceHandle,
    handle: ComputePipeline,
    descriptor: ComputePipelineDescriptor,
) -> RhiResult<RhiCompilationWork> {
    let (function, size) = {
        let registries = lock(&device.registries);
        let record = registries
            .shaders
            .get(&descriptor.shader.id())
            .filter(|record| record.stage == ShaderStage::Compute)
            .ok_or_else(|| {
                RhiError::invalid_argument("compute pipeline: shader module not found")
            })?;
        (record.function.clone(), record.threadgroup_size)
    };
    let native_device = device.device.clone();
    let registries = Arc::clone(&device.registries);
    Ok(RhiCompilationWork::new(move || {
        let state = native_device
            .newComputePipelineStateWithFunction_error(&function)
            .map_err(|error| {
                RhiError::invalid_argument(format!(
                    "compute pipeline failed: {}",
                    error.localizedDescription()
                ))
            })?;
        let limit =  { state.maxTotalThreadsPerThreadgroup() } as i64;
        rhi_require(
            size.x * size.y * size.z <= limit,
            "compute workgroup exceeds pipeline limit",
        )?;
        let id = handle.id();
        Ok(RhiCompiledObject::new(
            move || {
                let mut registries = lock(&registries);
                registries.compute_threadgroup_sizes.insert(id, size);
                registries.compute_pipelines.insert(id, state);
            },
            || {},
        ))
    }))
}

// MARK: - Deferred destruction (FrameRing already waited; just release)

pub fn destroy_buffer(device: &MetalDeviceHandle, handle: Buffer) {
    lock(&device.registries).retire_buffer(handle);
}

pub fn destroy_texture(device: &MetalDeviceHandle, handle: Texture) {
    lock(&device.registries).retire_texture(handle);
}

pub fn destroy_sampler(device: &MetalDeviceHandle, handle: Sampler) {
    let mut registries = lock(&device.registries);
    registries.binding_sets.remove_all_referencing(handle.id());
    registries.samplers.remove(&handle.id());
}

pub fn destroy_shader_module(device: &MetalDeviceHandle, handle: ShaderModule) {
    lock(&device.registries).shaders.remove(&handle.id());
}

pub fn destroy_graphics_pipeline(device: &MetalDeviceHandle, handle: GraphicsPipeline) {
    let mut registries = lock(&device.registries);
    let id = handle.id();
    registries.render_pipelines.remove(&id);
    registries.depth_states.remove(&id);
    registries.pipeline_primitives.remove(&id);
    registries.pipeline_raster_states.remove(&id);
}

pub fn destroy_compute_pipeline(device: &MetalDeviceHandle, handle: ComputePipeline) {
    let mut registries = lock(&device.registries);
    registries.compute_pipelines.remove(&handle.id());
    registries.compute_threadgroup_sizes.remove(&handle.id());
}

pub fn destroy_mesh_pipeline(device: &MetalDeviceHandle, handle: MeshPipeline) {
    lock(&device.registries).mesh_pipelines.remove(&handle.id());
}

/// Backend-reported allocation for one texture, or `None` when unknown.
pub fn texture_allocation(device: &MetalDeviceHandle, texture: Texture) -> Option<TextureAllocation> {
    let registries = lock(&device.registries);
    let native = registries.textures.get(&texture.id())?;
    Some(TextureAllocation::new(
        // Both `MTLAllocation` and `MTLResource` expose `allocatedSize` with the
        // same meaning; naming the protocol resolves the ambiguity.
        MTLAllocation::allocatedSize(&**native) as i64,
        native.storageMode() == MTLStorageMode::Memoryless,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mesh_workgroup_limit_is_a_total() {
        let total = |size: ThreadgroupSize| size.x * size.y * size.z;
        assert_eq!(total(ThreadgroupSize::new(8, 4, 2)), 64);
        assert_eq!(total(ThreadgroupSize::default()), 1);
    }

    #[test]
    fn a_vertex_descriptor_is_device_independent() {
        let descriptor = unsafe { MTLVertexDescriptor::new() };
        let layout = descriptor.layouts();
        // SAFETY: slot 0 always exists in the layout array.
        let first = unsafe { layout.objectAtIndexedSubscript(0) };
        assert_eq!(first.stride(), 0);
        assert_eq!(first.stepRate(), 1);
    }
}
