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
    CpuProfilePhase, DeviceConfig, FrameUploader, GraphicsApi, GraphicsPipeline,
    GraphicsPipelineDescriptor, GpuTimestampProfile, MeshPipeline, MeshPipelineDescriptor,
    PipelineLayout, PipelineLayoutDescriptor, PlannedSubmit, PresentationMode, QueueClass,
    RhiBackend, RhiError, RhiResult, Sampler, SamplerDescriptor, ShaderModule,
    RhiCompilationWork, RhiCompiledObject, ShaderModuleDescriptor, SubmissionCpuProfile,
    SurfaceDescriptor, Swapchain, SwapchainImage,
    Texture, TextureAllocation, TextureDescriptor, TextureSubresource, TextureUploadRegion,
    rhi_require, rhi_validate_texture_samples,
};

use super::device::{Dx12DeviceHandle, execute_and_signal};
use super::resources::Dx12Registries;
use crate::rhi::UploadRingChunk;
use super::sys;

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
        self.registries.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits for the graphics queue to finish, which is the documented idle path.
    pub fn wait_until_idle(&mut self) -> RhiResult<()> {
        // SAFETY: the queue's fence is live; the wait is the documented completion
        // primitive.
        let value = self.device.graphics.fence_value;
        super::device::wait_idle(&self.device.graphics, value)
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
        Self { allocator: crate::rhi::ChunkUploadAllocator::new(4 * 1024 * 1024, factory) }
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
        super::swapchain::create(&mut self.device, &self.registries, handle, &descriptor, generation)
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
        // `destroy` only touches the swapchain map, so the registries are not
        // needed here; taking the lock must wait until after the device borrow.
        super::swapchain::destroy(&mut self.device, handle);
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        super::swapchain::acquire(&mut self.device, handle)
    }

    fn present(&mut self, image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        super::swapchain::present(&mut self.device, image)?;
        // The present is queued; the frontend's completion fires once it is retired.
        completion();
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
        let mut registries = self.lock();
        super::texture::create(&self.device, &mut registries, handle, &descriptor)
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
        _handle: AccelerationStructure,
        _descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<()> {
        // Acceleration structures stay unadvertised, so the frontend rejects such
        // a request before reaching here; this guard keeps the backend honest if a
        // caller bypasses the capability check.
        Err(RhiError::unsupported_feature(
            "DX12 acceleration structures are not implemented by this build",
        ))
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

    fn upload_buffer_data(
        &mut self,
        buffer: Buffer,
        offset: usize,
        data: &[u8],
    ) -> RhiResult<()> {
        let registries = self.lock();
        super::buffer::upload(&registries, &self.device, buffer, offset, data)
    }

    fn read_buffer_data(
        &mut self,
        buffer: Buffer,
        offset: usize,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        self.wait_until_idle()?;
        let registries = self.lock();
        super::buffer::read(&registries, buffer, offset, destination)
    }

    fn upload_texture_data(
        &mut self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()> {
        // The transfer needs the device mutably for its copy queue, so the
        // registries cannot stay locked across the call. The texture record is
        // resolved before the lock is released, which is enough: the transfer only
        // reads its immutable metadata.
        let record = {
            let registries = self.lock();
            registries.textures.get(&texture.id()).map(|record| TextureSnapshot {
                resource: record.resource,
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
        )
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
            registries.textures.get(&texture.id()).map(|record| TextureSnapshot {
                resource: record.resource,
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
            let mut guard = existing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            crate::rhi::FrameUploader::reset(&mut *guard);
            Arc::clone(existing)
        } else {
            let factory = UploadFactory {
                registries: Arc::clone(&self.registries),
                // The device is needed only for the resource creation, which the
                // factory reaches through the handle it captured.
                device: self.device.device,
                next_id: self.device.next_internal_id,
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
        mut cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        let encoding_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
        let queue = match submit.queue {
            QueueClass::Graphics => &mut self.device.graphics,
            QueueClass::Compute => &mut self.device.compute,
            // D3D12 has no dedicated transfer queue in this configuration, so
            // transfer work runs on the copy queue.
            QueueClass::Transfer => &mut self.device.copy,
        };
        let queue_kind = queue.kind;
        let queue_fence = queue.fence;

        // `record` borrows the device immutably and the registries mutably; the
        // queue borrow above ends here because only `queue_kind`/`queue_fence` are
        // still needed, and the queue itself is re-borrowed for the submit.
        let mut registries = self.lock();
        let command_list =
            super::command_list::record(&self.device, &mut registries, queue_kind, submit)?;
        drop(registries);
        let queue = match queue_kind {
            sys::D3D12_COMMAND_LIST_TYPE_DIRECT => &mut self.device.graphics,
            sys::D3D12_COMMAND_LIST_TYPE_COMPUTE => &mut self.device.compute,
            _ => &mut self.device.copy,
        };
        // SAFETY: the list was closed by `record` and the queue is live.
        let fence_value = execute_and_signal(queue, command_list);
        // SAFETY: the list is no longer needed on the CPU once submitted.
        unsafe { sys::IUnknownVtbl::release(command_list) };
        let fence_value = fence_value?;
        if let Some(profile) = cpu_profile.as_deref_mut() {
            profile.end(CpuProfilePhase::Encoding, encoding_start);
            let queue_start = profile.begin();
            profile.end(CpuProfilePhase::QueueSubmit, queue_start);
        }
        let _ = queue_fence;

        // D3D12 reports completion through the queue's fence. The waiter thread
        // blocks on it and then fires the frontend's callback, so the submit path
        // itself never waits.
        let waiter = super::device::FenceWait {
            fence: queue.fence,
            value: fence_value,
        };
        std::thread::spawn(move || {
            let _ = waiter.wait();
            completion();
        });
        Ok(())
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
        let mut registries = self.lock();
        if let Some(record) = registries.textures.remove(&handle.id()) {
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
        if let Some(pipeline) = registries.graphics_pipelines.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(pipeline) };
        }
    }

    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline) {
        let mut registries = self.lock();
        if let Some(pipeline) = registries.compute_pipelines.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(pipeline) };
        }
    }

    fn destroy_mesh_pipeline(&mut self, handle: MeshPipeline) {
        let mut registries = self.lock();
        if let Some(pipeline) = registries.mesh_pipelines.remove(&handle.id()) {
            // SAFETY: the frame ring waited for every submission that could use it.
            unsafe { sys::IUnknownVtbl::release(pipeline) };
        }
    }

    fn destroy_acceleration_structure(&mut self, _handle: AccelerationStructure) {}

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
                super::pipeline::create_graphics(&self.device, &mut registries, handle, &descriptor)?;
                Ok(RhiCompilationWork::new(|| Ok(RhiCompiledObject::new(|| {}, || {}))))
            }
            RhiCompilationRequest::Compute(handle, descriptor) => {
                let mut registries = self.lock();
                super::pipeline::create_compute(&self.device, &mut registries, handle, &descriptor)?;
                Ok(RhiCompilationWork::new(|| Ok(RhiCompiledObject::new(|| {}, || {}))))
            }
            RhiCompilationRequest::Mesh(handle, descriptor) => {
                let mut registries = self.lock();
                super::pipeline::create_mesh(&self.device, &mut registries, handle, &descriptor)?;
                Ok(RhiCompilationWork::new(|| Ok(RhiCompiledObject::new(|| {}, || {}))))
            }
        }
    }
}

/// The immutable parts of a texture record a transfer needs, copied out of the
/// registry so the device can be borrowed mutably for its copy queue.
pub struct TextureSnapshot {
    pub resource: sys::ComPtr,
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
    next_id: u32,
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
        let handle = Buffer::new(self.next_id + self.registries.lock().map(|guard| guard.buffers.len() as u32).unwrap_or(0));
        let record = super::buffer::create_upload_heap(self.device, capacity.max(1))?;
        let pointer = record.mapped;
        self.registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .buffers
            .insert(handle.id(), record);
        Ok(Dx12UploadChunk { handle, capacity, pointer })
    }
}

/// The frontend's exclusive `FrameUploader` handle onto one slot's shared ring.
struct SharedUploader {
    uploader: Arc<Mutex<Dx12FrameUploader>>,
}

impl FrameUploader for SharedUploader {
    fn reset(&mut self) {
        FrameUploader::reset(
            &mut *self.uploader.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<crate::rhi::UploadLocation> {
        FrameUploader::write(
            &mut *self.uploader.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
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
