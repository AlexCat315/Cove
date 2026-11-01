//! Translation of NativeRHI/Vulkan/Backend/{Bindings,Pipelines}.swift: descriptor
//! set layouts, pipeline layouts, descriptor sets, and crates/graphics/compute/mesh
//! pipeline creation.
//!
//! Swift wrapped every create-info in a `VulkanScratch` arena because the C
//! pointers only have to survive the call. The Rust port does the same with stack
//! locals plus `Vec`s, which is the same guarantee with no manual free.
use std::sync::{Arc, Mutex};

use crate::rhi::{
    BindingLayout, BindingLayoutDescriptor, BindingLayoutEntry, BindingResource, BindingSet,
    BindingSetEntry, BindingType, ComputePipeline, ComputePipelineDescriptor, GraphicsPipeline,
    GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor, PipelineLayout,
    PipelineLayoutDescriptor, RhiCompilationWork, RhiCompiledObject, RhiError, RhiResult,
    rhi_require,
};

use super::formats;
use super::registries::{
    VulkanBindingSetRecord, VulkanComputePipelineRecord, VulkanGraphicsPipelineRecord,
    VulkanMeshPipelineRecord, VulkanPipelineLayoutRecord, VulkanPushRange, VulkanRegistries,
};
use super::resources::lock;
use super::setup::VulkanContext;
use super::sys;

// MARK: - Binding layouts

pub fn register_binding_layout(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: BindingLayout,
    descriptor: BindingLayoutDescriptor,
) -> RhiResult<()> {
    let bindings: Vec<sys::VkDescriptorSetLayoutBinding> = descriptor
        .entries
        .iter()
        .map(|entry| sys::VkDescriptorSetLayoutBinding {
            binding: entry.slot,
            descriptorType: formats::vk_descriptor_type(
                entry.binding_type,
                entry.buffer.has_dynamic_offset,
            ),
            descriptorCount: entry.array_size.max(1),
            stageFlags: formats::vk_visibility_flags(entry.visibility),
            pImmutableSamplers: core::ptr::null(),
        })
        .collect();

    let create_info = sys::VkDescriptorSetLayoutCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        bindingCount: bindings.len() as u32,
        pBindings: bindings.as_ptr(),
    };
    let mut native: sys::VkDescriptorSetLayout = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` points at `bindings`, which outlives the call.
    let status = unsafe {
        (context.resources.create_descriptor_set_layout)(
            context.device,
            &create_info,
            core::ptr::null(),
            &mut native,
        )
    };
    if status != sys::VK_SUCCESS || native == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }
    lock(registries).binding_layouts.insert(handle.id(), native);
    Ok(())
}

pub fn register_pipeline_layout(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: PipelineLayout,
    descriptor: PipelineLayoutDescriptor,
) -> RhiResult<()> {
    let set_layouts: Vec<sys::VkDescriptorSetLayout> = {
        let registries = lock(registries);
        descriptor
            .set_layouts
            .iter()
            .map(|layout| {
                registries
                    .binding_layouts
                    .get(&layout.id())
                    .copied()
                    .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))
            })
            .collect::<RhiResult<Vec<_>>>()?
    };

    // Each shader stage's single SPIR-V push block begins at byte zero.
    // Overlapping ranges with disjoint stage masks are legal in Vulkan.
    let push_ranges: Vec<VulkanPushRange> = descriptor
        .push_constants
        .iter()
        .map(|declaration| VulkanPushRange {
            declaration: *declaration,
            offset: 0,
        })
        .collect();
    let native_ranges: Vec<sys::VkPushConstantRange> = push_ranges
        .iter()
        .map(|range| sys::VkPushConstantRange {
            stageFlags: formats::vk_stage_flags(range.declaration.stage),
            offset: range.offset,
            size: range.declaration.byte_count.max(0) as u32,
        })
        .collect();

    let create_info = sys::VkPipelineLayoutCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        setLayoutCount: set_layouts.len() as u32,
        pSetLayouts: set_layouts.as_ptr(),
        pushConstantRangeCount: native_ranges.len() as u32,
        pPushConstantRanges: native_ranges.as_ptr(),
    };
    let mut native: sys::VkPipelineLayout = sys::VK_NULL_HANDLE;
    // SAFETY: both arrays `create_info` points at outlive the call.
    let status = unsafe {
        (context.resources.create_pipeline_layout)(
            context.device,
            &create_info,
            core::ptr::null(),
            &mut native,
        )
    };
    if status != sys::VK_SUCCESS || native == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }
    lock(registries).pipeline_layouts.insert(
        handle.id(),
        VulkanPipelineLayoutRecord {
            native,
            descriptor,
            push_ranges,
        },
    );
    Ok(())
}

// MARK: - Descriptor sets

pub fn register_binding_set(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    descriptor_pool: sys::VkDescriptorPool,
    handle: BindingSet,
    layout: BindingLayout,
    layout_entries: Vec<BindingLayoutEntry>,
    set_entries: Vec<BindingSetEntry>,
) -> RhiResult<()> {
    let native_layout = {
        let registries = lock(registries);
        registries
            .binding_layouts
            .get(&layout.id())
            .copied()
            .ok_or_else(|| RhiError::invalid_argument("unknown binding layout"))?
    };

    let allocate_info = sys::VkDescriptorSetAllocateInfo {
        sType: sys::VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO,
        pNext: core::ptr::null(),
        descriptorPool: descriptor_pool,
        descriptorSetCount: 1,
        pSetLayouts: &native_layout,
    };
    let mut descriptor_set: sys::VkDescriptorSet = sys::VK_NULL_HANDLE;
    // SAFETY: `allocate_info` points at a live layout handle, and the
    // out-parameter is a live local.
    let status = unsafe {
        (context.resources.allocate_descriptor_sets)(
            context.device,
            &allocate_info,
            &mut descriptor_set,
        )
    };
    if status != sys::VK_SUCCESS || descriptor_set == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }

    // A failed write releases the set so it cannot leak from the pool.
    if let Err(error) = write_descriptors(
        context,
        registries,
        descriptor_set,
        &layout_entries,
        &set_entries,
    ) {
        // SAFETY: the set was allocated above and holds no live GPU references
        // beyond the descriptors being rewritten.
        unsafe {
            (context.resources.free_descriptor_sets)(
                context.device,
                descriptor_pool,
                1,
                &descriptor_set,
            )
        };
        return Err(error);
    }

    let dynamic_count = layout_entries
        .iter()
        .filter(|entry| entry.buffer.has_dynamic_offset)
        .count();
    lock(registries).binding_sets.insert(
        handle.id(),
        VulkanBindingSetRecord {
            descriptor_set,
            layout_id: layout.id(),
            dynamic_offset_count: dynamic_count as u32,
        },
    );
    Ok(())
}

/// Writes every entry of one descriptor set. Resources are resolved once here, so
/// binding at draw time is a single `vkCmdBindDescriptorSets`.
fn write_descriptors(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    descriptor_set: sys::VkDescriptorSet,
    layout_entries: &[BindingLayoutEntry],
    set_entries: &[BindingSetEntry],
) -> RhiResult<()> {
    // Each write needs its own info struct whose address stays valid until
    // `update_descriptor_sets`. Keeping them in vectors that never reallocate
    // during the loop (all were reserved) guarantees that.
    let mut buffer_infos: Vec<sys::VkDescriptorBufferInfo> = Vec::new();
    let mut image_infos: Vec<sys::VkDescriptorImageInfo> = Vec::new();
    let mut acceleration_handles: Vec<sys::VkAccelerationStructureKHR> = Vec::new();
    let mut acceleration_infos: Vec<sys::VkWriteDescriptorSetAccelerationStructureKHR> = Vec::new();
    let mut writes: Vec<sys::VkWriteDescriptorSet> = Vec::new();

    buffer_infos.reserve(set_entries.len());
    image_infos.reserve(set_entries.len());
    acceleration_handles.reserve(set_entries.len());
    acceleration_infos.reserve(set_entries.len());
    writes.reserve(set_entries.len());

    {
        let registries = lock(registries);
        for set_entry in set_entries {
            let declaration = layout_entries
                .iter()
                .find(|entry| entry.slot == set_entry.slot)
                .ok_or_else(|| RhiError::layout_mismatch("binding has no declaration"))?;
            let mut write = sys::VkWriteDescriptorSet {
                sType: sys::VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET,
                pNext: core::ptr::null(),
                dstSet: descriptor_set,
                dstBinding: set_entry.slot,
                dstArrayElement: 0,
                descriptorCount: 1,
                descriptorType: formats::vk_descriptor_type(
                    declaration.binding_type,
                    declaration.buffer.has_dynamic_offset,
                ),
                pImageInfo: core::ptr::null(),
                pBufferInfo: core::ptr::null(),
                pTexelBufferView: core::ptr::null(),
            };

            match set_entry.resource {
                BindingResource::UniformBuffer {
                    buffer,
                    offset,
                    size,
                } => {
                    let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                        RhiError::invalid_argument("unknown buffer or invalid binding offset")
                    })?;
                    rhi_require(
                        offset < record.size,
                        "unknown buffer or invalid binding offset",
                    )?;
                    let alignment = context.limits.minUniformBufferOffsetAlignment.max(1);
                    rhi_require(
                        (offset as u64).is_multiple_of(alignment),
                        "buffer binding offset violates Vulkan alignment",
                    )?;
                    let length = size.unwrap_or(record.size - offset);
                    rhi_require(length > 0, "buffer binding size must be positive")?;
                    rhi_require(
                        length <= context.limits.maxUniformBufferRange as usize,
                        "buffer binding exceeds Vulkan descriptor range limit",
                    )?;
                    super::resources::check_binding_range(offset, length, record.size)?;
                    buffer_infos.push(sys::VkDescriptorBufferInfo {
                        buffer: record.buffer,
                        offset: offset as sys::VkDeviceSize,
                        range: length as sys::VkDeviceSize,
                    });
                    write.pBufferInfo = buffer_infos.last().expect("just pushed");
                }
                BindingResource::StorageBuffer { buffer, offset } => {
                    let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                        RhiError::invalid_argument("unknown buffer or invalid binding offset")
                    })?;
                    rhi_require(
                        offset < record.size,
                        "unknown buffer or invalid binding offset",
                    )?;
                    let alignment = context.limits.minStorageBufferOffsetAlignment.max(1);
                    rhi_require(
                        (offset as u64).is_multiple_of(alignment),
                        "buffer binding offset violates Vulkan alignment",
                    )?;
                    let length = record.size - offset;
                    rhi_require(length > 0, "buffer binding size must be positive")?;
                    rhi_require(
                        length <= context.limits.maxStorageBufferRange as usize,
                        "buffer binding exceeds Vulkan descriptor range limit",
                    )?;
                    buffer_infos.push(sys::VkDescriptorBufferInfo {
                        buffer: record.buffer,
                        offset: offset as sys::VkDeviceSize,
                        range: length as sys::VkDeviceSize,
                    });
                    write.pBufferInfo = buffer_infos.last().expect("just pushed");
                }
                BindingResource::Texture(texture) | BindingResource::StorageTexture(texture) => {
                    let record = registries
                        .textures
                        .get(&texture.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
                    image_infos.push(sys::VkDescriptorImageInfo {
                        sampler: sys::VK_NULL_HANDLE,
                        imageView: record.view,
                        imageLayout: if declaration.binding_type == BindingType::StorageTexture {
                            sys::VK_IMAGE_LAYOUT_GENERAL
                        } else {
                            sys::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL
                        },
                    });
                    write.pImageInfo = image_infos.last().expect("just pushed");
                }
                BindingResource::Sampler(sampler) => {
                    let record = registries
                        .samplers
                        .get(&sampler.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown sampler"))?;
                    image_infos.push(sys::VkDescriptorImageInfo {
                        sampler: record.sampler,
                        imageView: sys::VK_NULL_HANDLE,
                        imageLayout: sys::VK_IMAGE_LAYOUT_UNDEFINED,
                    });
                    write.pImageInfo = image_infos.last().expect("just pushed");
                }
                BindingResource::AccelerationStructure(structure) => {
                    let record = registries
                        .acceleration_structures
                        .get(&structure.id())
                        .ok_or_else(|| {
                            RhiError::invalid_argument("unknown acceleration structure binding")
                        })?;
                    acceleration_handles.push(record.native);
                    acceleration_infos.push(sys::VkWriteDescriptorSetAccelerationStructureKHR {
                        sType:
                            sys::VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET_ACCELERATION_STRUCTURE_KHR,
                        pNext: core::ptr::null(),
                        accelerationStructureCount: 1,
                        pAccelerationStructures: acceleration_handles.last().expect("just pushed"),
                    });
                    write.pNext = acceleration_infos.last().expect("just pushed") as *const _
                        as *const core::ffi::c_void;
                }
            }
            writes.push(write);
        }
    }

    if writes.is_empty() {
        return Ok(());
    }
    // SAFETY: every pointer in each write refers to an element of a vector that
    // was reserved up front and never reallocated, so all of them stay valid for
    // the duration of this call.
    unsafe {
        (context.resources.update_descriptor_sets)(
            context.device,
            writes.len() as u32,
            writes.as_ptr(),
            0,
            core::ptr::null(),
        )
    };
    Ok(())
}

pub fn unregister_binding_set(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    descriptor_pool: sys::VkDescriptorPool,
    handle: BindingSet,
) {
    let Some(record) = lock(registries).binding_sets.remove(&handle.id()) else {
        return;
    };
    let descriptor_set = record.descriptor_set;
    // SAFETY: the frame ring waited for every submission that could use this set,
    // and the set was allocated from this pool.
    unsafe {
        (context.resources.free_descriptor_sets)(
            context.device,
            descriptor_pool,
            1,
            &descriptor_set,
        )
    };
}

// MARK: - Specialization storage

/// Owns the specialization map a pipeline stage points at. The vectors must not
/// move after `info` captures their addresses, so this value is created once and
/// kept alive next to the stage that references it.
struct SpecializationStorage {
    #[allow(dead_code, reason = "`info.pMapEntries` points into this buffer")]
    entries: Vec<sys::VkSpecializationMapEntry>,
    #[allow(dead_code, reason = "`info.pData` points into this buffer")]
    data: Vec<u32>,
    info: sys::VkSpecializationInfo,
}

impl SpecializationStorage {
    fn build(registries: &VulkanRegistries, handle: crate::rhi::ShaderModule) -> Option<Self> {
        let record = registries.shader_modules.get(&handle.id())?;
        if record.specialization_constants.is_empty() {
            return None;
        }
        let entries: Vec<sys::VkSpecializationMapEntry> = record
            .specialization_constants
            .iter()
            .enumerate()
            .map(|(index, constant)| sys::VkSpecializationMapEntry {
                constantID: constant.id,
                offset: (index * 4) as u32,
                size: 4,
            })
            .collect();
        let data: Vec<u32> = record
            .specialization_constants
            .iter()
            .map(|constant| constant.value.bits())
            .collect();
        let info = sys::VkSpecializationInfo {
            mapEntryCount: entries.len() as u32,
            pMapEntries: entries.as_ptr(),
            dataSize: data.len() * 4,
            pData: data.as_ptr().cast(),
        };
        Some(Self {
            entries,
            data,
            info,
        })
    }
}

/// A pipeline's stage list together with the strings and constant storage the
/// stages point at, so the whole group can be moved into a compilation closure.
struct StageGroup {
    stages: Vec<sys::VkPipelineShaderStageCreateInfo>,
    entry_points: Vec<std::ffi::CString>,
    specializations: Vec<Option<SpecializationStorage>>,
}

impl StageGroup {
    fn build(
        registries: &VulkanRegistries,
        modules: &[(crate::rhi::ShaderModule, crate::rhi::ShaderStage)],
    ) -> RhiResult<Self> {
        let mut group = Self {
            stages: Vec::with_capacity(modules.len()),
            entry_points: Vec::with_capacity(modules.len()),
            specializations: Vec::with_capacity(modules.len()),
        };
        for (handle, expected) in modules {
            let record = registries
                .shader_modules
                .get(&handle.id())
                .filter(|record| record.stage == *expected)
                .ok_or_else(|| RhiError::invalid_argument("unknown shader module for its stage"))?;
            let entry_point =
                std::ffi::CString::new(record.entry_point.as_str()).map_err(|_| {
                    RhiError::invalid_argument("shader entry point contains a NUL byte")
                })?;
            let stage = sys::VkPipelineShaderStageCreateInfo {
                sType: sys::VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: core::ptr::null(),
                flags: 0,
                stage: formats::vk_stage_flags(record.stage),
                module: record.module,
                // Filled by `refresh` once the storage is in its final place.
                pName: core::ptr::null(),
                pSpecializationInfo: core::ptr::null(),
            };
            group.entry_points.push(entry_point);
            group
                .specializations
                .push(SpecializationStorage::build(registries, *handle));
            group.stages.push(stage);
        }
        group.refresh();
        Ok(group)
    }

    /// Re-points every stage at this group's own storage. Called after construction
    /// and again after the group is moved into a closure.
    fn refresh(&mut self) {
        for (index, stage) in self.stages.iter_mut().enumerate() {
            stage.pName = self.entry_points[index].as_ptr();
            stage.pSpecializationInfo = match &self.specializations[index] {
                Some(spec) => &spec.info,
                None => core::ptr::null(),
            };
        }
    }

    fn contains_mesh_stage(&self) -> bool {
        self.stages
            .iter()
            .any(|stage| stage.stage == sys::VK_SHADER_STAGE_MESH_BIT_EXT)
    }
}

/// Every fixed-function state block, held together with the arrays they point at
/// so one owner keeps all the pointers valid.
struct FixedFunctionState {
    vertex_bindings: Vec<sys::VkVertexInputBindingDescription>,
    vertex_attributes: Vec<sys::VkVertexInputAttributeDescription>,
    blend_attachments: Vec<sys::VkPipelineColorBlendAttachmentState>,
    dynamic_states: [u32; 2],
}

impl FixedFunctionState {
    fn build(descriptor: &GraphicsPipelineDescriptor) -> RhiResult<Self> {
        let mut vertex_bindings = Vec::new();
        let mut vertex_attributes = Vec::new();
        if let Some(layout) = &descriptor.vertex_layout {
            rhi_require(
                layout.attributes.len() <= 32 && layout.buffer_layouts.len() <= 8,
                "vertex input capacity exceeded",
            )?;
            for (index, buffer_layout) in layout.buffer_layouts.iter().enumerate() {
                rhi_require(buffer_layout.stride > 0, "vertex strides must be positive")?;
                vertex_bindings.push(sys::VkVertexInputBindingDescription {
                    binding: index as u32,
                    stride: buffer_layout.stride as u32,
                    inputRate: match buffer_layout.step_rate {
                        crate::rhi::VertexInputRate::PerInstance => {
                            sys::VK_VERTEX_INPUT_RATE_INSTANCE
                        }
                        crate::rhi::VertexInputRate::PerVertex => sys::VK_VERTEX_INPUT_RATE_VERTEX,
                    },
                });
            }
            for attribute in &layout.attributes {
                rhi_require(
                    (attribute.buffer_index as usize) < layout.buffer_layouts.len(),
                    "vertex attribute references an invalid buffer or offset",
                )?;
                vertex_attributes.push(sys::VkVertexInputAttributeDescription {
                    location: attribute.location,
                    binding: attribute.buffer_index,
                    format: formats::vk_vertex_format(attribute.format),
                    offset: attribute.offset as u32,
                });
            }
        }
        let blend_attachments = descriptor
            .color_attachments
            .iter()
            .map(|attachment| sys::VkPipelineColorBlendAttachmentState {
                blendEnable: u32::from(attachment.blend.enabled),
                srcColorBlendFactor: formats::vk_blend_factor(
                    attachment.blend.source_color_blend_factor,
                ),
                dstColorBlendFactor: formats::vk_blend_factor(
                    attachment.blend.destination_color_blend_factor,
                ),
                colorBlendOp: formats::vk_blend_op(attachment.blend.color_blend_operation),
                srcAlphaBlendFactor: formats::vk_blend_factor(
                    attachment.blend.source_alpha_blend_factor,
                ),
                dstAlphaBlendFactor: formats::vk_blend_factor(
                    attachment.blend.destination_alpha_blend_factor,
                ),
                alphaBlendOp: formats::vk_blend_op(attachment.blend.alpha_blend_operation),
                colorWriteMask: sys::VK_COLOR_COMPONENT_ALL,
            })
            .collect();
        Ok(Self {
            vertex_bindings,
            vertex_attributes,
            blend_attachments,
            dynamic_states: [
                sys::VK_DYNAMIC_STATE_VIEWPORT,
                sys::VK_DYNAMIC_STATE_SCISSOR,
            ],
        })
    }
}

/// Creates one graphics pipeline. All state lives in this function's frame, which
/// is exactly the lifetime Vulkan requires: it copies the create-info before
/// returning.
fn create_graphics_pipeline_native(
    device: sys::VkDevice,
    create_graphics_pipelines: sys::PFN_vkCreateGraphicsPipelines,
    rendering: super::rendering::RenderingHandles,
    descriptor: &GraphicsPipelineDescriptor,
    stages: &StageGroup,
    layout: sys::VkPipelineLayout,
) -> RhiResult<(sys::VkPipeline, super::rendering::RenderPassObjects)> {
    let state = FixedFunctionState::build(descriptor)?;
    let mut render_objects = super::rendering::RenderPassObjects::new(rendering);
    let render_pass = render_objects.pipeline_pass(descriptor)?;

    let raw_stages: Vec<sys::VkPipelineShaderStageCreateInfo> = stages.stages.to_vec();

    let vertex_input = sys::VkVertexInputStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        vertexBindingDescriptionCount: state.vertex_bindings.len() as u32,
        pVertexBindingDescriptions: state.vertex_bindings.as_ptr(),
        vertexAttributeDescriptionCount: state.vertex_attributes.len() as u32,
        pVertexAttributeDescriptions: state.vertex_attributes.as_ptr(),
    };
    let assembly = sys::VkInputAssemblyStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        topology: formats::vk_primitive_topology(descriptor.primitive),
        primitiveRestartEnable: sys::VK_FALSE,
    };
    let raster = sys::VkPipelineRasterizationStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        depthClampEnable: sys::VK_FALSE,
        rasterizerDiscardEnable: sys::VK_FALSE,
        polygonMode: if descriptor.rasterization.fill_mode == crate::rhi::FillMode::Lines {
            sys::VK_POLYGON_MODE_LINE
        } else {
            sys::VK_POLYGON_MODE_FILL
        },
        cullMode: formats::vk_cull_mode(descriptor.rasterization.cull_mode),
        frontFace: formats::vk_front_face(descriptor.rasterization.front_winding),
        depthBiasEnable: sys::VK_FALSE,
        depthBiasConstantFactor: 0.0,
        depthBiasClamp: 0.0,
        depthBiasSlopeFactor: 0.0,
        lineWidth: 1.0,
    };
    // Viewport and scissor are dynamic state, so only the counts are declared.
    let viewport = sys::VkPipelineViewportStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        viewportCount: 1,
        pViewports: core::ptr::null(),
        scissorCount: 1,
        pScissors: core::ptr::null(),
    };
    let samples = sys::VkPipelineMultisampleStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        rasterizationSamples: formats::vk_sample_count(descriptor.sample_count)?,
        sampleShadingEnable: sys::VK_FALSE,
        minSampleShading: 0.0,
        pSampleMask: core::ptr::null(),
        alphaToCoverageEnable: sys::VK_FALSE,
        alphaToOneEnable: sys::VK_FALSE,
    };
    let mut depth = sys::VkPipelineDepthStencilStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        depthTestEnable: sys::VK_FALSE,
        depthWriteEnable: sys::VK_FALSE,
        depthCompareOp: sys::VK_COMPARE_OP_ALWAYS,
        depthBoundsTestEnable: sys::VK_FALSE,
        stencilTestEnable: sys::VK_FALSE,
        front: sys::VkStencilOpState::default(),
        back: sys::VkStencilOpState::default(),
        minDepthBounds: 0.0,
        maxDepthBounds: 1.0,
    };
    if let (Some(state), true) = (descriptor.depth_stencil, descriptor.depth_format.is_some()) {
        depth.depthTestEnable = sys::VK_TRUE;
        depth.depthWriteEnable = u32::from(state.depth_write_enabled);
        depth.depthCompareOp = formats::vk_compare_op(state.depth_compare);
    }
    let blend = sys::VkPipelineColorBlendStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        logicOpEnable: sys::VK_FALSE,
        logicOp: 0,
        attachmentCount: state.blend_attachments.len() as u32,
        pAttachments: state.blend_attachments.as_ptr(),
        blendConstants: [0.0; 4],
    };
    let dynamic = sys::VkPipelineDynamicStateCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        dynamicStateCount: state.dynamic_states.len() as u32,
        pDynamicStates: state.dynamic_states.as_ptr(),
    };

    // A mesh pipeline has no vertex input or input assembly state.
    let is_mesh = stages.contains_mesh_stage();
    let create_info = sys::VkGraphicsPipelineCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        stageCount: raw_stages.len() as u32,
        pStages: raw_stages.as_ptr(),
        pVertexInputState: if is_mesh {
            core::ptr::null()
        } else {
            &vertex_input
        },
        pInputAssemblyState: if is_mesh {
            core::ptr::null()
        } else {
            &assembly
        },
        pTessellationState: core::ptr::null(),
        pViewportState: &viewport,
        pRasterizationState: &raster,
        pMultisampleState: &samples,
        pDepthStencilState: &depth,
        pColorBlendState: &blend,
        pDynamicState: &dynamic,
        layout,
        renderPass: render_pass,
        subpass: 0,
        basePipelineHandle: sys::VK_NULL_HANDLE,
        basePipelineIndex: -1,
    };

    let mut pipeline: sys::VkPipeline = sys::VK_NULL_HANDLE;
    // SAFETY: every pointer in `create_info` refers to storage owned by this
    // function's frame (or by `stages`, which outlives the call), and
    // `render_pass` is alive until `render_objects` is returned.
    let status = unsafe {
        (create_graphics_pipelines)(
            device,
            sys::VK_NULL_HANDLE,
            1,
            &create_info,
            core::ptr::null(),
            &mut pipeline,
        )
    };
    if status != sys::VK_SUCCESS || pipeline == sys::VK_NULL_HANDLE {
        return Err(RhiError::invalid_argument(
            "Vulkan graphics pipeline failed",
        ));
    }
    // The pipeline keeps its own reference to the render pass, so the pass objects
    // are released here; the driver's pipeline object no longer needs them.
    Ok((pipeline, render_objects))
}

// MARK: - Compute pipelines

pub fn create_compute_pipeline(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: ComputePipeline,
    descriptor: ComputePipelineDescriptor,
) -> RhiResult<()> {
    prepare_compute_compilation(context, registries, handle, descriptor)?.run_and_install()
}

pub fn prepare_compute_compilation(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: ComputePipeline,
    descriptor: ComputePipelineDescriptor,
) -> RhiResult<RhiCompilationWork> {
    let (group, layout_native) = {
        let registries = lock(registries);
        let layout = registries
            .pipeline_layouts
            .get(&descriptor.layout.id())
            .ok_or_else(|| {
                RhiError::invalid_argument("unknown compute shader or pipeline layout")
            })?;
        let group = StageGroup::build(
            &registries,
            &[(descriptor.shader, crate::rhi::ShaderStage::Compute)],
        )?;
        (group, layout.native)
    };

    let device = context.device;
    let create = context.resources.create_compute_pipelines;
    let destroy = context.resources.destroy_pipeline;
    let shared = Arc::clone(registries);
    let id = handle.id();
    Ok(RhiCompilationWork::new(move || {
        let mut group = group;
        // Re-point the stage at the storage this closure now owns.
        group.refresh();
        let create_info = sys::VkComputePipelineCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            stage: group.stages[0],
            layout: layout_native,
            basePipelineHandle: sys::VK_NULL_HANDLE,
            basePipelineIndex: -1,
        };
        let mut pipeline: sys::VkPipeline = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info` and the stage storage it points at live in this
        // closure's frame for the whole call.
        let status = unsafe {
            (create)(
                device,
                sys::VK_NULL_HANDLE,
                1,
                &create_info,
                core::ptr::null(),
                &mut pipeline,
            )
        };
        if status != sys::VK_SUCCESS || pipeline == sys::VK_NULL_HANDLE {
            return Err(RhiError::invalid_argument("Vulkan compute pipeline failed"));
        }
        let shared_publish = Arc::clone(&shared);
        Ok(RhiCompiledObject::new(
            move || {
                lock(&shared_publish).compute_pipelines.insert(
                    id,
                    VulkanComputePipelineRecord {
                        pipeline,
                        descriptor,
                    },
                );
            },
            // SAFETY: the pipeline was created above and is only destroyed when
            // this object is dropped without being installed.
            move || unsafe { (destroy)(device, pipeline, core::ptr::null()) },
        ))
    }))
}

pub fn destroy_compute_pipeline(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: ComputePipeline,
) {
    let Some(record) = lock(registries).compute_pipelines.remove(&handle.id()) else {
        return;
    };
    // SAFETY: the frame ring waited for every submission that could use it.
    unsafe {
        (context.resources.destroy_pipeline)(context.device, record.pipeline, core::ptr::null());
    }
}

// MARK: - Graphics pipelines

pub fn create_graphics_pipeline(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: GraphicsPipeline,
    descriptor: GraphicsPipelineDescriptor,
) -> RhiResult<()> {
    prepare_graphics_compilation(context, registries, handle, descriptor)?.run_and_install()
}

pub fn prepare_graphics_compilation(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: GraphicsPipeline,
    descriptor: GraphicsPipelineDescriptor,
) -> RhiResult<RhiCompilationWork> {
    rhi_require(
        descriptor.color_attachments.len() <= 8,
        "at most eight color attachments are supported",
    )?;
    rhi_require(
        !(descriptor.rasterization.fill_mode == crate::rhi::FillMode::Lines
            && !context.features.non_solid_fill),
        "Vulkan non-solid fill is unsupported",
    )?;

    let (group, layout_native) = {
        let registries = lock(registries);
        let layout = registries
            .pipeline_layouts
            .get(&descriptor.layout.id())
            .ok_or_else(|| {
                RhiError::invalid_argument("unknown graphics shader or pipeline layout")
            })?;
        let mut modules = vec![(descriptor.vertex, crate::rhi::ShaderStage::Vertex)];
        if let Some(fragment) = descriptor.fragment {
            modules.push((fragment, crate::rhi::ShaderStage::Fragment));
        }
        let group = StageGroup::build(&registries, &modules)?;
        (group, layout.native)
    };

    let shared = Arc::clone(registries);
    let id = handle.id();
    let device = context.device;
    let create = context.resources.create_graphics_pipelines;
    let destroy = context.resources.destroy_pipeline;
    let rendering = context.rendering_handles();
    Ok(RhiCompilationWork::new(move || {
        let mut group = group;
        group.refresh();
        let (pipeline, render_objects) = create_graphics_pipeline_native(
            device,
            create,
            rendering,
            &descriptor,
            &group,
            layout_native,
        )?;
        drop(render_objects);
        let shared_publish = Arc::clone(&shared);
        Ok(RhiCompiledObject::new(
            move || {
                lock(&shared_publish).graphics_pipelines.insert(
                    id,
                    VulkanGraphicsPipelineRecord {
                        pipeline,
                        descriptor,
                    },
                );
            },
            // SAFETY: the pipeline was created above and is only destroyed when
            // this object is dropped without being installed.
            move || unsafe { (destroy)(device, pipeline, core::ptr::null()) },
        ))
    }))
}

pub fn destroy_graphics_pipeline(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: GraphicsPipeline,
) {
    let Some(record) = lock(registries).graphics_pipelines.remove(&handle.id()) else {
        return;
    };
    // SAFETY: the frame ring waited for every submission that could use it.
    unsafe {
        (context.resources.destroy_pipeline)(context.device, record.pipeline, core::ptr::null());
    }
}

// MARK: - Mesh pipelines

pub fn create_mesh_pipeline(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: MeshPipeline,
    descriptor: MeshPipelineDescriptor,
) -> RhiResult<()> {
    prepare_mesh_compilation(context, registries, handle, descriptor)?.run_and_install()
}

pub fn prepare_mesh_compilation(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: MeshPipeline,
    descriptor: MeshPipelineDescriptor,
) -> RhiResult<RhiCompilationWork> {
    rhi_require(context.features.mesh, "Vulkan mesh shader unavailable")?;
    rhi_require(
        descriptor.task.is_none() || context.features.task,
        "Vulkan task shader unavailable",
    )?;

    let (group, layout_native) = {
        let registries = lock(registries);
        let layout = registries
            .pipeline_layouts
            .get(&descriptor.layout.id())
            .ok_or_else(|| {
                RhiError::unsupported_feature("Vulkan mesh shader unavailable or invalid layout")
            })?;
        let mut modules = vec![(descriptor.mesh, crate::rhi::ShaderStage::Mesh)];
        if let Some(task) = descriptor.task {
            modules.push((task, crate::rhi::ShaderStage::Task));
        }
        if let Some(fragment) = descriptor.fragment {
            modules.push((fragment, crate::rhi::ShaderStage::Fragment));
        }
        let group = StageGroup::build(&registries, &modules)?;
        (group, layout.native)
    };

    let graphics = mesh_graphics_descriptor(&descriptor);
    let shared = Arc::clone(registries);
    let id = handle.id();
    let device = context.device;
    let create = context.resources.create_graphics_pipelines;
    let destroy = context.resources.destroy_pipeline;
    let rendering = context.rendering_handles();
    Ok(RhiCompilationWork::new(move || {
        let mut group = group;
        group.refresh();
        let (pipeline, render_objects) = create_graphics_pipeline_native(
            device,
            create,
            rendering,
            &graphics,
            &group,
            layout_native,
        )?;
        drop(render_objects);
        let shared_publish = Arc::clone(&shared);
        Ok(RhiCompiledObject::new(
            move || {
                lock(&shared_publish).mesh_pipelines.insert(
                    id,
                    VulkanMeshPipelineRecord {
                        pipeline,
                        descriptor,
                    },
                );
            },
            // SAFETY: the pipeline was created above and is only destroyed when
            // this object is dropped without being installed.
            move || unsafe { (destroy)(device, pipeline, core::ptr::null()) },
        ))
    }))
}

/// Projects a mesh descriptor onto the graphics shape the shared pipeline builder
/// takes, so one creation path serves both. The vertex module handle is a
/// placeholder because a mesh pipeline has no vertex stage.
pub fn mesh_graphics_descriptor(descriptor: &MeshPipelineDescriptor) -> GraphicsPipelineDescriptor {
    let mut graphics =
        GraphicsPipelineDescriptor::new(descriptor.layout, crate::rhi::ShaderModule::new(0));
    graphics.fragment = descriptor.fragment;
    graphics.color_attachments = descriptor.color_attachments.clone();
    graphics.depth_format = descriptor.depth_format;
    graphics.rasterization = descriptor.rasterization;
    graphics.depth_stencil = descriptor.depth_stencil;
    graphics.label = descriptor.label;
    graphics
}

pub fn destroy_mesh_pipeline(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    handle: MeshPipeline,
) {
    let Some(record) = lock(registries).mesh_pipelines.remove(&handle.id()) else {
        return;
    };
    // SAFETY: the frame ring waited for every submission that could use it.
    unsafe {
        (context.resources.destroy_pipeline)(context.device, record.pipeline, core::ptr::null());
    }
}

/// Dynamic-offset positions follow the ascending dynamic binding slot.
#[cfg(test)]
pub fn dynamic_slot_positions(layout_entries: &[BindingLayoutEntry]) -> Vec<(u32, u32)> {
    let mut dynamic: Vec<&BindingLayoutEntry> = layout_entries
        .iter()
        .filter(|entry| entry.buffer.has_dynamic_offset)
        .collect();
    dynamic.sort_by_key(|entry| entry.slot);
    dynamic
        .into_iter()
        .enumerate()
        .map(|(index, entry)| (entry.slot, index as u32))
        .collect()
}

/// Rejects layouts whose slots collide or exceed the device's descriptor budget.
#[cfg(test)]
pub fn validate_binding_layout(
    entries: &[BindingLayoutEntry],
    max_descriptors: u32,
) -> RhiResult<()> {
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        rhi_require(
            seen.insert(entry.slot),
            "binding slots must be unique within a layout",
        )?;
    }
    rhi_require(
        entries.len() as u32 <= max_descriptors,
        "binding layout declares too many descriptors",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{BindingLayoutEntry, BufferBindingLayout, ShaderVisibility};

    fn entry(slot: u32, dynamic: bool) -> BindingLayoutEntry {
        BindingLayoutEntry::new(slot, BindingType::UniformBuffer, ShaderVisibility::VERTEX)
            .with_buffer_layout(BufferBindingLayout {
                read_only: false,
                has_dynamic_offset: dynamic,
                element_stride: 0,
            })
    }

    #[test]
    fn dynamic_positions_follow_the_ascending_slot() {
        let entries = vec![entry(5, true), entry(1, false), entry(2, true)];
        assert_eq!(dynamic_slot_positions(&entries), vec![(2, 0), (5, 1)]);
        assert!(dynamic_slot_positions(&[]).is_empty());
    }

    #[test]
    fn duplicate_binding_slots_are_rejected() {
        assert!(validate_binding_layout(&[entry(0, false), entry(1, false)], 8).is_ok());
        assert!(validate_binding_layout(&[entry(0, false), entry(0, true)], 8).is_err());
        assert!(
            validate_binding_layout(&[entry(0, false), entry(1, false), entry(2, false)], 2)
                .is_err()
        );
    }

    #[test]
    fn a_mesh_descriptor_projects_onto_the_graphics_shape() {
        let layout = crate::rhi::PipelineLayout::new(4);
        let mesh = MeshPipelineDescriptor::new(layout, crate::rhi::ShaderModule::new(9));
        let graphics = mesh_graphics_descriptor(&mesh);
        assert_eq!(graphics.layout, layout);
        assert_eq!(graphics.vertex, crate::rhi::ShaderModule::new(0));
        assert!(graphics.vertex_layout.is_none());
    }

    #[test]
    fn fixed_function_state_carries_dynamic_viewport_and_scissor() {
        let descriptor = GraphicsPipelineDescriptor::new(
            crate::rhi::PipelineLayout::new(1),
            crate::rhi::ShaderModule::new(2),
        );
        let state = FixedFunctionState::build(&descriptor).expect("builds");
        assert_eq!(
            state.dynamic_states,
            [
                sys::VK_DYNAMIC_STATE_VIEWPORT,
                sys::VK_DYNAMIC_STATE_SCISSOR
            ]
        );
        assert!(state.vertex_bindings.is_empty());
        assert!(state.blend_attachments.is_empty());
    }
}
