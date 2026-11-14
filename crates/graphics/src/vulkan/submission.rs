//! Translation of NativeRHI/Vulkan/{Backend/Submit, Backend/Encoder,
//! Backend/Timestamps, Backend/VulkanBackendTransfer, Submission/*}.swift:
//! per-frame state, command recording, queue submission, immediate transfers and
//! GPU timing.
//!
//! There is no `vkQueueWaitIdle` on the submit path: each submit gets its own
//! fence, the work is queued, and completion fires from a background waiter.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::rhi::{
    BarrierCommand, ColorLoadAction, Completion, ComputeCommand, ComputePassRecord, CopyCommand,
    CopyPassRecord, CpuProfilePhase, DepthLoadAction, GpuTimestampMarker, GpuTimestampProfile,
    PlannedCommand, PlannedSubmit, RenderCommand, RenderPassRecord, RenderTextureInfo, RhiError,
    RhiResult, SubmissionCpuProfile, Texture, TextureFormat, TextureSubresource,
    TextureUploadRegion, UploadLocation, UploadRingChunk, rhi_byte_range,
    rhi_color_texture_copy_extent, rhi_count, rhi_render_pass_signature, rhi_require,
    rhi_texture_subresource_extent, rhi_texture_upload_bytes,
};

use super::acceleration;
use super::formats;
use super::memory::VulkanMemoryAllocation;
use super::registries::{VulkanRegistries, VulkanTextureRecord};
use super::rendering::RenderPassObjects;
use super::rendering::{AttachmentShape, clear_value};
use super::resources::{create_buffer, destroy_buffer, lock};
use super::setup::VulkanContext;
use super::sys;

// MARK: - Command pools

/// A frame slot reuses its peak number of command buffers. Resetting a Vulkan
/// pool resets buffer state; it does not free the allocated objects.
pub struct CommandPool {
    pub handle: sys::VkCommandPool,
    buffers: Vec<sys::VkCommandBuffer>,
    used: usize,
}

impl CommandPool {
    pub fn new(handle: sys::VkCommandPool) -> Self {
        Self {
            handle,
            buffers: Vec::new(),
            used: 0,
        }
    }

    pub fn rewind(&mut self) {
        self.used = 0;
    }

    pub fn command_buffer(&mut self, context: &VulkanContext) -> RhiResult<sys::VkCommandBuffer> {
        if self.used == self.buffers.len() {
            let allocate_info = sys::VkCommandBufferAllocateInfo {
                sType: sys::VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
                pNext: core::ptr::null(),
                commandPool: self.handle,
                level: sys::VK_COMMAND_BUFFER_LEVEL_PRIMARY,
                commandBufferCount: 1,
            };
            let mut command: sys::VkCommandBuffer = core::ptr::null_mut();
            // SAFETY: the pool is live and the out-parameter is a live local.
            let status = unsafe {
                (context.resources.allocate_command_buffers)(
                    context.device,
                    &allocate_info,
                    &mut command,
                )
            };
            if status != sys::VK_SUCCESS || command.is_null() {
                return Err(RhiError::submit_failed("vkAllocateCommandBuffers failed"));
            }
            self.buffers.push(command);
        }
        let command = self.buffers[self.used];
        self.used += 1;
        Ok(command)
    }
}

/// Per-slot command pools and upload storage.
pub struct VulkanFrame {
    /// Shared so the frontend's `FrameUploader` handle and the slot's pool state
    /// can both reach it: the frontend owns the boxed handle for the backend's
    /// lifetime, while the slot resets it at each frame start.
    pub uploader: Arc<Mutex<VulkanFrameUploader>>,
    pub command_pools: HashMap<u32, CommandPool>,
}

impl VulkanFrame {
    pub fn new(uploader: Arc<Mutex<VulkanFrameUploader>>) -> Self {
        Self {
            uploader,
            command_pools: HashMap::new(),
        }
    }
}

// MARK: - Upload ring

/// A chunk of GPU-visible upload storage: a persistently mapped `VkBuffer` plus
/// the `Buffer` handle it is registered under.
#[derive(Clone)]
pub struct VulkanUploadChunk {
    pub allocation: VulkanMemoryAllocation,
    pub buffer_handle: crate::rhi::Buffer,
    pub capacity: usize,
}

// SAFETY: the chunk is a driver handle plus a persistently mapped region. The
// allocator only hands out non-overlapping suballocations for one frame slot at a
// time, and Vulkan's host-visible coherent memory is safe to write from the API
// thread.
unsafe impl Send for VulkanUploadChunk {}

impl UploadRingChunk for VulkanUploadChunk {
    fn capacity(&self) -> usize {
        self.capacity
    }

    unsafe fn cpu_pointer(&self) -> *mut u8 {
        // SAFETY: a host-visible block exposes a mapping valid for the chunk's
        // lifetime, and the allocator validated this suballocation's range.
        self.allocation
            .cpu_pointer(0)
            .unwrap_or(core::ptr::null_mut())
    }
}

/// Adapts the chunked upload ring to the frontend's `FrameUploader` contract. Each
/// chunk's persistently mapped `VkBuffer` is exposed as a transient `Buffer`
/// handle the encoder resolves back to the chunk.
pub struct VulkanFrameUploader {
    allocator: crate::rhi::ChunkUploadAllocator<VulkanUploadChunk>,
}

impl VulkanFrameUploader {
    pub fn new(
        factory: impl FnMut(usize) -> RhiResult<VulkanUploadChunk> + Send + 'static,
    ) -> Self {
        Self {
            allocator: crate::rhi::ChunkUploadAllocator::new(4 * 1024 * 1024, factory),
        }
    }
}

impl crate::rhi::FrameUploader for VulkanFrameUploader {
    fn reset(&mut self) {
        self.allocator.reset();
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadLocation> {
        let allocation = self.allocator.write(bytes, alignment)?;
        Ok(UploadLocation::new(
            allocation.chunk().buffer_handle,
            allocation.offset(),
        ))
    }
}

// MARK: - Barriers and layout tracking

/// Issues a global memory dependency.
///
/// # Safety
/// `cmd` must be recording.
pub unsafe fn memory_dependency(
    context: &VulkanContext,
    cmd: sys::VkCommandBuffer,
    source: u32,
    destination: u32,
) {
    let barrier = sys::VkMemoryBarrier {
        sType: sys::VK_STRUCTURE_TYPE_MEMORY_BARRIER,
        pNext: core::ptr::null(),
        srcAccessMask: source,
        dstAccessMask: destination,
    };
    // SAFETY: the caller guarantees `cmd` is recording and `barrier` outlives the
    // call.
    unsafe {
        (context.draw.cmd_pipeline_barrier)(
            cmd,
            sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
            sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
            0,
            1,
            &barrier,
            0,
            core::ptr::null(),
            0,
            core::ptr::null(),
        )
    };
}

/// The access mask a resource state implies.
pub fn state_access(state: crate::rhi::ResourceState) -> u32 {
    use crate::rhi::ResourceState;
    let mut flags = 0;
    if state.contains(ResourceState::CONSTANT_BUFFER) {
        flags |= sys::VK_ACCESS_UNIFORM_READ_BIT;
    }
    if state.contains(ResourceState::VERTEX_BUFFER) {
        flags |= sys::VK_ACCESS_VERTEX_ATTRIBUTE_READ_BIT;
    }
    if state.contains(ResourceState::INDEX_BUFFER) {
        flags |= sys::VK_ACCESS_INDEX_READ_BIT;
    }
    if state.contains(ResourceState::INDIRECT_ARGUMENT) {
        flags |= sys::VK_ACCESS_INDIRECT_COMMAND_READ_BIT;
    }
    if state.contains(ResourceState::SHADER_RESOURCE) {
        flags |= sys::VK_ACCESS_SHADER_READ_BIT;
    }
    if state.contains(ResourceState::UNORDERED_ACCESS) {
        flags |= sys::VK_ACCESS_SHADER_READ_BIT | sys::VK_ACCESS_SHADER_WRITE_BIT;
    }
    if state.contains(ResourceState::RENDER_TARGET) {
        flags |=
            sys::VK_ACCESS_COLOR_ATTACHMENT_READ_BIT | sys::VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;
    }
    if state.contains(ResourceState::DEPTH_WRITE) {
        flags |= sys::VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT
            | sys::VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT;
    }
    if state.contains(ResourceState::DEPTH_READ) {
        flags |= sys::VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT;
    }
    if state.contains(ResourceState::COPY_SOURCE) {
        flags |= sys::VK_ACCESS_TRANSFER_READ_BIT;
    }
    if state.contains(ResourceState::COPY_DESTINATION) {
        flags |= sys::VK_ACCESS_TRANSFER_WRITE_BIT;
    }
    if state.contains(ResourceState::ACCELERATION_STRUCTURE_READ) {
        flags |= sys::VK_ACCESS_ACCELERATION_STRUCTURE_READ_BIT_KHR;
    }
    if state.contains(ResourceState::ACCELERATION_STRUCTURE_WRITE) {
        flags |= sys::VK_ACCESS_ACCELERATION_STRUCTURE_WRITE_BIT_KHR;
    }
    flags
}

/// The image layout a resource state implies.
pub fn state_layout(state: crate::rhi::ResourceState) -> u32 {
    use crate::rhi::ResourceState;
    if state.contains(ResourceState::UNORDERED_ACCESS)
        || (state.contains(ResourceState::COPY_SOURCE)
            && state.contains(ResourceState::COPY_DESTINATION))
    {
        return sys::VK_IMAGE_LAYOUT_GENERAL;
    }
    if state.contains(ResourceState::RENDER_TARGET) {
        return sys::VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL;
    }
    if state.contains(ResourceState::DEPTH_WRITE) {
        return sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
    }
    if state.contains(ResourceState::DEPTH_READ) {
        return sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL;
    }
    if state.contains(ResourceState::COPY_DESTINATION) {
        return sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
    }
    if state.contains(ResourceState::COPY_SOURCE) {
        return sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL;
    }
    if state.contains(ResourceState::PRESENT) {
        return sys::VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
    }
    if state.contains(ResourceState::SHADER_RESOURCE) {
        return sys::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
    }
    sys::VK_IMAGE_LAYOUT_GENERAL
}

/// The access mask a layout implies, used as the barrier's source mask.
pub fn layout_access(layout: u32) -> u32 {
    match layout {
        sys::VK_IMAGE_LAYOUT_UNDEFINED => 0,
        sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL => sys::VK_ACCESS_TRANSFER_WRITE_BIT,
        sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL => sys::VK_ACCESS_TRANSFER_READ_BIT,
        sys::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL => sys::VK_ACCESS_SHADER_READ_BIT,
        sys::VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL => {
            sys::VK_ACCESS_COLOR_ATTACHMENT_READ_BIT | sys::VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT
        }
        sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL => {
            sys::VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT
                | sys::VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT
        }
        sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL => {
            sys::VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT
        }
        _ => sys::VK_ACCESS_MEMORY_READ_BIT | sys::VK_ACCESS_MEMORY_WRITE_BIT,
    }
}

/// Moves one texture into the layout `state` requires, recording the new layout.
///
/// # Safety
/// `cmd` must be recording.
pub unsafe fn transition_texture(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    cmd: sys::VkCommandBuffer,
    handle: Texture,
    state: crate::rhi::ResourceState,
) -> RhiResult<()> {
    let (image, old_layout, new_layout, aspect, mip_levels, layers) = {
        let registries = lock(registries);
        let record = registries
            .textures
            .get(&handle.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
        (
            record.image,
            record.layout,
            state_layout(state),
            formats::aspect_flags(record.format),
            record.mip_levels,
            record.layers,
        )
    };
    let barrier = sys::VkImageMemoryBarrier {
        sType: sys::VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
        pNext: core::ptr::null(),
        srcAccessMask: layout_access(old_layout),
        dstAccessMask: state_access(state),
        oldLayout: old_layout,
        newLayout: new_layout,
        srcQueueFamilyIndex: sys::VK_QUEUE_FAMILY_IGNORED,
        dstQueueFamilyIndex: sys::VK_QUEUE_FAMILY_IGNORED,
        image,
        subresourceRange: sys::VkImageSubresourceRange {
            aspectMask: aspect,
            baseMipLevel: 0,
            levelCount: mip_levels,
            baseArrayLayer: 0,
            layerCount: layers,
        },
    };
    // SAFETY: the caller guarantees `cmd` is recording and `barrier` outlives the
    // call.
    unsafe {
        (context.draw.cmd_pipeline_barrier)(
            cmd,
            sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
            sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
            0,
            0,
            core::ptr::null(),
            0,
            core::ptr::null(),
            1,
            &barrier,
        )
    };
    if let Some(record) = lock(registries).textures.get_mut(&handle.id()) {
        record.layout = new_layout;
    }
    Ok(())
}

/// Emits the recorded barriers.
///
/// # Safety
/// `cmd` must be recording.
pub unsafe fn encode_barriers(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    cmd: sys::VkCommandBuffer,
    barriers: &[BarrierCommand],
) -> RhiResult<()> {
    for barrier in barriers {
        match barrier.resource.kind {
            crate::rhi::ResourceKind::Texture => {
                if barrier.sync_action != crate::rhi::BarrierSyncAction::Release {
                    // SAFETY: forwarded from this function's contract.
                    unsafe {
                        transition_texture(
                            context,
                            registries,
                            cmd,
                            Texture::new(barrier.resource.id),
                            barrier.destination_state,
                        )?
                    };
                }
            }
            crate::rhi::ResourceKind::AccelerationStructure => {
                let source = if barrier.sync_action == crate::rhi::BarrierSyncAction::Acquire {
                    0
                } else {
                    state_access(barrier.source_state)
                };
                let destination = if barrier.sync_action == crate::rhi::BarrierSyncAction::Release {
                    0
                } else {
                    state_access(barrier.destination_state)
                };
                // SAFETY: forwarded from this function's contract.
                unsafe { memory_dependency(context, cmd, source, destination) };
            }
            _ => {
                let buffer = {
                    let registries = lock(registries);
                    registries
                        .buffers
                        .get(&barrier.resource.id)
                        .map(|record| (record.buffer, record.size))
                };
                let Some((native, size)) = buffer else {
                    continue;
                };
                let memory = sys::VkBufferMemoryBarrier {
                    sType: sys::VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER,
                    pNext: core::ptr::null(),
                    srcAccessMask: if barrier.sync_action == crate::rhi::BarrierSyncAction::Acquire
                    {
                        0
                    } else {
                        state_access(barrier.source_state)
                    },
                    dstAccessMask: if barrier.sync_action == crate::rhi::BarrierSyncAction::Release
                    {
                        0
                    } else {
                        state_access(barrier.destination_state)
                    },
                    srcQueueFamilyIndex: sys::VK_QUEUE_FAMILY_IGNORED,
                    dstQueueFamilyIndex: sys::VK_QUEUE_FAMILY_IGNORED,
                    buffer: native,
                    offset: 0,
                    size: size as sys::VkDeviceSize,
                };
                // SAFETY: forwarded from this function's contract.
                unsafe {
                    (context.draw.cmd_pipeline_barrier)(
                        cmd,
                        sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                        sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                        0,
                        0,
                        core::ptr::null(),
                        1,
                        &memory,
                        0,
                        core::ptr::null(),
                    )
                };
            }
        }
    }
    Ok(())
}

// MARK: - Encoder

/// Encodes planned commands, bracketing each with a GPU timestamp when a profile
/// is attached.
pub struct Encoder<'a> {
    context: &'a VulkanContext,
    registries: &'a Arc<Mutex<VulkanRegistries>>,
    cmd: sys::VkCommandBuffer,
}

impl<'a> Encoder<'a> {
    pub fn new(
        context: &'a VulkanContext,
        registries: &'a Arc<Mutex<VulkanRegistries>>,
        cmd: sys::VkCommandBuffer,
    ) -> Self {
        Self {
            context,
            registries,
            cmd,
        }
    }

    fn encode(
        &mut self,
        commands: &[PlannedCommand],
        timestamps: Option<&SubmissionTimestamps>,
    ) -> RhiResult<()> {
        let mut index = 2;
        for command in commands {
            let timing = timestamps.filter(|timing| {
                index < timing.count && !matches!(command, PlannedCommand::Barriers(_))
            });
            if let Some(timing) = timing {
                timing.write(self.cmd, sys::VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, index);
            }
            // SAFETY: the caller opened the command buffer for recording.
            unsafe { self.encode_command(command)? };
            if let Some(timing) = timing {
                timing.write(
                    self.cmd,
                    sys::VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
                    index + 1,
                );
                index += 2;
            }
        }
        Ok(())
    }

    unsafe fn encode_command(&mut self, command: &PlannedCommand) -> RhiResult<()> {
        match command {
            PlannedCommand::AccelerationStructureBuild(build) => {
                let registries = lock(self.registries);
                // SAFETY: the caller guarantees `cmd` is recording.
                unsafe {
                    acceleration::encode_build(self.context, &registries, self.cmd, build.structure)
                }
            }
            PlannedCommand::RenderPass(record) => {
                // SAFETY: forwarded from this function's contract.
                unsafe { self.encode_render_pass(record) }
            }
            PlannedCommand::ComputePass(record) => {
                // SAFETY: forwarded from this function's contract.
                unsafe { self.encode_compute_pass(record) }
            }
            PlannedCommand::CopyPass(record) => {
                // SAFETY: forwarded from this function's contract.
                unsafe { self.encode_copy_pass(record) }
            }
            PlannedCommand::Barriers(barriers) => {
                // SAFETY: forwarded from this function's contract.
                unsafe { encode_barriers(self.context, self.registries, self.cmd, barriers) }
            }
        }
    }

    unsafe fn encode_render_pass(&mut self, record: &RenderPassRecord) -> RhiResult<()> {
        let signature = self.render_pass_signature(record)?;
        let extent = sys::VkExtent2D {
            width: signature.extent[0] as u32,
            height: signature.extent[1] as u32,
        };

        // Transition every attachment, collecting the shapes and views the pass
        // needs in one place.
        let mut attachments = Vec::new();
        let mut colors = Vec::new();
        let mut resolves = Vec::new();
        let mut depth_attachment = None;
        let mut views: Vec<sys::VkImageView> = Vec::new();
        let mut clears: Vec<sys::VkClearValue> = Vec::new();
        for target in &record.descriptor.color_targets {
            // SAFETY: forwarded from this function's contract.
            unsafe {
                transition_texture(
                    self.context,
                    self.registries,
                    self.cmd,
                    target.texture,
                    crate::rhi::ResourceState::RENDER_TARGET,
                )?
            };
            let (view, format) = {
                let registries = lock(self.registries);
                let texture = registries
                    .textures
                    .get(&target.texture.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown color target"))?;
                (registries.attachment_view(texture), texture.format)
            };
            let shape = AttachmentShape {
                format: formats::vk_format(format),
                samples: signature.sample_count as u32,
                is_depth: false,
            };
            let (load_op, clear) = match target.load_action {
                ColorLoadAction::Clear(color) => (sys::VK_ATTACHMENT_LOAD_OP_CLEAR, Some(color)),
                ColorLoadAction::Load => (sys::VK_ATTACHMENT_LOAD_OP_LOAD, None),
                ColorLoadAction::DontCare => (sys::VK_ATTACHMENT_LOAD_OP_DONT_CARE, None),
            };
            let store_op = if target.store {
                sys::VK_ATTACHMENT_STORE_OP_STORE
            } else {
                sys::VK_ATTACHMENT_STORE_OP_DONT_CARE
            };
            colors.push(shape.reference(attachments.len()));
            attachments.push(shape.description(load_op, store_op));
            views.push(view);
            clears.push(clear_value(&shape, clear, None));
            if let Some(resolve) = target.resolve_texture {
                // SAFETY: forwarded from this function's contract.
                unsafe {
                    transition_texture(
                        self.context,
                        self.registries,
                        self.cmd,
                        resolve,
                        crate::rhi::ResourceState::RENDER_TARGET,
                    )?
                };
                let (resolve_view, resolve_format) = {
                    let registries = lock(self.registries);
                    let texture = registries
                        .textures
                        .get(&resolve.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown resolve target"))?;
                    (registries.attachment_view(texture), texture.format)
                };
                let resolve_shape = AttachmentShape {
                    format: formats::vk_format(resolve_format),
                    samples: 1,
                    is_depth: false,
                };
                resolves.push(resolve_shape.reference(attachments.len()));
                attachments.push(resolve_shape.description(
                    sys::VK_ATTACHMENT_LOAD_OP_DONT_CARE,
                    sys::VK_ATTACHMENT_STORE_OP_STORE,
                ));
                views.push(resolve_view);
                clears.push(clear_value(&resolve_shape, None, None));
            } else {
                resolves.push(sys::VkAttachmentReference {
                    attachment: sys::VK_ATTACHMENT_UNUSED,
                    layout: sys::VK_IMAGE_LAYOUT_UNDEFINED,
                });
            }
        }
        if let Some(target) = record.descriptor.depth_target {
            // SAFETY: forwarded from this function's contract.
            unsafe {
                transition_texture(
                    self.context,
                    self.registries,
                    self.cmd,
                    target.texture,
                    crate::rhi::ResourceState::DEPTH_WRITE,
                )?
            };
            let (view, format) = {
                let registries = lock(self.registries);
                let texture = registries
                    .textures
                    .get(&target.texture.id())
                    .ok_or_else(|| RhiError::invalid_argument("invalid depth target"))?;
                rhi_require(texture.format.is_depth(), "invalid depth target")?;
                (registries.attachment_view(texture), texture.format)
            };
            let shape = AttachmentShape {
                format: formats::vk_format(format),
                samples: signature.sample_count as u32,
                is_depth: true,
            };
            let (load_op, clear) = match target.load_action {
                DepthLoadAction::Clear(depth) => (sys::VK_ATTACHMENT_LOAD_OP_CLEAR, Some(depth)),
                DepthLoadAction::Load => (sys::VK_ATTACHMENT_LOAD_OP_LOAD, None),
                DepthLoadAction::DontCare => (sys::VK_ATTACHMENT_LOAD_OP_DONT_CARE, None),
            };
            let store_op = if target.store {
                sys::VK_ATTACHMENT_STORE_OP_STORE
            } else {
                sys::VK_ATTACHMENT_STORE_OP_DONT_CARE
            };
            depth_attachment = Some(shape.reference(attachments.len()));
            attachments.push(shape.description(load_op, store_op));
            views.push(view);
            clears.push(clear_value(&shape, None, clear));
        }

        let mut render_objects = RenderPassObjects::new(self.context.rendering_handles());
        let pass = render_objects.make_pass(&attachments, &colors, &resolves, depth_attachment)?;
        let framebuffer = render_objects.framebuffer(pass, &views, extent.width, extent.height)?;
        // SAFETY: every image was transitioned above into the layout the pass
        // declares, and `cmd` is recording.
        unsafe {
            render_objects.begin(
                self.cmd,
                pass,
                framebuffer,
                sys::VkRect2D {
                    offset: sys::VkOffset2D { x: 0, y: 0 },
                    extent,
                },
                &clears,
            )
        };

        // NativeRHI clip space has +Y up and framebuffer coordinates have +Y down.
        // Vulkan 1.1 supports the negative-height viewport transform directly.
        let mut viewport = sys::VkViewport {
            x: 0.0,
            y: extent.height as f32,
            width: extent.width as f32,
            height: -(extent.height as f32),
            minDepth: 0.0,
            maxDepth: 1.0,
        };
        let mut scissor = sys::VkRect2D {
            offset: sys::VkOffset2D { x: 0, y: 0 },
            extent,
        };
        // SAFETY: `cmd` is recording inside the pass.
        unsafe {
            (self.context.draw.cmd_set_viewport)(self.cmd, 0, 1, &viewport);
            (self.context.draw.cmd_set_scissor)(self.cmd, 0, 1, &scissor);
        }

        let mut layout_id: Option<u32> = None;
        let mut index_bound = false;
        let mut mesh_bound = false;
        let replay = (|| -> RhiResult<()> {
            for command in &record.body {
                match command {
                    RenderCommand::SetMeshPipeline(handle) => {
                        let (pipeline, id) = {
                            let registries = lock(self.registries);
                            let pipeline =
                                registries.mesh_pipelines.get(&handle.id()).ok_or_else(|| {
                                    RhiError::invalid_argument("unknown mesh pipeline")
                                })?;
                            (pipeline.pipeline, pipeline.descriptor.layout.id())
                        };
                        layout_id = Some(id);
                        mesh_bound = true;
                        // SAFETY: `cmd` is recording inside the pass.
                        unsafe {
                            (self.context.draw.cmd_bind_pipeline)(
                                self.cmd,
                                sys::VK_PIPELINE_BIND_POINT_GRAPHICS,
                                pipeline,
                            )
                        };
                    }
                    RenderCommand::DrawMeshTasks(grid) => {
                        rhi_require(
                            mesh_bound && self.context.advanced.draw_mesh_tasks.is_some(),
                            "mesh dispatch requires mesh pipeline",
                        )?;
                        let dispatch = self
                            .context
                            .advanced
                            .draw_mesh_tasks
                            .expect("checked above");
                        // SAFETY: `cmd` is recording inside the pass.
                        unsafe {
                            dispatch(
                                self.cmd,
                                rhi_count(grid.x)?,
                                rhi_count(grid.y)?,
                                rhi_count(grid.z)?,
                            )
                        };
                    }
                    RenderCommand::SetPipeline(handle) => {
                        let (pipeline, id, raster) = {
                            let registries = lock(self.registries);
                            let pipeline =
                                registries.graphics_pipelines.get(&handle.id()).ok_or_else(
                                    || RhiError::invalid_argument("unknown graphics pipeline"),
                                )?;
                            (
                                pipeline.pipeline,
                                pipeline.descriptor.layout.id(),
                                pipeline.descriptor.rasterization,
                            )
                        };
                        let _ = raster;
                        layout_id = Some(id);
                        mesh_bound = false;
                        // SAFETY: `cmd` is recording inside the pass.
                        unsafe {
                            (self.context.draw.cmd_bind_pipeline)(
                                self.cmd,
                                sys::VK_PIPELINE_BIND_POINT_GRAPHICS,
                                pipeline,
                            )
                        };
                    }
                    RenderCommand::SetBindingSet {
                        slot,
                        set,
                        dynamic_offsets,
                    } => {
                        // SAFETY: forwarded from the enclosing contract.
                        unsafe {
                            self.bind_set(
                                sys::VK_PIPELINE_BIND_POINT_GRAPHICS,
                                layout_id,
                                *slot,
                                *set,
                                dynamic_offsets,
                            )?
                        };
                    }
                    RenderCommand::PushConstant { stage, slot, data } => {
                        // SAFETY: forwarded from the enclosing contract.
                        unsafe { self.push_constants(layout_id, *stage, *slot, data)? };
                    }
                    RenderCommand::SetVertexBuffer {
                        slot,
                        buffer,
                        offset,
                    } => {
                        let native = {
                            let registries = lock(self.registries);
                            let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                                RhiError::invalid_argument("unknown vertex buffer")
                            })?;
                            rhi_byte_range(*offset, 0, record.size)?;
                            record.buffer
                        };
                        let byte_offset = *offset as sys::VkDeviceSize;
                        // SAFETY: the offset was validated inside the buffer, which is
                        // live, and `cmd` is recording.
                        unsafe {
                            (self.context.draw.cmd_bind_vertex_buffers)(
                                self.cmd,
                                *slot,
                                1,
                                &native,
                                &byte_offset,
                            )
                        };
                    }
                    RenderCommand::SetIndexBuffer {
                        buffer,
                        offset,
                        index_type,
                    } => {
                        let native = {
                            let registries = lock(self.registries);
                            let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                                RhiError::invalid_argument("unknown index buffer")
                            })?;
                            rhi_byte_range(*offset, 0, record.size)?;
                            record.buffer
                        };
                        // SAFETY: the offset was validated and `cmd` is recording.
                        unsafe {
                            (self.context.draw.cmd_bind_index_buffer)(
                                self.cmd,
                                native,
                                *offset as sys::VkDeviceSize,
                                formats::vk_index_type(*index_type),
                            )
                        };
                        index_bound = true;
                    }
                    RenderCommand::SetViewport(value) => {
                        viewport = sys::VkViewport {
                            x: value.x as f32,
                            y: (value.y + value.height) as f32,
                            width: value.width as f32,
                            height: -(value.height as f32),
                            minDepth: value.min_depth as f32,
                            maxDepth: value.max_depth as f32,
                        };
                        // SAFETY: `cmd` is recording inside the pass.
                        unsafe { (self.context.draw.cmd_set_viewport)(self.cmd, 0, 1, &viewport) };
                    }
                    RenderCommand::SetScissor(value) => {
                        scissor = sys::VkRect2D {
                            offset: sys::VkOffset2D {
                                x: value.x as i32,
                                y: value.y as i32,
                            },
                            extent: sys::VkExtent2D {
                                width: rhi_count(value.width)?,
                                height: rhi_count(value.height)?,
                            },
                        };
                        // SAFETY: `cmd` is recording inside the pass.
                        unsafe { (self.context.draw.cmd_set_scissor)(self.cmd, 0, 1, &scissor) };
                    }
                    RenderCommand::Draw {
                        vertex_count,
                        instance_count,
                        first_vertex,
                        first_instance,
                    } => {
                        rhi_require(
                            layout_id.is_some() && !mesh_bound,
                            "draw requires a graphics pipeline",
                        )?;
                        // SAFETY: a graphics pipeline is bound and `cmd` is recording.
                        unsafe {
                            (self.context.draw.cmd_draw)(
                                self.cmd,
                                rhi_count(*vertex_count)?,
                                rhi_count(*instance_count)?,
                                rhi_count(*first_vertex)?,
                                rhi_count(*first_instance)?,
                            )
                        };
                    }
                    RenderCommand::DrawIndexed(arguments) => {
                        rhi_require(
                            index_bound && layout_id.is_some(),
                            "indexed draw requires a pipeline and index buffer",
                        )?;
                        // SAFETY: a pipeline and index buffer are bound.
                        unsafe {
                            (self.context.draw.cmd_draw_indexed)(
                                self.cmd,
                                rhi_count(arguments.index_count)?,
                                rhi_count(arguments.instance_count)?,
                                rhi_count(arguments.first_index)?,
                                arguments.vertex_offset as i32,
                                rhi_count(arguments.first_instance)?,
                            )
                        };
                    }
                    RenderCommand::DrawIndirect {
                        buffer,
                        offset,
                        draw_count,
                    } => {
                        let native = {
                            let registries = lock(self.registries);
                            let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                                RhiError::invalid_argument("invalid indirect draw")
                            })?;
                            let count = rhi_count(*draw_count)? as usize;
                            rhi_byte_range(*offset, count * 16, record.size)?;
                            record.buffer
                        };
                        rhi_require(
                            !mesh_bound && offset % 4 == 0,
                            "invalid indirect draw alignment or pipeline",
                        )?;
                        // One draw per argument block, matching Swift: the RHI's
                        // `draw_count` is a pixel count of small draws, not a Vulkan
                        // multi-draw.
                        for index in 0..*draw_count {
                            // SAFETY: every block was range-checked above.
                            unsafe {
                                (self.context.draw.cmd_draw_indirect)(
                                    self.cmd,
                                    native,
                                    (*offset as sys::VkDeviceSize) + (index as u64) * 16,
                                    1,
                                    16,
                                )
                            };
                        }
                    }
                }
            }
            Ok(())
        })();

        // SAFETY: the pass was begun above and `cmd` is still recording.
        unsafe { render_objects.end(self.cmd) };
        replay
    }

    unsafe fn encode_compute_pass(&mut self, record: &ComputePassRecord) -> RhiResult<()> {
        let mut layout_id: Option<u32> = None;
        for command in &record.body {
            match command {
                ComputeCommand::SetPipeline(handle) => {
                    let (pipeline, id) =
                        {
                            let registries = lock(self.registries);
                            let pipeline =
                                registries.compute_pipelines.get(&handle.id()).ok_or_else(
                                    || RhiError::invalid_argument("unknown compute pipeline"),
                                )?;
                            (pipeline.pipeline, pipeline.descriptor.layout.id())
                        };
                    layout_id = Some(id);
                    // SAFETY: the caller guarantees `cmd` is recording.
                    unsafe {
                        (self.context.draw.cmd_bind_pipeline)(
                            self.cmd,
                            sys::VK_PIPELINE_BIND_POINT_COMPUTE,
                            pipeline,
                        )
                    };
                }
                ComputeCommand::SetBindingSet {
                    slot,
                    set,
                    dynamic_offsets,
                } => {
                    // SAFETY: forwarded from this function's contract.
                    unsafe {
                        self.bind_set(
                            sys::VK_PIPELINE_BIND_POINT_COMPUTE,
                            layout_id,
                            *slot,
                            *set,
                            dynamic_offsets,
                        )?
                    };
                }
                ComputeCommand::PushConstant { stage, slot, data } => {
                    // SAFETY: forwarded from this function's contract.
                    unsafe { self.push_constants(layout_id, *stage, *slot, data)? };
                }
                ComputeCommand::Dispatch {
                    groups_x,
                    groups_y,
                    groups_z,
                } => {
                    rhi_require(
                        layout_id.is_some() && *groups_x > 0 && *groups_y > 0 && *groups_z > 0,
                        "dispatch requires a pipeline and positive groups",
                    )?;
                    let limits = self.context.limits.maxComputeWorkGroupCount;
                    rhi_require(
                        *groups_x <= i64::from(limits[0])
                            && *groups_y <= i64::from(limits[1])
                            && *groups_z <= i64::from(limits[2]),
                        "dispatch exceeds Vulkan workgroup limits",
                    )?;
                    // SAFETY: a pipeline is bound and `cmd` is recording.
                    unsafe {
                        (self.context.draw.cmd_dispatch)(
                            self.cmd,
                            rhi_count(*groups_x)?,
                            rhi_count(*groups_y)?,
                            rhi_count(*groups_z)?,
                        )
                    };
                    // SAFETY: `cmd` is still recording.
                    unsafe {
                        memory_dependency(
                            self.context,
                            self.cmd,
                            sys::VK_ACCESS_SHADER_WRITE_BIT,
                            sys::VK_ACCESS_SHADER_READ_BIT | sys::VK_ACCESS_SHADER_WRITE_BIT,
                        )
                    };
                }
                ComputeCommand::DispatchIndirect { buffer, offset } => {
                    let native = {
                        let registries = lock(self.registries);
                        let record = registries.buffers.get(&buffer.id()).ok_or_else(|| {
                            RhiError::invalid_argument("invalid indirect dispatch")
                        })?;
                        rhi_byte_range(*offset, 12, record.size)?;
                        record.buffer
                    };
                    rhi_require(
                        layout_id.is_some() && offset % 4 == 0,
                        "indirect dispatch offset must be aligned",
                    )?;
                    // SAFETY: the argument block was range-checked and aligned above.
                    unsafe {
                        (self.context.draw.cmd_dispatch_indirect)(
                            self.cmd,
                            native,
                            *offset as sys::VkDeviceSize,
                        )
                    };
                    // SAFETY: `cmd` is still recording.
                    unsafe {
                        memory_dependency(
                            self.context,
                            self.cmd,
                            sys::VK_ACCESS_SHADER_WRITE_BIT,
                            sys::VK_ACCESS_SHADER_READ_BIT | sys::VK_ACCESS_SHADER_WRITE_BIT,
                        )
                    };
                }
            }
        }
        Ok(())
    }

    unsafe fn encode_copy_pass(&mut self, record: &CopyPassRecord) -> RhiResult<()> {
        for command in &record.body {
            match command {
                CopyCommand::CopyTexture {
                    src,
                    dst,
                    width,
                    height,
                } => {
                    let (src_image, dst_image, src_format, dst_format, src_size, dst_size) = {
                        let registries = lock(self.registries);
                        let source = registries.textures.get(&src.id()).ok_or_else(|| {
                            RhiError::invalid_argument("unknown texture copy resource")
                        })?;
                        let destination = registries.textures.get(&dst.id()).ok_or_else(|| {
                            RhiError::invalid_argument("unknown texture copy resource")
                        })?;
                        (
                            source.image,
                            destination.image,
                            source.format,
                            destination.format,
                            [source.width, source.height],
                            [destination.width, destination.height],
                        )
                    };
                    rhi_require(src != dst, "texture copy requires distinct textures")?;
                    rhi_color_texture_copy_extent(
                        *width as usize,
                        *height as usize,
                        crate::rhi::TextureCopyExtent {
                            width: src_size[0],
                            height: src_size[1],
                            format: src_format,
                        },
                        crate::rhi::TextureCopyExtent {
                            width: dst_size[0],
                            height: dst_size[1],
                            format: dst_format,
                        },
                    )?;
                    // SAFETY: forwarded from this function's contract.
                    unsafe {
                        transition_texture(
                            self.context,
                            self.registries,
                            self.cmd,
                            *src,
                            crate::rhi::ResourceState::COPY_SOURCE,
                        )?;
                        transition_texture(
                            self.context,
                            self.registries,
                            self.cmd,
                            *dst,
                            crate::rhi::ResourceState::COPY_DESTINATION,
                        )?;
                    }
                    let region = sys::VkImageCopy {
                        srcSubresource: sys::VkImageSubresourceLayers {
                            aspectMask: sys::VK_IMAGE_ASPECT_COLOR_BIT,
                            mipLevel: 0,
                            baseArrayLayer: 0,
                            layerCount: 1,
                        },
                        srcOffset: sys::VkOffset3D::default(),
                        dstSubresource: sys::VkImageSubresourceLayers {
                            aspectMask: sys::VK_IMAGE_ASPECT_COLOR_BIT,
                            mipLevel: 0,
                            baseArrayLayer: 0,
                            layerCount: 1,
                        },
                        dstOffset: sys::VkOffset3D::default(),
                        extent: sys::VkExtent3D {
                            width: *width as u32,
                            height: *height as u32,
                            depth: 1,
                        },
                    };
                    // SAFETY: both images are in the layouts the copy names.
                    unsafe {
                        (self.context.draw.cmd_copy_image)(
                            self.cmd,
                            src_image,
                            sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                            dst_image,
                            sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                            1,
                            &region,
                        )
                    };
                }
                CopyCommand::CopyBuffer {
                    src,
                    src_offset,
                    dst,
                    dst_offset,
                    size,
                } => {
                    let (src_buffer, dst_buffer, src_size, dst_size) = {
                        let registries = lock(self.registries);
                        let source = registries
                            .buffers
                            .get(&src.id())
                            .ok_or_else(|| RhiError::invalid_argument("unknown copy buffer"))?;
                        let destination = registries
                            .buffers
                            .get(&dst.id())
                            .ok_or_else(|| RhiError::invalid_argument("unknown copy buffer"))?;
                        (
                            source.buffer,
                            destination.buffer,
                            source.size,
                            destination.size,
                        )
                    };
                    rhi_byte_range(*src_offset, *size, src_size)?;
                    rhi_byte_range(*dst_offset, *size, dst_size)?;
                    let region = sys::VkBufferCopy {
                        srcOffset: *src_offset as sys::VkDeviceSize,
                        dstOffset: *dst_offset as sys::VkDeviceSize,
                        size: *size as sys::VkDeviceSize,
                    };
                    // SAFETY: both ranges were validated inside their buffers.
                    unsafe {
                        (self.context.draw.cmd_copy_buffer)(
                            self.cmd, src_buffer, dst_buffer, 1, &region,
                        )
                    };
                }
                CopyCommand::CopyBufferToTexture(upload) => {
                    // SAFETY: forwarded from this function's contract.
                    unsafe {
                        self.texture_copy(
                            upload.buffer,
                            upload.offset,
                            upload.bytes_per_row,
                            upload.texture,
                            upload.region,
                            upload.subresource,
                            true,
                        )?
                    };
                }
                CopyCommand::CopyTextureToBuffer {
                    texture,
                    width,
                    height,
                    buffer,
                    offset,
                    bytes_per_row,
                } => {
                    // SAFETY: forwarded from this function's contract.
                    unsafe {
                        self.texture_copy(
                            *buffer,
                            *offset,
                            *bytes_per_row,
                            *texture,
                            TextureUploadRegion::new(*width, *height),
                            TextureSubresource::default(),
                            false,
                        )?
                    };
                }
            }
            // SAFETY: `cmd` is still recording.
            unsafe {
                memory_dependency(
                    self.context,
                    self.cmd,
                    sys::VK_ACCESS_TRANSFER_WRITE_BIT,
                    sys::VK_ACCESS_TRANSFER_READ_BIT | sys::VK_ACCESS_TRANSFER_WRITE_BIT,
                )
            };
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn texture_copy(
        &mut self,
        buffer: crate::rhi::Buffer,
        offset: usize,
        bytes_per_row: usize,
        texture: Texture,
        region: TextureUploadRegion,
        subresource: TextureSubresource,
        upload: bool,
    ) -> RhiResult<()> {
        let (image, native_buffer, texture_format, mip_levels, layers, width, height, buffer_size) = {
            let registries = lock(self.registries);
            let image = registries
                .textures
                .get(&texture.id())
                .ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
            let native = registries
                .buffers
                .get(&buffer.id())
                .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
            (
                image.image,
                native.buffer,
                image.format,
                image.mip_levels,
                image.layers,
                image.width,
                image.height,
                native.size,
            )
        };
        let (extent_width, extent_height) = rhi_texture_subresource_extent(
            subresource,
            width,
            height,
            mip_levels as usize,
            layers as usize,
        )?;
        let available = buffer_size.saturating_sub(offset.min(buffer_size));
        let byte_count = rhi_texture_upload_bytes(
            region,
            bytes_per_row,
            texture_format,
            extent_width,
            extent_height,
            available,
        )?;
        rhi_byte_range(offset, byte_count, buffer_size)?;
        // SAFETY: forwarded from this function's contract.
        unsafe {
            transition_texture(
                self.context,
                self.registries,
                self.cmd,
                texture,
                if upload {
                    crate::rhi::ResourceState::COPY_DESTINATION
                } else {
                    crate::rhi::ResourceState::COPY_SOURCE
                },
            )?
        };
        let pixels = texture_format.bytes_per_texel().max(1);
        let copy = sys::VkBufferImageCopy {
            bufferOffset: offset as sys::VkDeviceSize,
            bufferRowLength: (bytes_per_row / pixels) as u32,
            bufferImageHeight: 0,
            imageSubresource: sys::VkImageSubresourceLayers {
                aspectMask: format_aspect(texture_format),
                mipLevel: subresource.mip_level as u32,
                baseArrayLayer: subresource.layer as u32,
                layerCount: 1,
            },
            imageOffset: sys::VkOffset3D {
                x: region.origin[0] as i32,
                y: region.origin[1] as i32,
                z: 0,
            },
            imageExtent: sys::VkExtent3D {
                width: region.width as u32,
                height: region.height as u32,
                depth: 1,
            },
        };
        // SAFETY: the region, pitch and byte range were validated above, and the
        // image is in the layout the copy names.
        unsafe {
            if upload {
                (self.context.draw.cmd_copy_buffer_to_image)(
                    self.cmd,
                    native_buffer,
                    image,
                    sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                    1,
                    &copy,
                )
            } else {
                (self.context.draw.cmd_copy_image_to_buffer)(
                    self.cmd,
                    image,
                    sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                    native_buffer,
                    1,
                    &copy,
                )
            }
        };
        Ok(())
    }

    /// Binds one descriptor set, validating it against the active pipeline layout.
    ///
    /// # Safety
    /// `cmd` must be recording.
    unsafe fn bind_set(
        &mut self,
        bind_point: u32,
        layout_id: Option<u32>,
        slot: u32,
        handle: crate::rhi::BindingSet,
        dynamic_offsets: &[u32],
    ) -> RhiResult<()> {
        let (native_layout, native_set, expected_count) = {
            let registries = lock(self.registries);
            let Some(layout_id) = layout_id else {
                return Err(RhiError::layout_mismatch(
                    "binding set does not match the active pipeline layout",
                ));
            };
            let layout = registries.pipeline_layouts.get(&layout_id).ok_or_else(|| {
                RhiError::layout_mismatch("binding set does not match the active pipeline layout")
            })?;
            let set = registries.binding_sets.get(&handle.id()).ok_or_else(|| {
                RhiError::layout_mismatch("binding set does not match the active pipeline layout")
            })?;
            rhi_require(
                (slot as usize) < layout.descriptor.set_layouts.len()
                    && layout.descriptor.set_layouts[slot as usize].id() == set.layout_id,
                "binding set does not match the active pipeline layout",
            )?;
            (layout.native, set.descriptor_set, set.dynamic_offset_count)
        };
        rhi_require(
            dynamic_offsets.len() as u32 == expected_count,
            "dynamic offset count does not match the binding layout",
        )?;
        // SAFETY: the layout and set are live, and `cmd` is recording.
        unsafe {
            (self.context.draw.cmd_bind_descriptor_sets)(
                self.cmd,
                bind_point,
                native_layout,
                slot,
                1,
                &native_set,
                dynamic_offsets.len() as u32,
                if dynamic_offsets.is_empty() {
                    core::ptr::null()
                } else {
                    dynamic_offsets.as_ptr()
                },
            )
        };
        Ok(())
    }

    /// Pushes small constants into the range the active layout declares.
    ///
    /// # Safety
    /// `cmd` must be recording.
    unsafe fn push_constants(
        &mut self,
        layout_id: Option<u32>,
        stage: crate::rhi::ShaderStage,
        slot: u32,
        data: &[u8],
    ) -> RhiResult<()> {
        let (native_layout, offset, byte_count) = {
            let registries = lock(self.registries);
            let Some(layout_id) = layout_id else {
                return Err(RhiError::layout_mismatch(
                    "push constants do not match the active pipeline layout",
                ));
            };
            let layout = registries.pipeline_layouts.get(&layout_id).ok_or_else(|| {
                RhiError::layout_mismatch("push constants do not match the active pipeline layout")
            })?;
            let range = layout
                .push_ranges
                .iter()
                .find(|range| range.declaration.stage == stage && range.declaration.slot == slot)
                .ok_or_else(|| {
                    RhiError::layout_mismatch(
                        "push constants do not match the active pipeline layout",
                    )
                })?;
            (layout.native, range.offset, range.declaration.byte_count)
        };
        rhi_require(
            data.len() as i64 <= byte_count && data.len().is_multiple_of(4),
            "push constants do not match the active pipeline layout",
        )?;
        // SAFETY: `data` is a live slice of exactly `data.len()` bytes and the
        // layout is live; `cmd` is recording.
        unsafe {
            (self.context.draw.cmd_push_constants)(
                self.cmd,
                native_layout,
                formats::vk_stage_flags(stage),
                offset,
                data.len() as u32,
                data.as_ptr().cast(),
            )
        };
        Ok(())
    }

    /// Builds and validates the attachment signature of one render pass.
    fn render_pass_signature(
        &self,
        record: &RenderPassRecord,
    ) -> RhiResult<crate::rhi::RenderPassSignature<u32>> {
        {
            let registries = lock(self.registries);
            for target in &record.descriptor.color_targets {
                if is_transient(&registries, target.texture) {
                    rhi_require(
                        !matches!(target.load_action, ColorLoadAction::Load),
                        "transient color attachments cannot load prior contents",
                    )?;
                    rhi_require(
                        !target.store,
                        "transient color attachments cannot preserve samples",
                    )?;
                }
            }
            if let Some(target) = record.descriptor.depth_target
                && is_transient(&registries, target.texture)
            {
                rhi_require(
                    !matches!(target.load_action, DepthLoadAction::Load),
                    "transient depth attachments cannot load prior contents",
                )?;
                rhi_require(
                    !target.store,
                    "transient depth attachments cannot preserve contents",
                )?;
            }
        }
        let registries = self.registries;
        rhi_render_pass_signature(&record.descriptor, |handle| {
            let registries = lock(registries);
            let texture = registries
                .textures
                .get(&handle.id())
                .ok_or_else(|| RhiError::invalid_argument("unknown render attachment"))?;
            describe_texture(texture)
        })
    }
}

/// Diagnostic names for the markers `encode` writes, in the same order.
pub fn encode_labels(commands: &[PlannedCommand]) -> Vec<String> {
    let mut counters: HashMap<&'static str, usize> = HashMap::new();
    commands
        .iter()
        .filter_map(|command| {
            let (kind, name) = match command {
                PlannedCommand::RenderPass(record) => ("renderPass", record.descriptor.label),
                PlannedCommand::ComputePass(_) => ("computePass", None),
                PlannedCommand::CopyPass(_) => ("copyPass", None),
                PlannedCommand::Barriers(_) => return None,
                PlannedCommand::AccelerationStructureBuild(_) => ("asBuild", None),
            };
            let ordinal = *counters.entry(kind).or_insert(0);
            counters.insert(kind, ordinal + 1);
            Some(match name {
                Some(name) if !name.is_empty() => name.to_owned(),
                _ => format!("{kind}#{ordinal}"),
            })
        })
        .collect()
}

// MARK: - Submission

/// Owns one submission's fence until the waiter retires it. The raw handles are
/// carried here so the thread boundary is a single, auditable `Send` impl.
struct FenceWaiter {
    device: sys::VkDevice,
    sync: sys::SyncCommands,
    fence: sys::VkFence,
    status: Arc<super::status::VulkanSubmissionStatus>,
    profile: Option<(Arc<GpuTimestampProfile>, Vec<String>)>,
    frame_timestamps: Option<SubmissionTimestamps>,
    completion: Result<crate::rhi::TimedCompletion, Completion>,
}

/// A query pool owned by one submission until its fence signals.
struct SubmissionTimestamps {
    device: sys::VkDevice,
    pool: sys::VkQueryPool,
    commands: sys::QueryCommands,
    period: f64,
    bits: u32,
    count: u32,
}

impl SubmissionTimestamps {
    fn new(context: &VulkanContext, bits: u32, pass_count: usize) -> Option<Self> {
        let period = f64::from(context.limits.timestampPeriod);
        if !(1..=64).contains(&bits) || !period.is_finite() || period <= 0.0 {
            return None;
        }
        let commands = context.resources.queries;
        let count = 2 + 2 * pass_count as u32;
        let descriptor = sys::VkQueryPoolCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            queryType: sys::VK_QUERY_TYPE_TIMESTAMP,
            queryCount: count,
            pipelineStatistics: 0,
        };
        let mut pool = sys::VK_NULL_HANDLE;
        // SAFETY: both pointers are valid for the duration of the call.
        let status = unsafe {
            (commands.create_query_pool)(context.device, &descriptor, core::ptr::null(), &mut pool)
        };
        (status == sys::VK_SUCCESS && pool != sys::VK_NULL_HANDLE).then(|| Self {
            device: context.device,
            pool,
            commands,
            period,
            bits,
            count,
        })
    }

    fn begin(&self, cmd: sys::VkCommandBuffer) {
        // SAFETY: cmd is recording and this submission owns the pool.
        unsafe { (self.commands.cmd_reset_query_pool)(cmd, self.pool, 0, self.count) };
        self.write(cmd, sys::VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, 0);
    }

    fn write(&self, cmd: sys::VkCommandBuffer, stage: u32, index: u32) {
        // SAFETY: cmd is recording and this query was reset by begin.
        unsafe { (self.commands.cmd_write_timestamp)(cmd, stage, self.pool, index) };
    }

    fn read(&self) -> Option<crate::rhi::GpuSubmissionTiming> {
        let mut ticks = vec![0u64; self.count as usize];
        // SAFETY: the fence signalled and the destination holds every result.
        let status = unsafe {
            (self.commands.get_query_pool_results)(
                self.device,
                self.pool,
                0,
                self.count,
                size_of_val(ticks.as_slice()),
                ticks.as_mut_ptr().cast(),
                size_of::<u64>() as u64,
                sys::VK_QUERY_RESULT_64_BIT,
            )
        };
        if status != sys::VK_SUCCESS {
            return None;
        }
        let mask = u64::MAX >> (64 - self.bits);
        let nanos = |pair: &[u64; 2]| {
            let elapsed = pair[1].wrapping_sub(pair[0]) & mask;
            let nanos = (elapsed as f64 * self.period).round();
            (nanos < u64::MAX as f64).then_some(nanos as u64)
        };
        Some(crate::rhi::GpuSubmissionTiming {
            nanoseconds: nanos(&[ticks[0], ticks[1]]),
            passes: ticks[2..].as_chunks::<2>().0.iter().map(nanos).collect(),
        })
    }
}

impl Drop for SubmissionTimestamps {
    fn drop(&mut self) {
        // SAFETY: the submission either failed before queueing or has retired.
        unsafe { (self.commands.destroy_query_pool)(self.device, self.pool, core::ptr::null()) };
    }
}

// SAFETY: every field is a Vulkan handle or a table of entry points, both of which
// Vulkan documents as safe to use from any thread once created. The fence is only
// touched by this waiter, and the timestamp recording owns its query pool.
unsafe impl Send for FenceWaiter {}

impl FenceWaiter {
    fn wait(self) {
        let mut fences = [self.fence];
        // SAFETY: the fence belongs to this submission; wait indefinitely.
        let result = unsafe {
            (self.sync.wait_for_fences)(self.device, 1, fences.as_mut_ptr(), sys::VK_TRUE, u64::MAX)
        };
        if result != sys::VK_SUCCESS {
            self.status
                .record(format!("vkWaitForFences failed: {result}"));
        }
        // SAFETY: the fence signalled (or the wait failed and the driver still owns
        // it), and nothing else references it.
        unsafe { (self.sync.destroy_fence)(self.device, self.fence, core::ptr::null()) };
        let timing = (result == sys::VK_SUCCESS)
            .then(|| self.frame_timestamps.as_ref()?.read())
            .flatten()
            .unwrap_or_default();
        if let Some((profile, labels)) = self.profile {
            profile.note(if timing.nanoseconds.is_some() {
                "ok"
            } else {
                "Vulkan timestamp queries unavailable"
            });
            for (label, nanos) in labels.into_iter().zip(&timing.passes) {
                if let Some(nanos) = nanos {
                    profile.append(GpuTimestampMarker::new(label, *nanos));
                }
            }
        }
        // Release the query pool before the callback can retire the last frame
        // and let the API thread destroy the device.
        drop(self.frame_timestamps);
        match self.completion {
            Ok(timed) => timed.complete(timing),
            Err(untimed) => untimed(),
        }
    }
}

/// A timeline or binary semaphore plus the value to wait for or signal.
#[derive(Clone, Copy, Debug)]
pub struct TimelineEntry {
    pub semaphore: sys::VkSemaphore,
    pub value: u64,
}

/// Submits one plan and arranges the asynchronous completion.
pub struct Submitter<'a> {
    pub context: &'a VulkanContext,
    pub registries: &'a Arc<Mutex<VulkanRegistries>>,
    pub frames: &'a mut Vec<VulkanFrame>,
    pub active_slot: usize,
    pub timeline_semaphores: &'a mut HashMap<u32, sys::VkSemaphore>,
    pub submission_status: Arc<super::status::VulkanSubmissionStatus>,
}

impl Submitter<'_> {
    /// Queues one submission. Returns once the work is queued; `completion` runs
    /// after the fence signals.
    pub fn submit(
        &mut self,
        submit: &PlannedSubmit,
        mut cpu_profile: Option<&mut SubmissionCpuProfile>,
        gpu_profile: Option<&Arc<GpuTimestampProfile>>,
        completion: Result<crate::rhi::TimedCompletion, Completion>,
    ) -> RhiResult<()> {
        let encoding_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
        let queue = match submit.queue {
            crate::rhi::QueueClass::Graphics => self.context.queues.graphics,
            crate::rhi::QueueClass::Compute => self.context.queues.compute,
            crate::rhi::QueueClass::Transfer => self.context.queues.transfer,
        };

        // Per-frame command buffer from this slot's pool.
        let pool_handle = self.command_pool(queue.family)?;
        let cmd = {
            // `frames` is borrowed mutably for the pool map, so take the buffer
            // out before recording.
            let frame = &mut self.frames[self.active_slot];
            let pool = frame
                .command_pools
                .get_mut(&queue.family)
                .expect("command_pool inserted it above");
            pool.command_buffer(self.context)?
        };

        let begin_info = sys::VkCommandBufferBeginInfo {
            sType: sys::VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            pNext: core::ptr::null(),
            flags: sys::VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
            pInheritanceInfo: core::ptr::null(),
        };
        rhi_require(
            // SAFETY: `cmd` was just allocated from this slot's pool and is not
            // recording.
            unsafe { (self.context.draw.begin_command_buffer)(cmd, &begin_info) }
                == sys::VK_SUCCESS,
            "vkBeginCommandBuffer failed",
        )?;

        let profile = gpu_profile.map(|profile| {
            let mut labels = encode_labels(&submit.commands);
            labels.truncate(128);
            (profile.clone(), labels)
        });
        let pass_count = completion
            .as_ref()
            .map_or(0, |timed| timed.pass_count)
            .max(profile.as_ref().map_or(0, |(_, labels)| labels.len()));
        let frame_timestamps = if completion.is_ok() || profile.is_some() {
            SubmissionTimestamps::new(self.context, queue.timestamp_bits, pass_count)
        } else {
            None
        };
        if let Some(timestamps) = &frame_timestamps {
            timestamps.begin(cmd);
        }
        Encoder::new(self.context, self.registries, cmd)
            .encode(&submit.commands, frame_timestamps.as_ref())?;
        if let Some(timestamps) = &frame_timestamps {
            timestamps.write(cmd, sys::VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, 1);
        }
        rhi_require(
            // SAFETY: `cmd` is recording and has not been ended yet.
            unsafe { (self.context.draw.end_command_buffer)(cmd) } == sys::VK_SUCCESS,
            "vkEndCommandBuffer failed",
        )?;

        // Resolve timeline semaphores (created on first use).
        let mut waits = Vec::with_capacity(submit.wait_semaphores.len());
        for semaphore in &submit.wait_semaphores {
            waits.push(self.semaphore(semaphore.id, semaphore.value)?);
        }
        let signals: Vec<TimelineEntry> = submit
            .signal_semaphores
            .iter()
            .map(|semaphore| self.semaphore(semaphore.id, semaphore.value))
            .collect::<RhiResult<Vec<_>>>()?;

        let mut fence: sys::VkFence = sys::VK_NULL_HANDLE;
        let fence_info = sys::VkFenceCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
        };
        rhi_require(
            // SAFETY: `fence_info` outlives the call and the out-parameter is a live
            // local.
            unsafe {
                (self.context.sync.create_fence)(
                    self.context.device,
                    &fence_info,
                    core::ptr::null(),
                    &mut fence,
                )
            } == sys::VK_SUCCESS
                && fence != sys::VK_NULL_HANDLE,
            "vkCreateFence failed",
        )?;

        if let Some(profile) = cpu_profile.as_deref_mut() {
            profile.end(CpuProfilePhase::Encoding, encoding_start);
        }
        let queue_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
        let status = self.queue_submit(queue.handle, cmd, &waits, &signals, fence);
        if let Some(profile) = cpu_profile {
            profile.end(CpuProfilePhase::QueueSubmit, queue_start);
        }
        if status != sys::VK_SUCCESS {
            // SAFETY: the submit failed, so the fence is unreferenced.
            unsafe {
                (self.context.sync.destroy_fence)(self.context.device, fence, core::ptr::null())
            };
            return Err(RhiError::submit_failed(format!(
                "vkQueueSubmit failed: {status}"
            )));
        }
        let _ = pool_handle;

        // Asynchronous completion: a background thread waits on the fence, reads
        // the timestamps and then fires the callback. No queue idle on the hot
        // path.
        // The waiter only touches the fence, the query pool and the profile, all
        // of which Vulkan documents as usable from any thread. The handles are
        // moved into a `Send` wrapper so the thread boundary is explicit.
        let waiter = FenceWaiter {
            device: self.context.device,
            sync: self.context.sync,
            fence,
            status: Arc::clone(&self.submission_status),
            profile,
            frame_timestamps,
            completion,
        };
        std::thread::spawn(move || waiter.wait());
        Ok(())
    }

    fn command_pool(&mut self, family: u32) -> RhiResult<sys::VkCommandPool> {
        let frame = &mut self.frames[self.active_slot];
        if let Some(pool) = frame.command_pools.get(&family) {
            return Ok(pool.handle);
        }
        let create_info = sys::VkCommandPoolCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: sys::VK_COMMAND_POOL_CREATE_TRANSIENT_BIT,
            queueFamilyIndex: family,
        };
        let mut handle: sys::VkCommandPool = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info` outlives the call and the out-parameter is a live
        // local.
        let status = unsafe {
            (self.context.resources.create_command_pool)(
                self.context.device,
                &create_info,
                core::ptr::null(),
                &mut handle,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && handle != sys::VK_NULL_HANDLE,
            "vkCreateCommandPool failed",
        )?;
        frame.command_pools.insert(family, CommandPool::new(handle));
        Ok(handle)
    }

    /// Resolves (creating on first use) the native semaphore for a timeline ID.
    fn semaphore(&mut self, id: u32, value: u64) -> RhiResult<TimelineEntry> {
        if let Some(existing) = self.timeline_semaphores.get(&id) {
            return Ok(TimelineEntry {
                semaphore: *existing,
                value,
            });
        }
        let type_info = sys::VkSemaphoreTypeCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
            pNext: core::ptr::null(),
            semaphoreType: sys::VK_SEMAPHORE_TYPE_TIMELINE,
            initialValue: 0,
        };
        let create_info = sys::VkSemaphoreCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
            pNext: (&type_info as *const sys::VkSemaphoreTypeCreateInfo).cast(),
            flags: 0,
        };
        let mut semaphore: sys::VkSemaphore = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info` and the type node it points at outlive the call.
        let status = unsafe {
            (self.context.sync.create_semaphore)(
                self.context.device,
                &create_info,
                core::ptr::null(),
                &mut semaphore,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && semaphore != sys::VK_NULL_HANDLE,
            "vkCreateSemaphore failed",
        )?;
        self.timeline_semaphores.insert(id, semaphore);
        Ok(TimelineEntry { semaphore, value })
    }

    /// All C pointers stay inside their storage's valid lifetime for the call.
    fn queue_submit(
        &self,
        queue: sys::VkQueue,
        cmd: sys::VkCommandBuffer,
        waits: &[TimelineEntry],
        signals: &[TimelineEntry],
        fence: sys::VkFence,
    ) -> sys::VkResult {
        let wait_values: Vec<u64> = waits.iter().map(|entry| entry.value).collect();
        let signal_values: Vec<u64> = signals.iter().map(|entry| entry.value).collect();
        let wait_handles: Vec<sys::VkSemaphore> =
            waits.iter().map(|entry| entry.semaphore).collect();
        let signal_handles: Vec<sys::VkSemaphore> =
            signals.iter().map(|entry| entry.semaphore).collect();
        let stages = vec![sys::VK_PIPELINE_STAGE_ALL_COMMANDS_BIT; waits.len()];

        let timeline = sys::VkTimelineSemaphoreSubmitInfo {
            sType: sys::VK_STRUCTURE_TYPE_TIMELINE_SEMAPHORE_SUBMIT_INFO,
            pNext: core::ptr::null(),
            waitSemaphoreValueCount: wait_values.len() as u32,
            pWaitSemaphoreValues: wait_values.as_ptr(),
            signalSemaphoreValueCount: signal_values.len() as u32,
            pSignalSemaphoreValues: signal_values.as_ptr(),
        };
        let info = sys::VkSubmitInfo {
            sType: sys::VK_STRUCTURE_TYPE_SUBMIT_INFO,
            pNext: (&timeline as *const sys::VkTimelineSemaphoreSubmitInfo).cast(),
            waitSemaphoreCount: wait_handles.len() as u32,
            pWaitSemaphores: wait_handles.as_ptr(),
            pWaitDstStageMask: stages.as_ptr(),
            commandBufferCount: 1,
            pCommandBuffers: &cmd,
            signalSemaphoreCount: signal_handles.len() as u32,
            pSignalSemaphores: signal_handles.as_ptr(),
        };
        // SAFETY: every array `info` points at is a live local in this frame, and
        // the command buffer was ended above.
        unsafe { (self.context.sync.queue_submit)(queue, 1, &info, fence) }
    }
}

// MARK: - Immediate transfers

/// Copies `data` into `buffer`, mapping directly when the allocation is
/// host-visible and staging otherwise.
pub fn upload_buffer_data(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    buffer: crate::rhi::Buffer,
    offset: usize,
    data: &[u8],
) -> RhiResult<()> {
    let (native, allocation, size) = {
        let registries = lock(registries);
        let record = registries
            .buffers
            .get(&buffer.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
        (record.buffer, record.allocation, record.size)
    };
    rhi_byte_range(offset, data.len(), size)?;
    if data.is_empty() {
        return Ok(());
    }
    if let Some(pointer) = allocation.cpu_pointer(offset) {
        // SAFETY: the mapping covers this allocation, the range was validated, and
        // `data` is a distinct live slice.
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), pointer, data.len()) };
        return Ok(());
    }
    // Device-local destination: stage through host-visible memory and copy.
    let staging = make_staging_buffer(context, registries, data.len())?;
    // SAFETY: the staging allocation is host visible and at least `data.len()`
    // bytes, and `data` is distinct.
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), staging.pointer, data.len());
    }
    let outcome = one_shot(context, |cmd| {
        let copy = sys::VkBufferCopy {
            srcOffset: 0,
            dstOffset: offset as sys::VkDeviceSize,
            size: data.len() as sys::VkDeviceSize,
        };
        // SAFETY: both buffers are live and the ranges were validated.
        unsafe { (context.draw.cmd_copy_buffer)(cmd, staging.buffer, native, 1, &copy) };
        Ok(())
    });
    destroy_staging_buffer(context, registries, staging);
    outcome
}

/// Reads `destination.len()` bytes back from `buffer`.
pub fn read_buffer_data(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    buffer: crate::rhi::Buffer,
    offset: usize,
    destination: &mut [u8],
) -> RhiResult<()> {
    let (native, usage, size) = {
        let registries = lock(registries);
        let record = registries
            .buffers
            .get(&buffer.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown buffer"))?;
        (record.buffer, record.usage, record.size)
    };
    rhi_byte_range(offset, destination.len(), size)?;
    rhi_require(
        usage.contains(crate::rhi::BufferUsage::TRANSFER_SOURCE),
        "buffer readback requires transferSource usage",
    )?;
    if destination.is_empty() {
        return Ok(());
    }
    let staging = make_staging_buffer(context, registries, destination.len())?;
    let outcome = one_shot(context, |cmd| {
        let copy = sys::VkBufferCopy {
            srcOffset: offset as sys::VkDeviceSize,
            dstOffset: 0,
            size: destination.len() as sys::VkDeviceSize,
        };
        // SAFETY: both buffers are live and the ranges were validated.
        unsafe {
            (context.draw.cmd_copy_buffer)(cmd, native, staging.buffer, 1, &copy);
        }
        Ok(())
    });
    if outcome.is_ok() {
        // SAFETY: the copy completed on the GPU and the staging mapping covers
        // the range; `destination` is a distinct live slice.
        unsafe {
            std::ptr::copy_nonoverlapping(
                staging.pointer,
                destination.as_mut_ptr(),
                destination.len(),
            )
        };
    }
    destroy_staging_buffer(context, registries, staging);
    outcome
}

/// A host-visible staging buffer plus its mapped pointer.
struct StagingBuffer {
    handle: crate::rhi::Buffer,
    buffer: sys::VkBuffer,
    pointer: *mut u8,
}

fn make_staging_buffer(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    size: usize,
) -> RhiResult<StagingBuffer> {
    // Transfer-only usage selects host-visible, coherent memory.
    let usage =
        crate::rhi::BufferUsage::TRANSFER_SOURCE | crate::rhi::BufferUsage::TRANSFER_DESTINATION;
    let handle = crate::rhi::Buffer::new(lock(registries).next_internal_id());
    let mut allocator = context
        .staging_allocator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    create_buffer(
        context,
        registries,
        &mut allocator,
        handle,
        crate::rhi::BufferDescriptor::new(size.max(1), usage),
    )?;
    let (buffer, pointer) = {
        let registries = lock(registries);
        let record = registries
            .buffers
            .get(&handle.id())
            .expect("new staging buffer is registered");
        (record.buffer, record.allocation.cpu_pointer(0))
    };
    let Some(pointer) = pointer else {
        destroy_buffer(context, registries, &mut allocator, handle);
        return Err(RhiError::OutOfMemory);
    };
    Ok(StagingBuffer {
        handle,
        buffer,
        pointer,
    })
}

fn destroy_staging_buffer(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    staging: StagingBuffer,
) {
    // The caller waited for the copy that used this buffer.
    let mut allocator = context
        .staging_allocator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    destroy_buffer(context, registries, &mut allocator, staging.handle);
}

/// Records and waits for one one-shot command buffer on the graphics queue.
///
/// This is the synchronous immediate-transfer path, which is why waiting here is
/// correct; the submit path never does this.
pub fn one_shot(
    context: &VulkanContext,
    body: impl FnOnce(sys::VkCommandBuffer) -> RhiResult<()>,
) -> RhiResult<()> {
    let mut scratch = context
        .scratch
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let pool = scratch.command_pool(context)?;
    // SAFETY: the pool belongs to this device and no recorded work references it.
    let reset = unsafe { (context.draw.reset_command_pool)(context.device, pool, 0) };
    rhi_require(
        reset == sys::VK_SUCCESS,
        "immediate command pool reset failed",
    )?;

    let allocate_info = sys::VkCommandBufferAllocateInfo {
        sType: sys::VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        pNext: core::ptr::null(),
        commandPool: pool,
        level: sys::VK_COMMAND_BUFFER_LEVEL_PRIMARY,
        commandBufferCount: 1,
    };
    let mut cmd: sys::VkCommandBuffer = core::ptr::null_mut();
    rhi_require(
        // SAFETY: the pool is live and the out-parameter is a live local.
        unsafe {
            (context.resources.allocate_command_buffers)(context.device, &allocate_info, &mut cmd)
        } == sys::VK_SUCCESS
            && !cmd.is_null(),
        "vkAllocateCommandBuffers failed",
    )?;

    let begin_info = sys::VkCommandBufferBeginInfo {
        sType: sys::VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
        pNext: core::ptr::null(),
        flags: sys::VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
        pInheritanceInfo: core::ptr::null(),
    };
    rhi_require(
        // SAFETY: `cmd` was just allocated and is not recording.
        unsafe { (context.draw.begin_command_buffer)(cmd, &begin_info) } == sys::VK_SUCCESS,
        "immediate command begin failed",
    )?;
    body(cmd)?;
    rhi_require(
        // SAFETY: `cmd` is recording.
        unsafe { (context.draw.end_command_buffer)(cmd) } == sys::VK_SUCCESS,
        "immediate command end failed",
    )?;

    let fence = scratch.fence(context)?;
    let info = sys::VkSubmitInfo {
        sType: sys::VK_STRUCTURE_TYPE_SUBMIT_INFO,
        pNext: core::ptr::null(),
        waitSemaphoreCount: 0,
        pWaitSemaphores: core::ptr::null(),
        pWaitDstStageMask: core::ptr::null(),
        commandBufferCount: 1,
        pCommandBuffers: &cmd,
        signalSemaphoreCount: 0,
        pSignalSemaphores: core::ptr::null(),
    };
    // SAFETY: `cmd` is ended and `info` outlives the call.
    let status =
        unsafe { (context.sync.queue_submit)(context.queues.graphics.handle, 1, &info, fence) };
    rhi_require(
        status == sys::VK_SUCCESS,
        "Vulkan immediate transfer submit failed",
    )?;
    let mut fences = [fence];
    rhi_require(
        // SAFETY: the fence belongs to this submission; wait indefinitely.
        unsafe {
            (context.sync.wait_for_fences)(
                context.device,
                1,
                fences.as_mut_ptr(),
                sys::VK_TRUE,
                u64::MAX,
            )
        } == sys::VK_SUCCESS,
        "immediate fence wait failed",
    )?;
    // SAFETY: the fence signalled.
    rhi_require(
        unsafe { (context.sync.reset_fences)(context.device, 1, fences.as_mut_ptr()) }
            == sys::VK_SUCCESS,
        "immediate fence reset failed",
    )
}

/// The one-shot command pool and fence immediate transfers reuse.
pub struct ScratchResources {
    pub command_pool: Option<sys::VkCommandPool>,
    pub fence: Option<sys::VkFence>,
}

impl ScratchResources {
    pub fn new() -> Self {
        Self {
            command_pool: None,
            fence: None,
        }
    }

    pub fn command_pool(&mut self, context: &VulkanContext) -> RhiResult<sys::VkCommandPool> {
        if let Some(pool) = self.command_pool {
            return Ok(pool);
        }
        let create_info = sys::VkCommandPoolCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: sys::VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT,
            queueFamilyIndex: context.queues.graphics.family,
        };
        let mut pool: sys::VkCommandPool = sys::VK_NULL_HANDLE;
        rhi_require(
            // SAFETY: `create_info` outlives the call and the out-parameter is a live
            // local.
            unsafe {
                (context.resources.create_command_pool)(
                    context.device,
                    &create_info,
                    core::ptr::null(),
                    &mut pool,
                )
            } == sys::VK_SUCCESS
                && pool != sys::VK_NULL_HANDLE,
            "vkCreateCommandPool failed",
        )?;
        self.command_pool = Some(pool);
        Ok(pool)
    }

    pub fn fence(&mut self, context: &VulkanContext) -> RhiResult<sys::VkFence> {
        if let Some(fence) = self.fence {
            return Ok(fence);
        }
        let create_info = sys::VkFenceCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
        };
        let mut fence: sys::VkFence = sys::VK_NULL_HANDLE;
        rhi_require(
            // SAFETY: `create_info` outlives the call and the out-parameter is a live
            // local.
            unsafe {
                (context.sync.create_fence)(
                    context.device,
                    &create_info,
                    core::ptr::null(),
                    &mut fence,
                )
            } == sys::VK_SUCCESS
                && fence != sys::VK_NULL_HANDLE,
            "vkCreateFence failed",
        )?;
        self.fence = Some(fence);
        Ok(fence)
    }
}

impl Default for ScratchResources {
    fn default() -> Self {
        Self::new()
    }
}

/// The aspect mask a texture format needs for transfers.
fn format_aspect(format: TextureFormat) -> u32 {
    formats::aspect_flags(format)
}

fn is_transient(registries: &VulkanRegistries, texture: Texture) -> bool {
    registries
        .textures
        .get(&texture.id())
        .is_some_and(|record| record.transient_attachment)
}

fn describe_texture(record: &VulkanTextureRecord) -> RhiResult<RenderTextureInfo<u32>> {
    let mut info = RenderTextureInfo::new(
        [record.width, record.height],
        record.sample_count,
        formats::vk_format(record.format),
    );
    info.usage = record.usage;
    info.single_layer_2d =
        record.dimension == crate::rhi::TextureDimension::Texture2D && record.layers == 1;
    info.is_depth = record.format.is_depth();
    info.supports_color_resolve = record.format != TextureFormat::R32Uint;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::ResourceState;

    #[test]
    fn state_layouts_cover_the_tracked_states() {
        assert_eq!(
            state_layout(ResourceState::RENDER_TARGET),
            sys::VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL
        );
        assert_eq!(
            state_layout(ResourceState::DEPTH_WRITE),
            sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL
        );
        assert_eq!(
            state_layout(ResourceState::COPY_SOURCE),
            sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL
        );
        assert_eq!(
            state_layout(ResourceState::COPY_DESTINATION),
            sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL
        );
        assert_eq!(
            state_layout(ResourceState::PRESENT),
            sys::VK_IMAGE_LAYOUT_PRESENT_SRC_KHR
        );
        assert_eq!(
            state_layout(ResourceState::SHADER_RESOURCE),
            sys::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL
        );
        assert_eq!(
            state_layout(ResourceState::UNORDERED_ACCESS),
            sys::VK_IMAGE_LAYOUT_GENERAL
        );
        // Both copy directions together need GENERAL, since one layout must serve.
        assert_eq!(
            state_layout(ResourceState::COPY_SOURCE | ResourceState::COPY_DESTINATION),
            sys::VK_IMAGE_LAYOUT_GENERAL
        );
    }

    #[test]
    fn state_access_unions_the_tracked_bits() {
        assert_eq!(state_access(ResourceState::EMPTY), 0);
        let both = state_access(ResourceState::UNORDERED_ACCESS);
        assert_ne!(both & sys::VK_ACCESS_SHADER_READ_BIT, 0);
        assert_ne!(both & sys::VK_ACCESS_SHADER_WRITE_BIT, 0);
        let target = state_access(ResourceState::RENDER_TARGET);
        assert_ne!(target & sys::VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT, 0);
        let acceleration = state_access(ResourceState::ACCELERATION_STRUCTURE_WRITE);
        assert_ne!(
            acceleration & sys::VK_ACCESS_ACCELERATION_STRUCTURE_WRITE_BIT_KHR,
            0
        );
    }

    #[test]
    fn layout_access_inverts_the_layout_selection() {
        assert_eq!(layout_access(sys::VK_IMAGE_LAYOUT_UNDEFINED), 0);
        assert_eq!(
            layout_access(sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL),
            sys::VK_ACCESS_TRANSFER_READ_BIT
        );
        assert_eq!(
            layout_access(sys::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL),
            sys::VK_ACCESS_SHADER_READ_BIT
        );
    }

    #[test]
    fn labels_name_passes_by_kind_and_ordinal() {
        use crate::rhi::{PlannedCommand, RenderPassDescriptor, RenderPassRecord};
        let commands = vec![
            PlannedCommand::RenderPass(RenderPassRecord {
                descriptor: RenderPassDescriptor::new(Vec::new()).with_label("scene"),
                body: Vec::new(),
            }),
            PlannedCommand::ComputePass(crate::rhi::ComputePassRecord { body: Vec::new() }),
            PlannedCommand::ComputePass(crate::rhi::ComputePassRecord { body: Vec::new() }),
        ];
        assert_eq!(
            encode_labels(&commands),
            vec!["scene", "computePass#0", "computePass#1"]
        );
    }

    #[test]
    fn scratch_resources_are_created_lazily() {
        let scratch = ScratchResources::new();
        assert!(scratch.command_pool.is_none());
        assert!(scratch.fence.is_none());
    }
}
