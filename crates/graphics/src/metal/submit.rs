//! Translation of NativeRHI/Metal/Pipelines/MetalSubmitEncoder.swift:
//! `PlannedSubmit` → `MTLCommandBuffer` encoding.
//!
//! Submission is asynchronous: work is encoded into an `MTLCommandBuffer`,
//! timeline semaphores map to `MTLSharedEvent` encode-wait/signal, and the
//! frontend completion callback fires from the completed handler. There is no
//! `waitUntilCompleted` and no per-submit idle.
use std::sync::Arc;
use std::sync::Mutex;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLAccelerationStructureCommandEncoder, MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer,
    MTLCommandBufferDescriptor, MTLCommandEncoder, MTLCommandQueue, MTLComputeCommandEncoder,
    MTLDevice, MTLIndexType, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPassDepthAttachmentDescriptor, MTLRenderPassDescriptor, MTLResource,
    MTLResourceOptions, MTLResourceUsage, MTLSamplerState, MTLScissorRect, MTLSize, MTLStorageMode,
    MTLTexture, MTLViewport,
};

use crate::rhi::{
    ColorLoadAction, Completion, ComputeCommand, ComputePassRecord, CopyCommand, CopyPassRecord,
    CpuProfilePhase, DepthLoadAction, GpuTimestampMarker, MeshPipeline, PlannedCommand,
    PlannedSubmit, QueueClass, RenderCommand, RenderPassRecord, RenderTextureInfo, RhiError,
    RhiResult, SubmissionCpuProfile, Texture, TextureUsage, rhi_byte_range,
    rhi_color_texture_copy_extent, rhi_count, rhi_render_pass_signature, rhi_require,
    rhi_texture_subresource_extent, rhi_texture_transfer_bytes, rhi_texture_upload_bytes,
};

use super::device::{MetalDeviceHandle, VERTEX_BUFFER_BASE_INDEX};
use super::formats;
use super::registries::{
    MetalBindingSet, MetalBoundEntry, MetalBoundValue, MetalDynamicKey, MetalMeshPipeline,
};
use super::resources::lock;
use super::transfer::as_event;

/// Encodes one planned submission and returns once it is queued.
pub fn submit(
    device: &mut MetalDeviceHandle,
    submit: &PlannedSubmit,
    mut cpu_profile: Option<&mut SubmissionCpuProfile>,
    completion: Completion,
) -> RhiResult<()> {
    device.submission_status.check()?;
    let encoding_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
    let queue = device.queue_for(submit.queue).clone();
    let descriptor = { MTLCommandBufferDescriptor::new() };
    if device.config.enable_validation {
        descriptor
            .setErrorOptions(objc2_metal::MTLCommandBufferErrorOption::EncoderExecutionStatus);
    }
    let command_buffer = { queue.commandBufferWithDescriptor(&descriptor) }
        .ok_or_else(|| RhiError::submit_failed("cannot create command buffer"))?;

    // Wait on timeline semaphores (cross-queue release fences).
    for semaphore in &submit.wait_semaphores {
        let event = device.shared_event(semaphore.id).ok_or_else(|| {
            RhiError::submit_failed(format!(
                "cannot create shared event for wait semaphore {}",
                semaphore.id
            ))
        })?;
        command_buffer.encodeWaitForEvent_value(as_event(&event), semaphore.value);
    }

    let registries = Arc::clone(&device.registries);
    let mut encoder = PassEncoder::new(
        registries,
        device.device.clone(),
        device.disabled_depth_state.clone(),
        &command_buffer,
    );
    let encode_result = (|| -> RhiResult<()> {
        for command in &submit.commands {
            encoder.encode(command, cpu_profile.as_deref_mut())?;
        }
        Ok(())
    })();
    encoder.end_active_encoders();
    encode_result?;

    // Signal timeline semaphores when this buffer completes.
    for semaphore in &submit.signal_semaphores {
        let event = device.shared_event(semaphore.id).ok_or_else(|| {
            RhiError::submit_failed(format!(
                "cannot create shared event for signal semaphore {}",
                semaphore.id
            ))
        })?;
        command_buffer.encodeSignalEvent_value(as_event(&event), semaphore.value);
    }

    // Capture the profile for this submission now; later frames may detach it
    // before this buffer reaches its GPU completion handler.
    let gpu_profile = device.gpu_timestamp_profile.clone();
    let gpu_label = match submit.queue {
        QueueClass::Compute => "compute submission",
        QueueClass::Graphics | QueueClass::Transfer => "graphics submission",
    };
    let status = Arc::clone(&device.submission_status);
    let user_completion = Arc::new(Mutex::new(Some(completion)));
    let block = block2::RcBlock::new(
        move |buffer: core::ptr::NonNull<ProtocolObject<dyn MTLCommandBuffer>>| {
            // SAFETY: Metal passes a live command buffer for the duration of the
            // callback.
            let buffer = unsafe { buffer.as_ref() };
            if let Some(error) = buffer.error() {
                status.record(error.localizedDescription().to_string());
            }
            if let Some(profile) = &gpu_profile {
                note_gpu_timestamps(profile, buffer, gpu_label);
            }
            let taken = user_completion
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(completion) = taken {
                completion();
            }
        },
    );
    // SAFETY: `as_ptr` yields the `*mut DynBlock` shape the handler type
    // declares, and `addCompletedHandler:` copies the block, so the pointer
    // stays valid for exactly this call.
    unsafe { command_buffer.addCompletedHandler(block2::RcBlock::as_ptr(&block) as *mut _) };

    if let Some(profile) = cpu_profile.as_deref_mut() {
        profile.end(CpuProfilePhase::Encoding, encoding_start);
    }
    let queue_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
    command_buffer.commit();
    if let Some(profile) = cpu_profile {
        profile.end(CpuProfilePhase::QueueSubmit, queue_start);
    }
    Ok(())
}

/// Records one GPU timing marker for a completed command buffer. A buffer that
/// did not expose finished timestamps is reported by status, never as zero.
fn note_gpu_timestamps(
    profile: &crate::rhi::GpuTimestampProfile,
    buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    label: &str,
) {
    let seconds = buffer.GPUEndTime() - buffer.GPUStartTime();
    if buffer.status() == objc2_metal::MTLCommandBufferStatus::Completed
        && buffer.GPUStartTime() > 0.0
        && seconds.is_finite()
        && seconds > 0.0
        && seconds < u64::MAX as f64 / 1e9
    {
        profile.note("ok");
        profile.append(GpuTimestampMarker::new(label, (seconds * 1e9) as u64));
    } else {
        profile.note("Metal command buffer did not expose completed GPU timestamps");
    }
}

/// Per-submission encoder state. Metal needs the current pipeline, index buffer
/// and primitive type remembered between commands, so they live here.
struct PassEncoder<'a> {
    registries: Arc<Mutex<super::registries::MetalRegistries>>,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    command_buffer: &'a ProtocolObject<dyn MTLCommandBuffer>,
    render_encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,
    render_bindings: Option<super::render_bindings::MetalRenderBindings>,
    compute_encoder: Option<Retained<ProtocolObject<dyn MTLComputeCommandEncoder>>>,
    current_mesh_pipeline: Option<MeshPipeline>,
    compute_threadgroup_size: crate::rhi::ThreadgroupSize,
    current_primitive: MTLPrimitiveType,
    current_index_buffer: Option<Retained<ProtocolObject<dyn MTLBuffer>>>,
    current_index_offset: usize,
    current_index_type: MTLIndexType,
    disabled_depth_state: Retained<ProtocolObject<dyn objc2_metal::MTLDepthStencilState>>,
}

impl<'a> PassEncoder<'a> {
    fn new(
        registries: Arc<Mutex<super::registries::MetalRegistries>>,
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        disabled_depth_state: Retained<ProtocolObject<dyn objc2_metal::MTLDepthStencilState>>,
        command_buffer: &'a ProtocolObject<dyn MTLCommandBuffer>,
    ) -> Self {
        Self {
            registries,
            device,
            command_buffer,
            render_encoder: None,
            render_bindings: None,
            compute_encoder: None,
            current_mesh_pipeline: None,
            compute_threadgroup_size: crate::rhi::ThreadgroupSize::default(),
            current_primitive: MTLPrimitiveType::Triangle,
            current_index_buffer: None,
            current_index_offset: 0,
            current_index_type: MTLIndexType::UInt32,
            disabled_depth_state,
        }
    }

    fn encode(
        &mut self,
        command: &PlannedCommand,
        mut cpu_profile: Option<&mut SubmissionCpuProfile>,
    ) -> RhiResult<()> {
        match command {
            PlannedCommand::AccelerationStructureBuild(build) => {
                self.end_active_encoders();
                let (structure, descriptor, scratch_size) = {
                    let registries = lock(&self.registries);
                    let native = registries
                        .acceleration_structures
                        .get(&build.structure.id())
                        .ok_or_else(|| {
                            RhiError::invalid_argument(
                                "unknown acceleration structure build target",
                            )
                        })?;
                    (
                        native.structure.clone(),
                        native.descriptor.clone(),
                        native.scratch_size,
                    )
                };
                let scratch = self
                    .device
                    .newBufferWithLength_options(
                        scratch_size.max(1),
                        MTLResourceOptions::StorageModePrivate,
                    )
                    .ok_or(RhiError::OutOfMemory)?;
                let encoder = self
                    .command_buffer
                    .accelerationStructureCommandEncoder()
                    .ok_or(RhiError::OutOfMemory)?;
                encoder.buildAccelerationStructure_descriptor_scratchBuffer_scratchBufferOffset(
                    &structure,
                    &descriptor,
                    &scratch,
                    0,
                );
                encoder.endEncoding();
                Ok(())
            }
            // Metal tracks resource hazards automatically within a command
            // buffer; cross-queue fences are expressed via shared events.
            PlannedCommand::Barriers(_) => Ok(()),
            PlannedCommand::RenderPass(record) => {
                self.end_active_encoders();
                self.encode_render_pass(record, cpu_profile)
            }
            PlannedCommand::ComputePass(record) => {
                self.end_active_encoders();
                self.encode_compute_pass(record)
            }
            PlannedCommand::CopyPass(record) => {
                self.end_active_encoders();
                self.encode_copy_pass(record)
            }
        }
    }

    fn end_active_encoders(&mut self) {
        if let Some(encoder) = self.render_encoder.take() {
            encoder.endEncoding();
        }
        self.render_bindings = None;
        if let Some(encoder) = self.compute_encoder.take() {
            encoder.endEncoding();
        }
    }

    // MARK: Render pass

    /// Builds and validates the attachment signature, and enforces the
    /// memoryless-attachment contract Swift checks before encoding.
    fn validate_render_attachments(
        &self,
        record: &RenderPassRecord,
    ) -> RhiResult<crate::rhi::RenderPassSignature<objc2_metal::MTLPixelFormat>> {
        let registries = &self.registries;
        let signature = {
            let registries = lock(registries);
            rhi_render_pass_signature(&record.descriptor, |handle| {
                let texture = registries
                    .textures
                    .get(&handle.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown render attachment"))?;
                describe_texture(texture)
            })?
        };
        {
            let registries = lock(registries);
            for target in &record.descriptor.color_targets {
                if is_memoryless(&registries, target.texture) {
                    if matches!(target.load_action, ColorLoadAction::Load) {
                        return Err(RhiError::invalid_argument(
                            "memoryless color attachments cannot load prior contents",
                        ));
                    }
                    rhi_require(
                        !target.store,
                        "memoryless color attachments cannot store multisample contents",
                    )?;
                }
            }
            if let Some(target) = record.descriptor.depth_target
                && is_memoryless(&registries, target.texture)
            {
                if matches!(target.load_action, DepthLoadAction::Load) {
                    return Err(RhiError::invalid_argument(
                        "memoryless depth attachments cannot load prior contents",
                    ));
                }
                rhi_require(
                    !target.store,
                    "memoryless depth attachments cannot store contents",
                )?;
            }
        }

        for command in &record.body {
            match command {
                RenderCommand::SetPipeline(handle) => {
                    let registries = lock(registries);
                    let pipeline = registries
                        .render_pipelines
                        .get(&handle.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown graphics pipeline"))?;
                    let descriptor = &pipeline.descriptor;
                    let colors: Vec<_> = descriptor
                        .color_attachments
                        .iter()
                        .map(|attachment| formats::pixel_format(attachment.format))
                        .collect();
                    let depth = descriptor.depth_format.map(formats::pixel_format);
                    signature.validate(&colors, depth.as_ref(), descriptor.sample_count)?;
                }
                RenderCommand::SetMeshPipeline(handle) => {
                    let registries = lock(registries);
                    let pipeline = registries
                        .mesh_pipelines
                        .get(&handle.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown mesh pipeline"))?;
                    let descriptor = &pipeline.descriptor;
                    let colors: Vec<_> = descriptor
                        .color_attachments
                        .iter()
                        .map(|attachment| formats::pixel_format(attachment.format))
                        .collect();
                    let depth = descriptor.depth_format.map(formats::pixel_format);
                    signature.validate(&colors, depth.as_ref(), 1)?;
                }
                _ => {}
            }
        }
        Ok(signature)
    }

    fn encode_render_pass(
        &mut self,
        record: &RenderPassRecord,
        mut cpu_profile: Option<&mut SubmissionCpuProfile>,
    ) -> RhiResult<()> {
        let setup_start = cpu_profile
            .as_deref()
            .and_then(|profile| profile.begin_detail(CpuProfilePhase::PassSetup));
        self.validate_render_attachments(record)?;
        let pass_descriptor = { MTLRenderPassDescriptor::new() };
        let color_attachments = pass_descriptor.colorAttachments();

        {
            let registries = lock(&self.registries);
            for (index, target) in record.descriptor.color_targets.iter().enumerate() {
                rhi_require(index < 8, "invalid render color target")?;
                let texture = registries
                    .textures
                    .get(&target.texture.id())
                    .ok_or_else(|| RhiError::invalid_argument("invalid render color target"))?;
                // SAFETY: the check above keeps the index inside the eight
                // slots the attachment array exposes.
                let attachment = unsafe { color_attachments.objectAtIndexedSubscript(index) };
                attachment.setTexture(Some(texture));
                attachment.setLoadAction(formats::load_action(target.load_action));
                attachment.setStoreAction(if target.store {
                    objc2_metal::MTLStoreAction::Store
                } else {
                    objc2_metal::MTLStoreAction::DontCare
                });
                if let Some(resolve) = target.resolve_texture {
                    attachment.setResolveTexture(
                        registries.textures.get(&resolve.id()).map(|value| &**value),
                    );
                    attachment.setStoreAction(if target.store {
                        objc2_metal::MTLStoreAction::StoreAndMultisampleResolve
                    } else {
                        objc2_metal::MTLStoreAction::MultisampleResolve
                    });
                }
                if let ColorLoadAction::Clear(color) = target.load_action {
                    attachment.setClearColor(objc2_metal::MTLClearColor {
                        red: color[0] as f64,
                        green: color[1] as f64,
                        blue: color[2] as f64,
                        alpha: color[3] as f64,
                    });
                }
            }

            if let Some(depth) = record.descriptor.depth_target {
                let texture = registries
                    .textures
                    .get(&depth.texture.id())
                    .ok_or_else(|| {
                        RhiError::invalid_argument("render pass references unknown depth texture")
                    })?;
                let attachment: Retained<MTLRenderPassDepthAttachmentDescriptor> =
                    pass_descriptor.depthAttachment();
                attachment.setTexture(Some(texture));
                attachment.setLoadAction(formats::depth_load_action(depth.load_action));
                attachment.setStoreAction(if depth.store {
                    objc2_metal::MTLStoreAction::Store
                } else {
                    objc2_metal::MTLStoreAction::DontCare
                });
                if let DepthLoadAction::Clear(value) = depth.load_action {
                    attachment.setClearDepth(value);
                }
            }
        }

        if let Some(profile) = cpu_profile.as_deref_mut() {
            profile.end_detail(CpuProfilePhase::PassSetup, setup_start);
        }
        let create_start = cpu_profile
            .as_deref()
            .and_then(|profile| profile.begin_detail(CpuProfilePhase::EncoderCreation));
        let encoder = self
            .command_buffer
            .renderCommandEncoderWithDescriptor(&pass_descriptor)
            .ok_or_else(|| RhiError::submit_failed("cannot create render command encoder"))?;
        self.render_encoder = Some(encoder);
        self.render_bindings = Some(super::render_bindings::MetalRenderBindings::new());
        self.current_index_buffer = None;
        self.current_mesh_pipeline = None;
        if let Some(profile) = cpu_profile.as_deref_mut() {
            profile.end_detail(CpuProfilePhase::EncoderCreation, create_start);
        }

        let replay_start = cpu_profile
            .as_deref()
            .and_then(|profile| profile.begin_detail(CpuProfilePhase::CommandReplay));
        for command in &record.body {
            self.encode_render_command(command, cpu_profile.as_deref_mut())?;
        }
        if let Some(profile) = cpu_profile {
            profile.end_detail(CpuProfilePhase::CommandReplay, replay_start);
        }
        Ok(())
    }

    fn encode_render_command(
        &mut self,
        command: &RenderCommand,
        mut cpu_profile: Option<&mut SubmissionCpuProfile>,
    ) -> RhiResult<()> {
        let name = |command: &RenderCommand| {
            format!("{command:?}")
                .split(['(', ' '])
                .next()
                .unwrap_or("command")
                .to_owned()
        };
        let _ = name(command);
        let Some(encoder) = self.render_encoder.clone() else {
            return Err(RhiError::invalid_argument("render encoder is not active"));
        };
        match command {
            RenderCommand::SetMeshPipeline(handle) => {
                let pipeline = {
                    let registries = lock(&self.registries);
                    let mesh = registries
                        .mesh_pipelines
                        .get(&handle.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown mesh pipeline"))?;
                    (
                        mesh.state.clone(),
                        mesh.rasterization,
                        mesh.descriptor.clone(),
                    )
                };
                self.current_mesh_pipeline = Some(*handle);
                encoder.setRenderPipelineState(&pipeline.0);
                encoder.setDepthStencilState(Some(&self.disabled_depth_state));
                encoder.setCullMode(formats::cull_mode(pipeline.1.cull_mode));
                encoder.setFrontFacingWinding(formats::winding(pipeline.1.front_winding));
                encoder.setTriangleFillMode(formats::triangle_fill_mode(pipeline.1.fill_mode));
                Ok(())
            }
            RenderCommand::DrawMeshTasks(grid) => {
                grid.validate()?;
                let Some(handle) = self.current_mesh_pipeline else {
                    return Err(RhiError::invalid_argument(
                        "drawMeshTasks requires a mesh pipeline",
                    ));
                };
                let (mesh_size, task_size) = {
                    let registries = lock(&self.registries);
                    let mesh: &MetalMeshPipeline = registries
                        .mesh_pipelines
                        .get(&handle.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown mesh pipeline"))?;
                    (mesh.mesh_size, mesh.task_size)
                };
                encoder.drawMeshThreadgroups_threadsPerObjectThreadgroup_threadsPerMeshThreadgroup(
                    MTLSize {
                        width: grid.x as usize,
                        height: grid.y as usize,
                        depth: grid.z as usize,
                    },
                    MTLSize {
                        width: task_size.x as usize,
                        height: task_size.y as usize,
                        depth: task_size.z as usize,
                    },
                    MTLSize {
                        width: mesh_size.x as usize,
                        height: mesh_size.y as usize,
                        depth: mesh_size.z as usize,
                    },
                );
                Ok(())
            }
            RenderCommand::SetPipeline(handle) => {
                self.current_mesh_pipeline = None;
                let (state, primitive, depth_state, raster) = {
                    let registries = lock(&self.registries);
                    let pipeline = registries
                        .render_pipelines
                        .get(&handle.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown graphics pipeline"))?;
                    (
                        pipeline.state.clone(),
                        registries
                            .pipeline_primitives
                            .get(&handle.id())
                            .copied()
                            .unwrap_or(MTLPrimitiveType::Triangle),
                        registries.depth_states.get(&handle.id()).cloned(),
                        registries.pipeline_raster_states.get(&handle.id()).copied(),
                    )
                };
                encoder.setRenderPipelineState(&state);
                self.current_primitive = primitive;
                encoder.setDepthStencilState(Some(
                    depth_state.as_deref().unwrap_or(&self.disabled_depth_state),
                ));
                if let Some(raster) = raster {
                    encoder.setCullMode(formats::cull_mode(raster.cull_mode));
                    encoder.setFrontFacingWinding(formats::winding(raster.front_winding));
                    encoder.setTriangleFillMode(formats::triangle_fill_mode(raster.fill_mode));
                }
                Ok(())
            }
            RenderCommand::SetBindingSet {
                slot,
                set,
                dynamic_offsets,
            } => {
                rhi_require(
                    *slot == 0,
                    "Metal direct binding currently supports set 0 only",
                )?;
                let bind_start = cpu_profile
                    .as_deref()
                    .and_then(|profile| profile.begin_detail(CpuProfilePhase::BindingApply));
                self.bind_render_binding_set(*set, dynamic_offsets, &encoder)?;
                if let Some(profile) = cpu_profile {
                    profile.end_detail(CpuProfilePhase::BindingApply, bind_start);
                }
                Ok(())
            }
            RenderCommand::SetVertexBuffer {
                slot,
                buffer,
                offset,
            } => {
                let native = {
                    let registries = lock(&self.registries);
                    let native = registries
                        .buffers
                        .get(&buffer.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                    rhi_byte_range(*offset, 0, native.length())?;
                    native.clone()
                };
                let index = VERTEX_BUFFER_BASE_INDEX as usize + *slot as usize;
                // SAFETY: the offset was validated inside the buffer and the
                // buffer outlives the encoder's use of it.
                unsafe { encoder.setVertexBuffer_offset_atIndex(Some(&native), *offset, index) };
                Ok(())
            }
            RenderCommand::SetIndexBuffer {
                buffer,
                offset,
                index_type,
            } => {
                let native = {
                    let registries = lock(&self.registries);
                    let native = registries
                        .buffers
                        .get(&buffer.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                    rhi_byte_range(*offset, 0, native.length())?;
                    native.clone()
                };
                self.current_index_buffer = Some(native);
                self.current_index_offset = *offset;
                self.current_index_type = formats::index_type(*index_type);
                Ok(())
            }
            RenderCommand::PushConstant { stage, slot, data } => {
                if !data.is_empty()
                    && let Some(bindings) = self.render_bindings.as_mut()
                {
                    bindings.invalidate_buffer(*slot, *stage);
                }
                if data.is_empty() {
                    return Ok(());
                }
                let index = *slot as usize;
                // SAFETY: `data` is a live slice of `data.len()` bytes, which is
                // exactly the length passed, and Metal copies it before return.
                unsafe {
                    match stage {
                        crate::rhi::ShaderStage::Vertex => encoder.setVertexBytes_length_atIndex(
                            core::ptr::NonNull::new(data.as_ptr() as *mut core::ffi::c_void)
                                .expect("a non-empty slice has a pointer"),
                            data.len(),
                            index,
                        ),
                        crate::rhi::ShaderStage::Fragment => encoder
                            .setFragmentBytes_length_atIndex(
                                core::ptr::NonNull::new(data.as_ptr() as *mut core::ffi::c_void)
                                    .expect("a non-empty slice has a pointer"),
                                data.len(),
                                index,
                            ),
                        crate::rhi::ShaderStage::Task => encoder.setObjectBytes_length_atIndex(
                            core::ptr::NonNull::new(data.as_ptr() as *mut core::ffi::c_void)
                                .expect("a non-empty slice has a pointer"),
                            data.len(),
                            index,
                        ),
                        crate::rhi::ShaderStage::Mesh => encoder.setMeshBytes_length_atIndex(
                            core::ptr::NonNull::new(data.as_ptr() as *mut core::ffi::c_void)
                                .expect("a non-empty slice has a pointer"),
                            data.len(),
                            index,
                        ),
                        crate::rhi::ShaderStage::Compute => {}
                    }
                }
                Ok(())
            }
            RenderCommand::SetViewport(viewport) => {
                encoder.setViewport(MTLViewport {
                    originX: viewport.x,
                    originY: viewport.y,
                    width: viewport.width,
                    height: viewport.height,
                    znear: viewport.min_depth,
                    zfar: viewport.max_depth,
                });
                Ok(())
            }
            RenderCommand::SetScissor(scissor) => {
                encoder.setScissorRect(MTLScissorRect {
                    x: scissor.x as usize,
                    y: scissor.y as usize,
                    width: scissor.width as usize,
                    height: scissor.height as usize,
                });
                Ok(())
            }
            RenderCommand::Draw {
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            } => {
                // SAFETY: the counts are validated as non-negative below, and
                // the pipeline/vertex layout come from this device's registry.
                unsafe {
                    encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                        self.current_primitive,
                        rhi_count(*first_vertex)? as usize,
                        rhi_count(*vertex_count)? as usize,
                        rhi_count(*instance_count)? as usize,
                        rhi_count(*first_instance)? as usize,
                    )
                };
                Ok(())
            }
            RenderCommand::DrawIndexed(args) => {
                let Some(index_buffer) = self.current_index_buffer.clone() else {
                    return Err(RhiError::invalid_argument(
                        "drawIndexed requires an index buffer",
                    ));
                };
                let index_size = if self.current_index_type == MTLIndexType::UInt16 {
                    2
                } else {
                    4
                };
                let first_index = rhi_count(args.first_index)? as usize;
                let offset = self.current_index_offset + first_index * index_size;
                // SAFETY: the index range was validated when the buffer was
                // bound, and the buffer is retained for the encoder's lifetime.
                unsafe {
                    encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount_baseVertex_baseInstance(
                        self.current_primitive,
                        rhi_count(args.index_count)? as usize,
                        self.current_index_type,
                        &index_buffer,
                        offset,
                        rhi_count(args.instance_count)? as usize,
                        args.vertex_offset as isize,
                        rhi_count(args.first_instance)? as usize,
                    )
                };
                Ok(())
            }
            RenderCommand::DrawIndirect {
                buffer,
                offset,
                draw_count,
            } => {
                let native = {
                    let registries = lock(&self.registries);
                    registries
                        .buffers
                        .get(&buffer.id())
                        .cloned()
                        .ok_or_else(|| RhiError::invalid_argument("unknown indirect draw buffer"))?
                };
                let stride =
                    core::mem::size_of::<objc2_metal::MTLDrawPrimitivesIndirectArguments>();
                let count = rhi_count(*draw_count)? as usize;
                let size = count.checked_mul(stride).ok_or_else(|| {
                    RhiError::invalid_argument("indirect draw arguments exceed buffer bounds")
                })?;
                rhi_require(
                    count > 0
                        && offset % 4 == 0
                        && *offset <= native.length()
                        && size <= native.length() - *offset,
                    "indirect draw arguments exceed buffer bounds",
                )?;
                for index in 0..count {
                    // SAFETY: the stride table above proved every argument block
                    // fits inside the bound buffer.
                    unsafe {
                        encoder.drawPrimitives_indirectBuffer_indirectBufferOffset(
                            self.current_primitive,
                            &native,
                            *offset + index * stride,
                        )
                    };
                }
                Ok(())
            }
        }
    }

    /// Replays a pre-resolved apply list. Resources, offsets and stage
    /// visibility were resolved when the set was registered, so the hot path
    /// only compares against the encoder's current bindings.
    fn bind_render_binding_set(
        &mut self,
        handle: crate::rhi::BindingSet,
        dynamic_offsets: &[u32],
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> RhiResult<()> {
        let Some(binding_set) = ({
            let registries = lock(&self.registries);
            registries
                .binding_sets
                .get(handle.id())
                .map(shallow_clone_binding_set)
        }) else {
            return Err(RhiError::invalid_argument("unknown render binding set"));
        };
        let unique_positions: std::collections::HashSet<usize> =
            binding_set.dynamic_offset_index.values().copied().collect();
        rhi_require(
            dynamic_offsets.len() == unique_positions.len(),
            "dynamic offset count does not match the binding layout",
        )?;
        let applies = if self.current_mesh_pipeline.is_none() {
            &binding_set.graphics_applies
        } else {
            &binding_set.mesh_applies
        };
        for apply in applies {
            match &apply.resource {
                MetalBoundValue::Sampler(state) => {
                    self.apply_sampler(state, apply.stage, apply.index, encoder)
                }
                MetalBoundValue::Texture(texture) => {
                    self.apply_texture(texture, apply.stage, apply.index, encoder)
                }
                MetalBoundValue::Buffer { buffer, offset } => {
                    let resolved = binding_set
                        .dynamic_offset_index
                        .get(&MetalDynamicKey {
                            stage: apply.stage,
                            index: apply.index,
                        })
                        .map(|position| dynamic_offsets[*position] as usize)
                        .unwrap_or(*offset);
                    self.apply_buffer(buffer, resolved, apply.stage, apply.index, encoder);
                }
                MetalBoundValue::AccelerationStructure(_) => {}
            }
        }
        Ok(())
    }

    fn apply_sampler(
        &mut self,
        state: &Retained<ProtocolObject<dyn MTLSamplerState>>,
        stage: crate::rhi::ShaderStage,
        index: usize,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) {
        let Some(bindings) = self.render_bindings.as_mut() else {
            return;
        };
        let slot = index as u32;
        if !bindings.sampler_changed(state, slot, stage) {
            return;
        }
        match stage {
            crate::rhi::ShaderStage::Vertex => {
                // SAFETY: the sampler comes from the registry and outlives the
                // encoder's use of it.
                unsafe { encoder.setVertexSamplerState_atIndex(Some(state), index) }
            }
            crate::rhi::ShaderStage::Fragment => {
                // SAFETY: same as the vertex case above.
                unsafe { encoder.setFragmentSamplerState_atIndex(Some(state), index) }
            }
            crate::rhi::ShaderStage::Mesh => {
                // SAFETY: same as the vertex case above.
                unsafe { encoder.setMeshSamplerState_atIndex(Some(state), index) }
            }
            crate::rhi::ShaderStage::Task => {
                // SAFETY: same as the vertex case above.
                unsafe { encoder.setObjectSamplerState_atIndex(Some(state), index) }
            }
            crate::rhi::ShaderStage::Compute => {}
        }
    }

    fn apply_texture(
        &mut self,
        texture: &Retained<ProtocolObject<dyn MTLTexture>>,
        stage: crate::rhi::ShaderStage,
        index: usize,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) {
        let Some(bindings) = self.render_bindings.as_mut() else {
            return;
        };
        let slot = index as u32;
        if !bindings.texture_changed(texture, slot, stage) {
            return;
        }
        match stage {
            crate::rhi::ShaderStage::Vertex => {
                // SAFETY: the texture comes from the registry and outlives the
                // encoder's use of it.
                unsafe { encoder.setVertexTexture_atIndex(Some(texture), index) }
            }
            crate::rhi::ShaderStage::Fragment => {
                // SAFETY: same as the vertex case above.
                unsafe { encoder.setFragmentTexture_atIndex(Some(texture), index) }
            }
            crate::rhi::ShaderStage::Mesh => {
                // SAFETY: same as the vertex case above.
                unsafe { encoder.setMeshTexture_atIndex(Some(texture), index) }
            }
            crate::rhi::ShaderStage::Task => {
                // SAFETY: same as the vertex case above.
                unsafe { encoder.setObjectTexture_atIndex(Some(texture), index) }
            }
            crate::rhi::ShaderStage::Compute => {}
        }
    }

    fn apply_buffer(
        &mut self,
        buffer: &Retained<ProtocolObject<dyn MTLBuffer>>,
        offset: usize,
        stage: crate::rhi::ShaderStage,
        index: usize,
        encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) {
        let Some(bindings) = self.render_bindings.as_mut() else {
            return;
        };
        let slot = index as u32;
        if !bindings.buffer_changed(buffer, offset, slot, stage) {
            return;
        }
        // SAFETY: the offset was range-checked at set registration and the
        // buffer outlives the encoder's use of it.
        unsafe {
            match stage {
                crate::rhi::ShaderStage::Vertex => {
                    encoder.setVertexBuffer_offset_atIndex(Some(buffer), offset, index)
                }
                crate::rhi::ShaderStage::Fragment => {
                    encoder.setFragmentBuffer_offset_atIndex(Some(buffer), offset, index)
                }
                crate::rhi::ShaderStage::Mesh => {
                    encoder.setMeshBuffer_offset_atIndex(Some(buffer), offset, index)
                }
                crate::rhi::ShaderStage::Task => {
                    encoder.setObjectBuffer_offset_atIndex(Some(buffer), offset, index)
                }
                crate::rhi::ShaderStage::Compute => {}
            }
        }
    }

    // MARK: Compute pass

    fn encode_compute_pass(&mut self, record: &ComputePassRecord) -> RhiResult<()> {
        let encoder = self
            .command_buffer
            .computeCommandEncoder()
            .ok_or_else(|| RhiError::submit_failed("cannot create compute command encoder"))?;
        // Install the encoder before encoding anything. Swift assigns
        // `computeEncoder` first for exactly this reason: an encoding failure
        // must still leave a live encoder for `endActiveEncoders()` to end.
        // Otherwise the abandoned encoder is released without `endEncoding()`
        // and Metal aborts the process on dealloc.
        self.compute_encoder = Some(encoder);
        self.compute_threadgroup_size = crate::rhi::ThreadgroupSize::default();
        let encoder = self
            .compute_encoder
            .as_ref()
            .expect("the encoder was just installed")
            .clone();
        for command in &record.body {
            self.encode_compute_command(command, &encoder)?;
        }
        Ok(())
    }

    fn encode_compute_command(
        &mut self,
        command: &ComputeCommand,
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    ) -> RhiResult<()> {
        match command {
            ComputeCommand::SetPipeline(handle) => {
                let (state, size) = {
                    let registries = lock(&self.registries);
                    let state = registries
                        .compute_pipelines
                        .get(&handle.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown compute pipeline"))?;
                    (
                        state.clone(),
                        registries
                            .compute_threadgroup_sizes
                            .get(&handle.id())
                            .copied()
                            .unwrap_or_default(),
                    )
                };
                self.compute_threadgroup_size = size;
                encoder.setComputePipelineState(&state);
                Ok(())
            }
            ComputeCommand::SetBindingSet {
                slot,
                set,
                dynamic_offsets,
            } => {
                rhi_require(
                    *slot == 0,
                    "Metal direct binding currently supports set 0 only",
                )?;
                let Some(binding_set) = ({
                    let registries = lock(&self.registries);
                    registries
                        .binding_sets
                        .get(set.id())
                        .map(shallow_clone_binding_set)
                }) else {
                    return Err(RhiError::invalid_argument("unknown compute binding set"));
                };
                let unique_positions: std::collections::HashSet<usize> =
                    binding_set.dynamic_offset_index.values().copied().collect();
                rhi_require(
                    dynamic_offsets.len() == unique_positions.len(),
                    "dynamic offset count does not match the binding layout",
                )?;
                for apply in &binding_set.compute_applies {
                    match &apply.resource {
                        MetalBoundValue::AccelerationStructure(native) => {
                            // SAFETY: the structure comes from the registry and
                            // outlives the encoder's use of it.
                            unsafe {
                                encoder.setAccelerationStructure_atBufferIndex(
                                    Some(&native.structure),
                                    apply.index,
                                )
                            };
                            // TLAS traversal indirectly references each BLAS.
                            for dependency in &native.dependencies {
                                encoder.useResource_usage(dependency, MTLResourceUsage::Read);
                            }
                        }
                        MetalBoundValue::Sampler(state) => {
                            // SAFETY: the sampler comes from the registry and
                            // outlives the encoder's use of it.
                            unsafe { encoder.setSamplerState_atIndex(Some(state), apply.index) }
                        }
                        MetalBoundValue::Texture(texture) => {
                            // SAFETY: the texture comes from the registry and
                            // outlives the encoder's use of it.
                            unsafe { encoder.setTexture_atIndex(Some(texture), apply.index) }
                        }
                        MetalBoundValue::Buffer { buffer, offset } => {
                            let resolved = binding_set
                                .dynamic_offset_index
                                .get(&MetalDynamicKey {
                                    stage: crate::rhi::ShaderStage::Compute,
                                    index: apply.index,
                                })
                                .map(|position| dynamic_offsets[*position] as usize)
                                .unwrap_or(*offset);
                            // SAFETY: the offset was range-checked at set
                            // registration and the buffer outlives the encoder.
                            unsafe {
                                encoder.setBuffer_offset_atIndex(
                                    Some(buffer),
                                    resolved,
                                    apply.index,
                                )
                            };
                        }
                    }
                }
                Ok(())
            }
            ComputeCommand::PushConstant { slot, data, .. } => {
                if data.is_empty() {
                    return Ok(());
                }
                // SAFETY: `data` is a live slice of `data.len()` bytes and Metal
                // copies it before returning.
                unsafe {
                    encoder.setBytes_length_atIndex(
                        core::ptr::NonNull::new(data.as_ptr() as *mut core::ffi::c_void)
                            .expect("a non-empty slice has a pointer"),
                        data.len(),
                        *slot as usize,
                    )
                };
                Ok(())
            }
            ComputeCommand::Dispatch {
                groups_x,
                groups_y,
                groups_z,
            } => {
                let grid = crate::rhi::ThreadgroupSize::new(*groups_x, *groups_y, *groups_z);
                grid.validate()?;
                let local = self.compute_threadgroup_size;
                encoder.dispatchThreadgroups_threadsPerThreadgroup(
                    MTLSize {
                        width: rhi_count(*groups_x)? as usize,
                        height: rhi_count(*groups_y)? as usize,
                        depth: rhi_count(*groups_z)? as usize,
                    },
                    MTLSize {
                        width: local.x as usize,
                        height: local.y as usize,
                        depth: local.z as usize,
                    },
                );
                encoder.memoryBarrierWithScope(
                    objc2_metal::MTLBarrierScope::Buffers | objc2_metal::MTLBarrierScope::Textures,
                );
                Ok(())
            }
            ComputeCommand::DispatchIndirect { buffer, offset } => {
                let native = {
                    let registries = lock(&self.registries);
                    let native = registries
                        .buffers
                        .get(&buffer.id())
                        .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                    rhi_byte_range(*offset, 12, native.length())?;
                    native.clone()
                };
                let local = self.compute_threadgroup_size;
                // SAFETY: the 12-byte argument block was validated inside the
                // buffer and the buffer outlives the encoder's use of it.
                unsafe {
                    encoder.dispatchThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerThreadgroup(
                        &native,
                        *offset,
                        MTLSize {
                            width: local.x as usize,
                            height: local.y as usize,
                            depth: local.z as usize,
                        },
                    )
                };
                encoder.memoryBarrierWithScope(
                    objc2_metal::MTLBarrierScope::Buffers | objc2_metal::MTLBarrierScope::Textures,
                );
                Ok(())
            }
        }
    }

    // MARK: Copy pass

    fn encode_copy_pass(&mut self, record: &CopyPassRecord) -> RhiResult<()> {
        let encoder = self
            .command_buffer
            .blitCommandEncoder()
            .ok_or(RhiError::OutOfMemory)?;
        let result = (|| -> RhiResult<()> {
            for command in &record.body {
                self.encode_copy_command(command, &encoder)?;
            }
            Ok(())
        })();
        encoder.endEncoding();
        result
    }

    fn encode_copy_command(
        &self,
        command: &CopyCommand,
        encoder: &ProtocolObject<dyn MTLBlitCommandEncoder>,
    ) -> RhiResult<()> {
        let registries = lock(&self.registries);
        match command {
            CopyCommand::CopyTexture {
                src,
                dst,
                width,
                height,
            } => {
                let source = registries
                    .textures
                    .get(&src.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown texture copy resource"))?;
                let destination = registries
                    .textures
                    .get(&dst.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown texture copy resource"))?;
                rhi_require(
                    src != dst
                        && source.textureType() == objc2_metal::MTLTextureType::Type2D
                        && destination.textureType() == objc2_metal::MTLTextureType::Type2D
                        && source.sampleCount() == 1
                        && destination.sampleCount() == 1,
                    "texture copy requires distinct single-sample 2D textures",
                )?;
                rhi_color_texture_copy_extent(
                    *width as usize,
                    *height as usize,
                    crate::rhi::TextureCopyExtent {
                        width: source.width(),
                        height: source.height(),
                        format: formats::color_format(source.pixelFormat()),
                    },
                    crate::rhi::TextureCopyExtent {
                        width: destination.width(),
                        height: destination.height(),
                        format: formats::color_format(destination.pixelFormat()),
                    },
                )?;
                // SAFETY: the extent was validated against both textures, which
                // outlive the encoder's use of them.
                unsafe {
                    encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                        source,
                        0,
                        0,
                        objc2_metal::MTLOrigin { x: 0, y: 0, z: 0 },
                        MTLSize { width: *width as usize, height: *height as usize, depth: 1 },
                        destination,
                        0,
                        0,
                        objc2_metal::MTLOrigin { x: 0, y: 0, z: 0 },
                    )
                };
                Ok(())
            }
            CopyCommand::CopyBuffer {
                src,
                src_offset,
                dst,
                dst_offset,
                size,
            } => {
                let source = registries
                    .buffers
                    .get(&src.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                let destination = registries
                    .buffers
                    .get(&dst.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                rhi_byte_range(*src_offset, *size, source.length())?;
                rhi_byte_range(*dst_offset, *size, destination.length())?;
                rhi_require(
                    !std::ptr::eq(
                        Retained::as_ptr(source) as *const u8,
                        Retained::as_ptr(destination) as *const u8,
                    ),
                    "copies within one buffer are not supported",
                )?;
                // SAFETY: both ranges were validated inside their buffers, and a
                // same-object copy was rejected above.
                unsafe {
                    encoder.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(
                        source,
                        *src_offset,
                        destination,
                        *dst_offset,
                        *size,
                    )
                };
                Ok(())
            }
            CopyCommand::CopyBufferToTexture(upload) => {
                let source = registries
                    .buffers
                    .get(&upload.buffer.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                let destination = registries
                    .textures
                    .get(&upload.texture.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                rhi_require(
                    destination.sampleCount() == 1,
                    "buffer texture copies require a single-sample texture",
                )?;
                let layers = if destination.textureType() == objc2_metal::MTLTextureType::TypeCube {
                    6
                } else {
                    destination.arrayLength()
                };
                let (extent_width, extent_height) = rhi_texture_subresource_extent(
                    upload.subresource,
                    destination.width(),
                    destination.height(),
                    destination.mipmapLevelCount(),
                    layers,
                )?;
                let bytes = rhi_texture_upload_bytes(
                    upload.region,
                    upload.bytes_per_row,
                    formats::color_format(destination.pixelFormat()),
                    extent_width,
                    extent_height,
                    source.length(),
                )?;
                rhi_byte_range(upload.offset, bytes, source.length())?;
                // SAFETY: the upload region, row pitch and byte range were all
                // validated against the texture extent and buffer length.
                unsafe {
                    encoder.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                        source,
                        upload.offset,
                        upload.bytes_per_row,
                        upload.bytes_per_row * upload.region.height as usize,
                        MTLSize {
                            width: upload.region.width as usize,
                            height: upload.region.height as usize,
                            depth: 1,
                        },
                        destination,
                        upload.subresource.layer,
                        upload.subresource.mip_level,
                        objc2_metal::MTLOrigin {
                            x: upload.region.origin[0] as usize,
                            y: upload.region.origin[1] as usize,
                            z: 0,
                        },
                    )
                };
                Ok(())
            }
            CopyCommand::CopyTextureToBuffer {
                texture,
                width,
                height,
                buffer,
                offset,
                bytes_per_row,
            } => {
                let source = registries
                    .textures
                    .get(&texture.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                let destination = registries
                    .buffers
                    .get(&buffer.id())
                    .ok_or_else(|| RhiError::invalid_argument("unknown buffer or texture"))?;
                rhi_require(
                    source.sampleCount() == 1,
                    "buffer texture copies require a single-sample texture",
                )?;
                let bytes = rhi_texture_transfer_bytes(
                    *width as usize,
                    *height as usize,
                    *bytes_per_row,
                    formats::color_format(source.pixelFormat()),
                    source.width(),
                    source.height(),
                    destination.length(),
                )?;
                rhi_byte_range(*offset, bytes, destination.length())?;
                // SAFETY: the copy extent and destination pitch were validated
                // against the texture extent and buffer length.
                unsafe {
                    encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                        source,
                        0,
                        0,
                        objc2_metal::MTLOrigin { x: 0, y: 0, z: 0 },
                        MTLSize { width: *width as usize, height: *height as usize, depth: 1 },
                        destination,
                        *offset,
                        *bytes_per_row,
                        *bytes_per_row * *height as usize,
                    )
                };
                Ok(())
            }
        }
    }
}

/// Describes a native texture for attachment validation.
fn describe_texture(
    texture: &Retained<ProtocolObject<dyn MTLTexture>>,
) -> RhiResult<RenderTextureInfo<objc2_metal::MTLPixelFormat>> {
    let format = formats::color_format(texture.pixelFormat());
    let is_depth = matches!(
        texture.pixelFormat(),
        objc2_metal::MTLPixelFormat::Depth32Float
            | objc2_metal::MTLPixelFormat::Depth32Float_Stencil8
    );
    // Metal reports render-target capability through its own usage bits, so the
    // classification comes from the native usage rather than the descriptor.
    let usage = if texture
        .usage()
        .contains(objc2_metal::MTLTextureUsage::RenderTarget)
    {
        if is_depth {
            TextureUsage::DEPTH_STENCIL_TARGET
        } else {
            TextureUsage::COLOR_TARGET
        }
    } else {
        TextureUsage::EMPTY
    };
    let mut info = RenderTextureInfo::new(
        [texture.width(), texture.height()],
        texture.sampleCount(),
        texture.pixelFormat(),
    );
    info.usage = usage;
    info.single_layer_2d = matches!(
        texture.textureType(),
        objc2_metal::MTLTextureType::Type2D | objc2_metal::MTLTextureType::Type2DMultisample
    );
    info.is_depth = is_depth;
    info.supports_color_resolve = texture.pixelFormat() != objc2_metal::MTLPixelFormat::R32Uint;
    let _ = format;
    Ok(info)
}

/// Whether a texture was allocated with memoryless (tile-only) storage.
fn is_memoryless(registries: &super::registries::MetalRegistries, handle: Texture) -> bool {
    registries
        .textures
        .get(&handle.id())
        .is_some_and(|texture| texture.storageMode() == MTLStorageMode::Memoryless)
}

/// Copies a registered binding set's resolved data out of the registry lock so
/// the encoder can replay it without holding the mutex across GPU calls.
fn shallow_clone_binding_set(set: &MetalBindingSet) -> MetalBindingSet {
    MetalBindingSet {
        entries: set
            .entries
            .iter()
            .map(|entry| MetalBoundEntry {
                slot: entry.slot,
                visibility: entry.visibility,
                resource: entry.resource.clone(),
            })
            .collect(),
        graphics_applies: set.graphics_applies.clone(),
        mesh_applies: set.mesh_applies.clone(),
        compute_applies: set.compute_applies.clone(),
        dynamic_offset_index: set.dynamic_offset_index.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_indirect_draw_stride_matches_the_native_argument_block() {
        assert_eq!(
            core::mem::size_of::<objc2_metal::MTLDrawPrimitivesIndirectArguments>(),
            16
        );
    }

    #[test]
    fn a_shallow_binding_set_copy_preserves_apply_tables() {
        let mut dynamic = std::collections::HashMap::new();
        dynamic.insert(3u32, 0usize);
        let set = MetalBindingSet::new(Vec::new(), &dynamic);
        let copy = shallow_clone_binding_set(&set);
        assert_eq!(copy.graphics_applies.len(), set.graphics_applies.len());
        assert_eq!(
            copy.dynamic_offset_index.len(),
            set.dynamic_offset_index.len()
        );
    }
}
