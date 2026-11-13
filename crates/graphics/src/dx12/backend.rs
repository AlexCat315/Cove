//! Translation of NativeRHI/DX12/{DX12Backend, Submission/DX12Submission}.swift:
//! the concrete `RhiBackend` and its per-submission command-list recording.
//!
//! Swift sent a flat `GRHI_DX12Command` stream to the C++ bridge, which decoded it
//! into command-list calls. The Rust port encodes the recorded commands straight
//! into an `ID3D12GraphicsCommandList`, so the intermediate opcode stream — and the
//! double decode it required — is gone while the command set is identical.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::rhi::{
    AccelerationStructure, AccelerationStructureDescriptor, AdapterCapabilities, BindingLayout,
    BindingLayoutDescriptor, BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer,
    BufferDescriptor, Capabilities, Completion, ComputePipeline, ComputePipelineDescriptor,
    CpuProfilePhase, DeviceConfig, FrameUploader, GpuTimestampProfile, GraphicsApi,
    GraphicsPipeline, GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor,
    PipelineLayout, PipelineLayoutDescriptor, PlannedSubmit, PresentationMode, RhiBackend,
    RhiCompilationWork, RhiCompiledObject, RhiError, RhiResult, Sampler, SamplerDescriptor,
    ShaderModule, ShaderModuleDescriptor, SubmissionCpuProfile, SurfaceDescriptor, Swapchain,
    SwapchainImage, Texture, TextureAllocation, TextureDescriptor, TextureSubresource,
    TextureUploadRegion, rhi_require, rhi_validate_texture_samples,
};

use super::device::Dx12DeviceHandle;
use super::resources::Dx12Registries;
use super::sys;
use crate::rhi::UploadRingChunk;

// SAFETY: every field is a COM pointer or an `Arc<Mutex<_>>`. Direct3D 12 objects
// are documented as usable from multiple threads, and the backend is handed to the
// frontend behind its own mutex, so two threads never touch this state
// concurrently. The impl exists because raw `*mut c_void` COM pointers do not
// implement `Send`, which leaves the auto-derived bound too weak for
// `RhiBackend: Send`.
unsafe impl Send for Dx12Backend {}

/// The DX12 backend.
pub struct Dx12Backend {
    device: Dx12DeviceHandle,
    registries: Arc<Mutex<Dx12Registries>>,
    uploaders: HashMap<usize, Arc<Mutex<Dx12FrameUploader>>>,
    gpu_timestamp_profile: Option<Arc<GpuTimestampProfile>>,
}

impl Dx12Backend {
    fn submit_impl(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Result<crate::rhi::TimedCompletion, Completion>,
    ) -> RhiResult<()> {
        let encoding_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
        // Execute planner releases, graphics, compute and copies in order on
        // one queue. Resource states cannot cross native queue types without
        // explicit handoffs and fence waits.

        // Recording may create the queue's allocator on first use.
        let mut registries = self
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let super::command_list::EncodedSubmission {
            list: command_list,
            staged,
            texture_states,
            #[cfg(target_os = "windows")]
            timestamps,
            ..
        } = super::command_list::record(
            &mut self.device,
            &mut registries,
            submit,
            completion.is_ok(),
        )?;
        // SAFETY: the list was closed by `record` and the queue is live.
        let fence_value = self.device.execute(command_list);
        // SAFETY: the list is no longer needed on the CPU once submitted.
        unsafe { sys::IUnknownVtbl::release(command_list) };
        let fence_value = match fence_value {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: the submission never reached the GPU, so its staging
                // buffers have no outstanding readers.
                unsafe {
                    for resource in staged {
                        sys::IUnknownVtbl::release(resource);
                    }
                }
                return Err(error);
            }
        };
        for (id, state) in texture_states {
            if let Some(record) = registries.textures.get_mut(&id) {
                record.state = state;
            }
        }
        drop(registries);
        if let Some(profile) = cpu_profile {
            profile.end(CpuProfilePhase::Encoding, encoding_start);
            let queue_start = profile.begin();
            profile.end(CpuProfilePhase::QueueSubmit, queue_start);
        }

        // D3D12 reports completion through the queue's fence. The waiter thread
        // blocks on it and then fires the frontend's callback, so the submit path
        // itself never waits.
        let waiter = super::device::FenceWait {
            fence: self.device.graphics.fence,
            value: fence_value,
        };
        // Raw COM pointers are not `Send`; carrying their addresses keeps the
        // release on the thread that observed the fence.
        let staged: Vec<u64> = staged.into_iter().map(|resource| resource as u64).collect();
        std::thread::spawn(move || {
            #[cfg(target_os = "windows")]
            let nanos = waiter
                .wait()
                .ok()
                .and_then(|_| timestamps.as_ref()?.nanos());
            #[cfg(target_os = "windows")]
            drop(timestamps);
            #[cfg(not(target_os = "windows"))]
            let nanos = {
                let _ = waiter.wait();
                None
            };
            // SAFETY: each address belongs to a staging buffer created for this
            // submission, and the GPU has retired the fence that referenced it.
            unsafe {
                for address in staged {
                    sys::IUnknownVtbl::release(address as sys::ComPtr);
                }
            }
            match completion {
                Ok(timed) => timed(nanos),
                Err(untimed) => untimed(),
            }
        });
        Ok(())
    }

    /// Creates the backend, or reports that DX12 needs Windows.
    pub fn make(config: DeviceConfig) -> RhiResult<Self> {
        Ok(Self {
            device: Dx12DeviceHandle::make(config)?,
            registries: Arc::new(Mutex::new(Dx12Registries::default())),
            uploaders: HashMap::new(),
            gpu_timestamp_profile: None,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Dx12Registries> {
        self.registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits for the graphics queue to finish, which is the documented idle path.
    pub fn wait_until_idle(&mut self) -> RhiResult<()> {
        // SAFETY: the queue's fence is live; the wait is the documented completion
        // primitive.
        let value = self.device.graphics.fence_value;
        super::device::wait_idle(&self.device.graphics, value)
    }
}

impl Drop for Dx12Backend {
    fn drop(&mut self) {
        // The device releases its swapchain back buffers in its own `Drop`, while
        // `Dx12Registries::Drop` releases every texture it owns. Back buffers
        // appear in both, so detach them here to avoid releasing the same COM
        // pointer twice at shutdown.
        let ids: Vec<u32> = self
            .device
            .swapchains
            .values()
            .flat_map(|chain| chain.texture_ids.iter().copied())
            .collect();
        if !ids.is_empty() {
            let mut registries = self.lock();
            for id in ids {
                registries.textures.remove(&id);
            }
        }
    }
}

/// Per-slot upload ring: a host-visible committed buffer plus the handle the
/// frontend binds.
#[derive(Clone)]
pub struct Dx12UploadChunk {
    pub handle: Buffer,
    pub capacity: usize,
    /// Mapping into the upload heap.
    pointer: *mut u8,
}

// SAFETY: the chunk is a handle plus a CPU mapping into an upload heap. The
// allocator only hands out non-overlapping suballocations for one frame slot at a
// time, and upload-heap memory is written only by the API thread.
unsafe impl Send for Dx12UploadChunk {}

impl UploadRingChunk for Dx12UploadChunk {
    fn capacity(&self) -> usize {
        self.capacity
    }

    unsafe fn cpu_pointer(&self) -> *mut u8 {
        self.pointer
    }
}

/// The frontend's exclusive handle onto one slot's upload ring.
pub struct Dx12FrameUploader {
    allocator: crate::rhi::ChunkUploadAllocator<Dx12UploadChunk>,
}

impl Dx12FrameUploader {
    pub fn new(factory: impl FnMut(usize) -> RhiResult<Dx12UploadChunk> + Send + 'static) -> Self {
        Self {
            allocator: crate::rhi::ChunkUploadAllocator::new(4 * 1024 * 1024, factory),
        }
    }
}

impl FrameUploader for Dx12FrameUploader {
    fn reset(&mut self) {
        self.allocator.reset();
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<crate::rhi::UploadLocation> {
        // A constant-buffer view must start on a 256-byte boundary, so the ring
        // never hands out a tighter alignment.
        let allocation = self.allocator.write(bytes, alignment.max(256))?;
        Ok(crate::rhi::UploadLocation::new(
            allocation.chunk().handle,
            allocation.offset(),
        ))
    }
}

impl RhiBackend for Dx12Backend {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::Dx12
    }

    fn device_name(&self) -> &str {
        &self.device.device_name
    }

    fn query_capabilities(&self) -> Capabilities {
        self.device.capabilities
    }

    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        self.device.adapter_capabilities
    }

    // MARK: Swapchain

    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        rhi_require(
            descriptor.kind == crate::rhi::NativeSurfaceKind::Win32Window,
            "DX12 requires an HWND surface",
        )?;
        self.wait_until_idle()?;
        let generation = self
            .device
            .swapchains
            .get(&handle.id())
            .map_or(1, |chain| chain.generation + 1);
        self.destroy_swapchain(handle);
        super::swapchain::create(
            &mut self.device,
            &self.registries,
            handle,
            &descriptor,
            generation,
        )
    }

    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        self.device
            .swapchains
            .get(&handle.id())
            .map(|chain| {
                if chain.vsync {
                    PresentationMode::Fifo
                } else {
                    PresentationMode::Immediate
                }
            })
            .ok_or_else(|| RhiError::invalid_argument("query a live swapchain"))
    }

    fn destroy_swapchain(&mut self, handle: Swapchain) {
        let texture_ids = super::swapchain::destroy(&mut self.device, handle);
        if !texture_ids.is_empty() {
            // Back buffers are owned by the swapchain, which released them above;
            // only the registry entries are removed here.
            let mut registries = self.lock();
            for id in texture_ids {
                registries.textures.remove(&id);
            }
        }
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        super::swapchain::acquire(&mut self.device, handle)
    }

    fn present(&mut self, image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        #[cfg(target_os = "windows")]
        {
            let mut registries = self
                .registries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let record = registries
                .textures
                .get_mut(&image.texture.id())
                .ok_or_else(|| RhiError::present_failed("unknown DX12 swapchain image"))?;
            let present = sys::D3D12_RESOURCE_STATE_PRESENT;
            if record.state != present {
                let list = super::command_list::open_graphics_list(&mut self.device)?;
                // SAFETY: the acquired image and the direct command list are live.
                unsafe {
                    super::resources::transition(list, record.resource, record.state, present)
                };
                super::command_list::submit_graphics_list(&mut self.device, list)?;
                record.state = present;
            }
        }
        super::swapchain::present(&mut self.device, image)?;
        let waiter = super::device::FenceWait {
            fence: self.device.graphics.fence,
            value: self.device.graphics.fence_value,
        };
        std::thread::spawn(move || {
            let _ = waiter.wait();
            completion();
        });
        Ok(())
    }

    // MARK: Resource creation

    fn create_buffer(&mut self, handle: Buffer, descriptor: BufferDescriptor) -> RhiResult<()> {
        rhi_require(descriptor.size > 0, "buffer size must be positive")?;
        let mut registries = self.lock();
        super::buffer::create(&self.device, &mut registries, handle, &descriptor)
    }

    fn create_texture(&mut self, handle: Texture, descriptor: TextureDescriptor) -> RhiResult<()> {
        rhi_validate_texture_samples(&descriptor)?;
        // RTV/DSV allocation advances a device counter, so lock the registry
        // field directly: `self.lock()` borrows all of `self` immutably.
        let mut registries = self
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        super::texture::create(&mut self.device, &mut registries, handle, &descriptor)
    }

    fn create_sampler(&mut self, handle: Sampler, descriptor: SamplerDescriptor) -> RhiResult<()> {
        let mut registries = self.lock();
        super::texture::create_sampler(&self.device, &mut registries, handle, &descriptor)
    }

    fn create_shader_module(
        &mut self,
        handle: ShaderModule,
        descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        rhi_require(
            descriptor.format == crate::rhi::ShaderFormat::Dxil,
            "DX12 requires DXIL",
        )?;
        rhi_require(
            descriptor.specialization_constants.is_empty(),
            "DXIL specialization requires an offline shader variant",
        )?;
        let mut registries = self.lock();
        registries.shaders.insert(
            handle.id(),
            super::resources::Dx12ShaderRecord {
                bytecode: descriptor.code,
                stage: descriptor.stage,
            },
        );
        Ok(())
    }

    fn create_acceleration_structure(
        &mut self,
        handle: AccelerationStructure,
        descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<()> {
        #[cfg(target_os = "windows")]
        {
            let mut registries = self.lock();
            let record =
                super::acceleration::Acceleration::new(&self.device, &registries, descriptor)?;
            registries.accelerations.insert(handle.id(), record);
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (handle, descriptor);
            Err(RhiError::unsupported_backend(
                "DX12 is only available on Windows",
            ))
        }
    }

    fn texture_allocation(&self, texture: Texture) -> Option<TextureAllocation> {
        let registries = self.lock();
        let record = registries.textures.get(&texture.id())?;
        let bytes = super::texture::allocation_bytes(record);
        Some(TextureAllocation::new(bytes as i64, false))
    }

    // MARK: Pipelines

    fn create_graphics_pipeline(
        &mut self,
        handle: GraphicsPipeline,
        descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        let mut registries = self.lock();
        super::pipeline::create_graphics(&self.device, &mut registries, handle, &descriptor)
    }

    fn create_mesh_pipeline(
        &mut self,
        handle: MeshPipeline,
        descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<()> {
        rhi_require(
            self.device.adapter_capabilities.mesh_shading,
            "adapter has no mesh shader support",
        )?;
        let mut registries = self.lock();
        super::pipeline::create_mesh(&self.device, &mut registries, handle, &descriptor)
    }

    fn create_compute_pipeline(
        &mut self,
        handle: ComputePipeline,
        descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        let mut registries = self.lock();
        super::pipeline::create_compute(&self.device, &mut registries, handle, &descriptor)
    }

    // MARK: Binding sets

    fn register_binding_layout(
        &mut self,
        handle: BindingLayout,
        descriptor: BindingLayoutDescriptor,
    ) -> RhiResult<()> {
        let mut registries = self.lock();
        super::binding::register_layout(&mut registries, handle, &descriptor)
    }

    fn register_pipeline_layout(
        &mut self,
        handle: PipelineLayout,
        descriptor: PipelineLayoutDescriptor,
    ) -> RhiResult<()> {
        let mut registries = self.lock();
        super::binding::register_pipeline_layout(&self.device, &mut registries, handle, &descriptor)
    }

    fn register_binding_set(
        &mut self,
        handle: BindingSet,
        layout: BindingLayout,
        layout_entries: Vec<BindingLayoutEntry>,
        set_entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()> {
        let mut registries = self.lock();
        super::binding::register_set(
            &self.device,
            &mut registries,
            handle,
            layout,
            &layout_entries,
            &set_entries,
        )
    }

    fn unregister_binding_set(&mut self, handle: BindingSet) {
        super::binding::unregister_set(&mut self.lock(), handle);
    }

    // MARK: Immediate data transfer

    fn upload_buffer_data(&mut self, buffer: Buffer, offset: usize, data: &[u8]) -> RhiResult<()> {
        let registries = self
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        super::buffer::upload(&registries, &mut self.device, buffer, offset, data)
    }

    fn read_buffer_data(
        &mut self,
        buffer: Buffer,
        offset: usize,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        self.wait_until_idle()?;
        let registries = self
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        super::buffer::read(&registries, &mut self.device, buffer, offset, destination)
    }

    fn upload_texture_data(
        &mut self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()> {
        // Snapshot the resource and its state before opening a direct list.
        let record = {
            let registries = self.lock();
            registries
                .textures
                .get(&texture.id())
                .map(|record| TextureSnapshot {
                    resource: record.resource,
                    state: record.state,
                    format: record.format,
                    width: record.width,
                    height: record.height,
                    mip_levels: record.mip_levels,
                    layers: record.layers,
                    sample_count: record.sample_count,
                })
        };
        let record = record.ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
        super::texture::upload_snapshot(
            &mut self.device,
            &record,
            data,
            region,
            bytes_per_row,
            subresource,
        )?;
        if let Some(record) = self.lock().textures.get_mut(&texture.id()) {
            record.state = sys::D3D12_RESOURCE_STATE_COPY_DEST;
        }
        Ok(())
    }

    fn read_texture_data(
        &mut self,
        texture: Texture,
        width: usize,
        height: usize,
        bytes_per_row: usize,
        subresource: TextureSubresource,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        self.wait_until_idle()?;
        let record = {
            let registries = self.lock();
            registries
                .textures
                .get(&texture.id())
                .map(|record| TextureSnapshot {
                    resource: record.resource,
                    state: record.state,
                    format: record.format,
                    width: record.width,
                    height: record.height,
                    mip_levels: record.mip_levels,
                    layers: record.layers,
                    sample_count: record.sample_count,
                })
        };
        let record = record.ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
        super::texture::read_snapshot(
            &mut self.device,
            &record,
            width,
            height,
            bytes_per_row,
            subresource,
            destination,
        )
    }

    // MARK: Submission

    fn make_frame_uploader(&mut self, slot: usize) -> RhiResult<Box<dyn FrameUploader>> {
        let uploader = if let Some(existing) = self.uploaders.get(&slot) {
            let mut guard = existing
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            crate::rhi::FrameUploader::reset(&mut *guard);
            Arc::clone(existing)
        } else {
            let factory = UploadFactory {
                registries: Arc::clone(&self.registries),
                // The device is needed only for the resource creation, which the
                // factory reaches through the handle it captured.
                device: self.device.device,
            };
            let created = Arc::new(Mutex::new(Dx12FrameUploader::new(move |capacity| {
                factory.make_chunk(capacity)
            })));
            self.uploaders.insert(slot, Arc::clone(&created));
            created
        };
        Ok(Box::new(SharedUploader { uploader }))
    }

    fn submit(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        self.submit_impl(submit, cpu_profile, Err(completion))
    }

    fn submit_timed(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: crate::rhi::TimedCompletion,
    ) -> RhiResult<()> {
        self.submit_impl(submit, cpu_profile, Ok(completion))
    }

    // MARK: Deferred destruction

    fn destroy_buffer(&mut self, handle: Buffer) {
        let mut registries = self.lock();
        if let Some(record) = registries.buffers.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(record.resource) };
        }
    }

    fn destroy_texture(&mut self, handle: Texture) {
        let record = self.lock().textures.remove(&handle.id());
        if let Some(record) = record {
            if let Some(index) = record.attachment_index {
                self.device.release_rtv_index(index);
            }
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(record.resource) };
        }
    }

    fn destroy_sampler(&mut self, handle: Sampler) {
        // A sampler is a descriptor-heap address, not a COM object, so dropping the
        // record is the whole release.
        self.lock().samplers.remove(&handle.id());
    }

    fn destroy_shader_module(&mut self, handle: ShaderModule) {
        self.lock().shaders.remove(&handle.id());
    }

    fn destroy_graphics_pipeline(&mut self, handle: GraphicsPipeline) {
        let mut registries = self.lock();
        if let Some(record) = registries.graphics_pipelines.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(record.pipeline) };
        }
    }

    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline) {
        let mut registries = self.lock();
        if let Some(record) = registries.compute_pipelines.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(record.pipeline) };
        }
    }

    fn destroy_mesh_pipeline(&mut self, handle: MeshPipeline) {
        let mut registries = self.lock();
        if let Some(record) = registries.mesh_pipelines.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(record.pipeline) };
        }
    }

    fn destroy_acceleration_structure(&mut self, handle: AccelerationStructure) {
        #[cfg(target_os = "windows")]
        self.lock().accelerations.remove(&handle.id());
        #[cfg(not(target_os = "windows"))]
        let _ = handle;
    }

    // MARK: Synchronization

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        Dx12Backend::wait_until_idle(self)
    }

    // MARK: Optional capabilities

    fn supports_background_compilation(&self) -> bool {
        // Each creation call has its own mutex-protected registry, so the same
        // snapshot/compile/publish split is available.
        true
    }

    fn supports_gpu_timestamps(&self) -> bool {
        false
    }

    fn gpu_timestamp_profile(&self) -> Option<Arc<GpuTimestampProfile>> {
        self.gpu_timestamp_profile.clone()
    }

    fn set_gpu_timestamp_profile(&mut self, profile: Option<Arc<GpuTimestampProfile>>) {
        self.gpu_timestamp_profile = profile;
    }
}

impl Dx12Backend {
    /// Runs one background compilation request.
    pub fn prepare_compilation(
        &self,
        request: crate::rhi::RhiCompilationRequest,
    ) -> RhiResult<RhiCompilationWork> {
        use crate::rhi::RhiCompilationRequest;
        // D3D12 creation is synchronous on its own registry lock, so the work object
        // performs the creation and reports failure through the same value the
        // other backends use.
        match request {
            RhiCompilationRequest::Shader(handle, descriptor) => {
                let mut registries = self.lock();
                registries.shaders.insert(
                    handle.id(),
                    super::resources::Dx12ShaderRecord {
                        bytecode: descriptor.code,
                        stage: descriptor.stage,
                    },
                );
                Ok(RhiCompilationWork::new(|| {
                    Ok(RhiCompiledObject::new(|| {}, || {}))
                }))
            }
            RhiCompilationRequest::Graphics(handle, descriptor) => {
                let mut registries = self.lock();
                super::pipeline::create_graphics(
                    &self.device,
                    &mut registries,
                    handle,
                    &descriptor,
                )?;
                Ok(RhiCompilationWork::new(|| {
                    Ok(RhiCompiledObject::new(|| {}, || {}))
                }))
            }
            RhiCompilationRequest::Compute(handle, descriptor) => {
                let mut registries = self.lock();
                super::pipeline::create_compute(
                    &self.device,
                    &mut registries,
                    handle,
                    &descriptor,
                )?;
                Ok(RhiCompilationWork::new(|| {
                    Ok(RhiCompiledObject::new(|| {}, || {}))
                }))
            }
            RhiCompilationRequest::Mesh(handle, descriptor) => {
                let mut registries = self.lock();
                super::pipeline::create_mesh(&self.device, &mut registries, handle, &descriptor)?;
                Ok(RhiCompilationWork::new(|| {
                    Ok(RhiCompiledObject::new(|| {}, || {}))
                }))
            }
        }
    }
}

/// The immutable parts of a texture record a transfer needs, copied out of the
/// registry so the device can be borrowed mutably for its copy queue.
pub struct TextureSnapshot {
    pub resource: sys::ComPtr,
    pub state: u32,
    pub format: crate::rhi::TextureFormat,
    pub width: usize,
    pub height: usize,
    pub mip_levels: usize,
    pub layers: usize,
    pub sample_count: usize,
}

/// Factory state for one slot's upload ring.
struct UploadFactory {
    registries: Arc<Mutex<Dx12Registries>>,
    device: sys::ComPtr,
}

// SAFETY: the factory holds a COM pointer and an `Arc<Mutex<_>>`. D3D12 resources
// are usable from any thread once created, and the chunk factory only runs while
// the backend holds the registry lock.
unsafe impl Send for UploadFactory {}
// SAFETY: the only shared state is the registry mutex; the device pointer is used
// solely to create resources, which D3D12 documents as thread-safe.
unsafe impl Sync for UploadFactory {}

impl UploadFactory {
    fn make_chunk(&self, capacity: usize) -> RhiResult<Dx12UploadChunk> {
        let mut registries = self
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = registries
            .next_upload_buffer_id
            .max(super::device::INTERNAL_ID_BASE);
        let next = id.checked_add(1).ok_or(RhiError::OutOfMemory)?;
        let record = super::buffer::create_upload_heap(self.device, capacity.max(1))?;
        let pointer = record.mapped;
        registries.next_upload_buffer_id = next;
        registries.buffers.insert(id, record);
        Ok(Dx12UploadChunk {
            handle: Buffer::new(id),
            capacity,
            pointer,
        })
    }
}

/// The frontend's exclusive `FrameUploader` handle onto one slot's shared ring.
struct SharedUploader {
    uploader: Arc<Mutex<Dx12FrameUploader>>,
}

impl FrameUploader for SharedUploader {
    fn reset(&mut self) {
        FrameUploader::reset(
            &mut *self
                .uploader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<crate::rhi::UploadLocation> {
        FrameUploader::write(
            &mut *self
                .uploader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            bytes,
            alignment,
        )
    }
}

/// Factory used by the backend registry.
pub fn make_backend(config: &DeviceConfig) -> RhiResult<Box<dyn RhiBackend>> {
    Ok(Box::new(Dx12Backend::make(config.clone())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let result = Dx12Backend::make(DeviceConfig::default());
            assert!(matches!(result, Err(RhiError::UnsupportedBackend(_))));
        }
    }

    #[test]
    fn the_backend_reports_directx() {
        assert_eq!(GraphicsApi::Dx12.display_name(), "DirectX 12");
    }
}
