//! Translation of NativeRHI/Device/Device.swift: the public frontend device.
//!
//! The device owns:
//!   * monotonic handle allocation ([`IdentifierPool`]),
//!   * binding-layout / binding-set / pipeline-layout caches ([`DeviceCaches`]),
//!   * the frames-in-flight ring plus deferred destruction ([`FrameRing`]),
//!   * the [`SubmissionPlanner`] that turns recorded commands into a
//!     [`SubmitPlan`],
//!
//! and delegates all concrete GPU work to the selected [`RhiBackend`].
//!
//! Swift serializes every mutating operation on an `NSLock`; the Rust port keeps
//! the same contract with a `Mutex` over the frontend state, and a second
//! `Mutex` that serializes whole frame sessions.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cove_logging::{Level, Location, LogRuntime, LogValue};

use super::{
    AccelerationStructure, AccelerationStructureBuild, AccelerationStructureDescriptor,
    AdapterCapabilities, BindingLayout, BindingLayoutDescriptor, BindingLayoutEntry,
    BindingResource, BindingSet, BindingSetDescriptor, Buffer, BufferDescriptor, CacheStats,
    Capabilities, CommandBuffer, ComputePipeline, ComputePipelineDescriptor, ComputePipelineKey,
    DeviceCaches, DeviceConfig, FrameRing, FrameUploader, GpuTimestampProfile, GraphicsPipeline,
    GraphicsPipelineDescriptor, GraphicsPipelineKey, MeshPipeline, MeshPipelineDescriptor,
    MeshPipelineKey, PipelineInterfaces, PipelineKind, PipelineLayout, PipelineLayoutDescriptor,
    PipelineStatistics, PipelineStore, PipelineUse, PresentationMode, QueueClass,
    ResourcePreparationContext, ResourcePreparationProgress, RhiBackend, RhiError, RhiResult,
    Sampler, SamplerDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderModuleKey,
    SubmissionCpuProfile, SubmissionPlanner, SubmitDescriptor, SurfaceDescriptor, Swapchain,
    SwapchainImage, Texture, TextureAllocation, TextureDescriptor, TextureSubresource,
    TextureUploadRegion, TimelineSemaphore, UploadLocation, rhi_require, rhi_validate_sample_count,
    rhi_validate_texture_samples,
};

/// Monotonic handle allocator. A single counter across all resource kinds
/// guarantees no two live handles share an ID (ID 0 is reserved/invalid).
#[derive(Debug)]
pub struct IdentifierPool {
    next_id: AtomicU32,
}
impl Default for IdentifierPool {
    fn default() -> Self {
        Self::new()
    }
}
impl IdentifierPool {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
        }
    }

    pub fn next(&self) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        assert!(id != 0 && id != u32::MAX, "RHI handle counter overflowed");
        id
    }
}

struct WindowSwapchain {
    descriptor: SurfaceDescriptor,
    generation: Option<u64>,
    acquired: Option<SwapchainImage>,
    acquisition_frame: Option<u64>,
    textures: HashSet<Texture>,
}

struct DeviceState {
    planner: SubmissionPlanner,
    caches: DeviceCaches,
    interfaces: PipelineInterfaces,
    pipeline_store: PipelineStore,
    acceleration_build_inputs: HashMap<u32, Vec<super::ResourceRef>>,
    current_slot: Option<usize>,
    current_uploader: Option<Box<dyn FrameUploader>>,
    submission_fault: Option<RhiError>,
    swapchains: HashMap<u32, WindowSwapchain>,
    frame_sequence: u64,
}

pub struct Device {
    /// Stable identity used by resource-preparation ownership bookkeeping.
    id: u64,
    config: DeviceConfig,
    capabilities: Capabilities,
    adapter_capabilities: AdapterCapabilities,
    backend_api: super::GraphicsApi,
    /// The backend lives behind its own lock so deferred destruction callbacks
    /// (which run from GPU completion threads) can reach it without holding the
    /// frontend state lock.
    backend: Arc<Mutex<Box<dyn RhiBackend>>>,
    frame_ring: Arc<FrameRing>,
    identifiers: Arc<IdentifierPool>,
    state: Arc<Mutex<DeviceState>>,
    session: Mutex<()>,
    logger: Mutex<Option<LogRuntime>>,
    retirements: Arc<Mutex<HashMap<u64, super::Completion>>>,
    ready_retirements: Arc<Mutex<Vec<u64>>>,
    next_retirement: Arc<std::sync::atomic::AtomicU64>,
}
impl Device {
    fn lock(&self) -> MutexGuard<'_, DeviceState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn backend(&self) -> MutexGuard<'_, Box<dyn RhiBackend>> {
        self.backend
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn new(backend: Box<dyn RhiBackend>, config: DeviceConfig) -> RhiResult<Self> {
        let capabilities = backend.query_capabilities();
        let adapter_capabilities = backend.query_adapter_capabilities();
        let backend_api = backend.api();
        let frames_in_flight = config.frames_in_flight.max(1);
        Ok(Self {
            id: super::next_device_id(),
            config,
            capabilities,
            adapter_capabilities,
            backend_api,
            backend: Arc::new(Mutex::new(backend)),
            frame_ring: Arc::new(FrameRing::new(frames_in_flight)),
            identifiers: Arc::new(IdentifierPool::new()),
            state: Arc::new(Mutex::new(DeviceState {
                planner: SubmissionPlanner::new(),
                caches: DeviceCaches::new(),
                interfaces: PipelineInterfaces::new(),
                pipeline_store: PipelineStore::new(),
                acceleration_build_inputs: HashMap::new(),
                current_slot: None,
                current_uploader: None,
                submission_fault: None,
                swapchains: HashMap::new(),
                frame_sequence: 0,
            })),
            session: Mutex::new(()),
            logger: Mutex::new(None),
            retirements: Arc::new(Mutex::new(HashMap::new())),
            ready_retirements: Arc::new(Mutex::new(Vec::new())),
            next_retirement: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        })
    }

    /// Creates a device, trying the configured backends in order and using the
    /// first that constructs successfully.
    pub fn make(config: DeviceConfig) -> RhiResult<Self> {
        config.validate()?;
        let mut last_error: Option<RhiError> = None;
        for api in &config.preferred_backends {
            if !super::is_compiled_in(*api) {
                last_error = Some(RhiError::unsupported_backend(format!(
                    "{} is not built for this platform",
                    api.display_name()
                )));
                continue;
            }
            match super::make_backend(*api, &config) {
                Ok(backend) => {
                    let device = Self::new(backend, config.clone())?;
                    device.log(
                        Level::Info,
                        "Graphics device created",
                        [
                            ("backend", api.display_name().to_owned()),
                            ("adapter", device.device_name().to_owned()),
                            ("frames_in_flight", config.frames_in_flight.to_string()),
                        ],
                    );
                    return Ok(device);
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
        }
        Err(last_error
            .unwrap_or_else(|| RhiError::unsupported_backend("no usable graphics backend")))
    }

    pub fn identity(&self) -> u64 {
        self.id
    }

    pub fn backend_api(&self) -> super::GraphicsApi {
        self.backend_api
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn adapter_capabilities(&self) -> AdapterCapabilities {
        self.adapter_capabilities
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    /// Human-readable physical device / adapter name.
    pub fn device_name(&self) -> String {
        self.backend().device_name().to_owned()
    }

    /// Installs the logger used for pipeline-creation diagnostics.
    pub fn set_logger(&self, logger: Option<LogRuntime>) {
        *self
            .logger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = logger;
    }

    // MARK: Surface

    pub fn make_swapchain(&self, descriptor: SurfaceDescriptor) -> RhiResult<Swapchain> {
        let mut state = self.lock();
        rhi_require(
            state.current_slot.is_none(),
            "create a swapchain outside a frame",
        )?;
        Self::validate_surface(&descriptor)?;
        Self::validate_unique_surface(&state, &descriptor, None)?;
        let handle = Swapchain::new(self.identifiers.next());
        self.backend().configure_swapchain(handle, descriptor)?;
        state.swapchains.insert(
            handle.id(),
            WindowSwapchain {
                descriptor,
                generation: None,
                acquired: None,
                acquisition_frame: None,
                textures: HashSet::new(),
            },
        );
        Ok(handle)
    }

    pub fn configure_swapchain(
        &self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        let mut state = self.lock();
        let Some(previous) = state.swapchains.get(&handle.id()) else {
            return Err(RhiError::invalid_argument(
                "resize a live swapchain outside a frame",
            ));
        };
        rhi_require(
            state.current_slot.is_none(),
            "resize a live swapchain outside a frame",
        )?;
        let stale: Vec<Texture> = previous.textures.iter().copied().collect();
        Self::validate_surface(&descriptor)?;
        Self::validate_unique_surface(&state, &descriptor, Some(handle))?;
        self.wait_for_frame_retirements()?;
        self.backend().wait_until_idle()?;
        self.backend().configure_swapchain(handle, descriptor)?;
        for texture in stale {
            state
                .planner
                .forget_resource(super::ResourceRef::texture(texture.id()));
        }
        state.swapchains.insert(
            handle.id(),
            WindowSwapchain {
                descriptor,
                generation: None,
                acquired: None,
                acquisition_frame: None,
                textures: HashSet::new(),
            },
        );
        Ok(())
    }

    pub fn destroy_swapchain(&self, handle: Swapchain) {
        let mut state = self.lock();
        self.destroy_swapchain_locked(&mut state, handle);
    }

    /// Queries the applied native mode, including a synchronized fallback when
    /// disabling VSync is unsupported. Refreshes after native swapchain
    /// recreation.
    pub fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        let state = self.lock();
        rhi_require(
            state.swapchains.contains_key(&handle.id()),
            "query a live swapchain",
        )?;
        self.backend().presentation_mode(handle)
    }

    fn destroy_swapchain_locked(&self, state: &mut DeviceState, handle: Swapchain) {
        let Some(previous) = state.swapchains.remove(&handle.id()) else {
            return;
        };
        for texture in &previous.textures {
            state
                .planner
                .forget_resource(super::ResourceRef::texture(texture.id()));
        }
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_swapchain(handle);
        }));
    }

    /// The owning resource calls this before its platform window is destroyed.
    /// Native error reporting must not leave an already-idle window registered.
    pub fn close_swapchain(&self, handle: Swapchain) -> RhiResult<()> {
        let mut state = self.lock();
        if !state.swapchains.contains_key(&handle.id()) {
            return Ok(());
        }
        rhi_require(
            state.current_slot.is_none(),
            "close a swapchain outside a frame",
        )?;
        self.wait_for_frame_retirements()?;
        self.backend().wait_until_idle()?;
        self.destroy_swapchain_locked(&mut state, handle);
        Ok(())
    }

    fn validate_surface(descriptor: &SurfaceDescriptor) -> RhiResult<()> {
        rhi_require(
            descriptor.native_handle != 0
                && descriptor.width > 0
                && descriptor.height > 0
                && descriptor.width <= u32::MAX as usize
                && descriptor.height <= u32::MAX as usize
                && descriptor.color_format != super::TextureFormat::Invalid,
            "swapchain requires a native handle, positive extent and color format",
        )
    }

    fn validate_unique_surface(
        state: &DeviceState,
        descriptor: &SurfaceDescriptor,
        excluding: Option<Swapchain>,
    ) -> RhiResult<()> {
        let duplicate = state.swapchains.iter().any(|(id, window)| {
            Some(*id) != excluding.map(Swapchain::id)
                && window.descriptor.kind == descriptor.kind
                && window.descriptor.native_handle == descriptor.native_handle
                && window.descriptor.display == descriptor.display
        });
        rhi_require(
            !duplicate,
            "a native window already has a swapchain on this device",
        )
    }

    /// Serializes a complete host operation, including resource preparation,
    /// begin_frame, submissions, presentation and end_frame, on a shared device.
    /// Low-level frame methods still require balanced begin/end on their caller.
    pub fn with_frame_session<T>(&self, body: impl FnOnce() -> RhiResult<T>) -> RhiResult<T> {
        // Swift uses `NSRecursiveLock`, so same-thread nesting is legal: a
        // preparation session may open a frame session, and the Swift suite
        // explicitly nests to prove an inner call is refused *with an error*
        // rather than blocking. A plain `Mutex` deadlocks there and hangs the
        // render thread silently. `std::sync::ReentrantLock` is still unstable,
        // so reentrancy is tracked per thread and the mutex is taken once.
        thread_local! {
            static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        }
        let outermost = DEPTH.with(|depth| {
            let previous = depth.get();
            depth.set(previous + 1);
            previous == 0
        });
        // Only the outermost call owns the lock; nested calls on the same thread
        // pass through, exactly like a recursive lock.
        let _guard = outermost.then(|| {
            self.session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        let result = body();
        DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        result
    }

    /// Keep each preparation's leases independent, including cache hits. The
    /// cleanup owns shared bookkeeping rather than a borrowed Device pointer.
    fn track_prepared(&self, resource: PreparedResource) {
        let Some(context) = ResourcePreparationContext::current() else {
            return;
        };
        let state = Arc::clone(&self.state);
        let backend = Arc::clone(&self.backend);
        let frame_ring = Arc::clone(&self.frame_ring);
        let retirements = Arc::clone(&self.retirements);
        let ready_retirements = Arc::clone(&self.ready_retirements);
        let next = Arc::clone(&self.next_retirement);
        context.acquired(self.id, resource.id(), move || {
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !resource.release(&mut state) {
                return;
            }
            let backend = Arc::clone(&backend);
            let id = next.fetch_add(1, Ordering::Relaxed);
            assert!(id != 0 && id != u64::MAX, "retirement sequence exhausted");
            retirements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(
                    id,
                    Box::new(move || {
                        resource.destroy(
                            &mut **backend
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner),
                        )
                    }),
                );
            let mut ready = frame_ring.register_retirement(id);
            ready.extend(std::mem::take(
                &mut *ready_retirements
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            ));
            let actions: Vec<_> = {
                let mut pending = retirements
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                ready
                    .into_iter()
                    .filter_map(|id| pending.remove(&id))
                    .collect()
            };
            for action in actions {
                action();
            }
        });
    }

    // MARK: Resources

    pub fn make_buffer(&self, descriptor: BufferDescriptor) -> RhiResult<Buffer> {
        let _state = self.lock();
        rhi_require(descriptor.size > 0, "buffer size must be positive")?;
        let handle = Buffer::new(self.identifiers.next());
        self.backend().create_buffer(handle, descriptor)?;
        self.track_prepared(PreparedResource::Buffer(handle));
        Ok(handle)
    }

    pub fn make_texture(&self, descriptor: TextureDescriptor) -> RhiResult<Texture> {
        let _state = self.lock();
        rhi_require(
            descriptor.width > 0
                && descriptor.height > 0
                && descriptor.depth > 0
                && descriptor.layers > 0
                && descriptor.mip_levels > 0
                && descriptor.sample_count > 0,
            "texture dimensions, layers, mips and samples must be positive",
        )?;
        if descriptor.dimension == super::TextureDimension::Texture3D {
            rhi_require(
                descriptor.layers == 1,
                "3D textures cannot have array layers",
            )?;
        } else {
            rhi_require(
                descriptor.depth == 1,
                "2D and cube textures require depth one",
            )?;
        }
        if descriptor.dimension == super::TextureDimension::Texture2D {
            rhi_require(
                descriptor.layers == 1,
                "use texture2DArray for array layers",
            )?;
        }
        if descriptor.dimension == super::TextureDimension::Cube {
            rhi_require(
                descriptor.layers == 1 && descriptor.width == descriptor.height,
                "cube textures require six square faces in one cube",
            )?;
        }
        rhi_require(
            descriptor.format != super::TextureFormat::Invalid,
            "texture format must be valid",
        )?;
        rhi_validate_texture_samples(&descriptor)?;
        let handle = Texture::new(self.identifiers.next());
        self.backend().create_texture(handle, descriptor)?;
        self.track_prepared(PreparedResource::Texture(handle));
        Ok(handle)
    }

    /// Allocation of this resource, when the backend can report it.
    pub fn texture_allocation(&self, texture: Texture) -> Option<TextureAllocation> {
        let _state = self.lock();
        self.backend().texture_allocation(texture)
    }

    pub fn make_sampler(&self, descriptor: SamplerDescriptor) -> RhiResult<Sampler> {
        let _state = self.lock();
        let handle = Sampler::new(self.identifiers.next());
        self.backend().create_sampler(handle, descriptor)?;
        self.track_prepared(PreparedResource::Sampler(handle));
        Ok(handle)
    }

    pub fn make_shader_module(
        &self,
        descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<ShaderModule> {
        let mut state = self.lock();
        descriptor.threadgroup_size.validate()?;
        let mut ids: Vec<u32> = descriptor
            .specialization_constants
            .iter()
            .map(|constant| constant.id)
            .collect();
        let requested = ids.len();
        ids.sort_unstable();
        ids.dedup();
        rhi_require(ids.len() == requested, "duplicate shader specialization ID")?;
        rhi_require(
            !descriptor.code.is_empty() && !descriptor.entry_point.is_empty(),
            "shader code and entry point must be non-empty",
        )?;
        if descriptor.stage == super::ShaderStage::Task && !self.capabilities.mesh_shading.task {
            return Err(RhiError::unsupported_feature(
                "task shaders are not implemented by this backend",
            ));
        }
        if (descriptor.stage == super::ShaderStage::Mesh
            || descriptor.stage == super::ShaderStage::Task)
            && !self.capabilities.mesh_shading.mesh
        {
            return Err(RhiError::unsupported_feature(
                "mesh shading is not implemented by this backend",
            ));
        }
        let during_frame = state.current_slot.is_some();
        let label = descriptor.entry_point.clone();
        let started = std::time::Instant::now();
        let key = ShaderModuleKey::new(&descriptor);
        let mut backend = self.backend();
        let lease = state.pipeline_store.shaders.acquire(key, || {
            let handle = ShaderModule::new(self.identifiers.next());
            backend.create_shader_module(handle, descriptor)?;
            Ok(handle)
        })?;
        drop(backend);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        if let Some(duplicate) = lease.discarded {
            self.backend().destroy_shader_module(duplicate);
        }
        if lease.created || lease.discarded.is_some() {
            state
                .pipeline_store
                .record_shader_module(nanoseconds, during_frame);
            self.log_pipeline_creation("shader module", Some(&label), nanoseconds);
            if during_frame {
                self.warn_pipeline_creation_during_frame("shader module", Some(&label));
            }
            if let Some(slot) = state.current_slot {
                self.frame_ring.note_pipeline_creation(slot, nanoseconds);
            }
        } else {
            state.pipeline_store.record_shader_cache_hit();
        }
        self.track_prepared(PreparedResource::Shader(lease.handle));
        Ok(lease.handle)
    }

    pub fn make_acceleration_structure(
        &self,
        descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<AccelerationStructure> {
        let mut state = self.lock();
        if !self.capabilities.ray_tracing.acceleration_structures {
            return Err(RhiError::unsupported_feature(
                "acceleration structures are not implemented by this backend",
            ));
        }
        match &descriptor {
            AccelerationStructureDescriptor::BottomLevel(triangles) => {
                rhi_require(
                    !triangles.is_empty()
                        && triangles.iter().all(|triangle| {
                            triangle.triangle_count > 0
                                && triangle.triangle_count <= (u32::MAX / 3) as i64
                                && triangle.vertex_offset >= 0
                                && triangle.vertex_offset % 4 == 0
                                && triangle.vertex_stride >= 12
                                && triangle.vertex_stride <= u32::MAX as i64
                                && triangle.vertex_stride % 4 == 0
                        }),
                    "invalid triangle geometry",
                )?;
            }
            AccelerationStructureDescriptor::TopLevel(instances) => {
                rhi_require(
                    !instances.is_empty()
                        && instances.len() <= 0x100_0000
                        && instances.iter().all(|instance| {
                            instance.mask <= 255
                                && [
                                    instance.transform.x,
                                    instance.transform.y,
                                    instance.transform.z,
                                ]
                                .into_iter()
                                .all(|row| row.into_iter().all(f32::is_finite))
                        }),
                    "invalid acceleration instance",
                )?;
            }
        }
        let handle = AccelerationStructure::new(self.identifiers.next());
        self.backend()
            .create_acceleration_structure(handle, descriptor.clone())?;
        let inputs = match &descriptor {
            AccelerationStructureDescriptor::BottomLevel(geometries) => geometries
                .iter()
                .map(|geometry| super::ResourceRef::buffer(geometry.vertices.id()))
                .collect(),
            AccelerationStructureDescriptor::TopLevel(instances) => instances
                .iter()
                .map(|instance| super::ResourceRef::acceleration_structure(instance.structure.id()))
                .collect(),
        };
        state.acceleration_build_inputs.insert(handle.id(), inputs);
        self.track_prepared(PreparedResource::Acceleration(handle));
        Ok(handle)
    }

    pub fn record_build(
        &self,
        structure: AccelerationStructure,
        commands: &mut CommandBuffer,
    ) -> RhiResult<()> {
        let state = self.lock();
        let Some(inputs) = state.acceleration_build_inputs.get(&structure.id()) else {
            return Err(RhiError::invalid_argument("unknown acceleration structure"));
        };
        commands.build_acceleration_structure(AccelerationStructureBuild {
            structure,
            inputs: inputs.clone(),
        });
        Ok(())
    }

    pub fn destroy_acceleration_structure(&self, structure: AccelerationStructure) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, structure.id());
        }
        let mut state = self.lock();
        state.acceleration_build_inputs.remove(&structure.id());
        state
            .planner
            .forget_resource(super::ResourceRef::acceleration_structure(structure.id()));
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_acceleration_structure(structure);
        }));
    }

    // MARK: Resource destruction (deferred)

    pub fn destroy_buffer(&self, buffer: Buffer) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, buffer.id());
        }
        let mut state = self.lock();
        state
            .planner
            .forget_resource(super::ResourceRef::buffer(buffer.id()));
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_buffer(buffer);
        }));
    }

    pub fn destroy_texture(&self, texture: Texture) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, texture.id());
        }
        let mut state = self.lock();
        state
            .planner
            .forget_resource(super::ResourceRef::texture(texture.id()));
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_texture(texture);
        }));
    }

    pub fn destroy_sampler(&self, sampler: Sampler) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, sampler.id());
        }
        let _state = self.lock();
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_sampler(sampler);
        }));
    }

    pub fn destroy_shader_module(&self, shader: ShaderModule) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, shader.id());
        }
        let mut state = self.lock();
        if !state.pipeline_store.shaders.release(shader) {
            return;
        }
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_shader_module(shader);
        }));
    }

    // MARK: Pipelines

    pub fn make_graphics_pipeline(
        &self,
        descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<GraphicsPipeline> {
        let mut state = self.lock();
        if !self.capabilities.graphics {
            return Err(RhiError::unsupported_feature(
                "graphics pipelines are not implemented by this backend",
            ));
        }
        rhi_require(
            state
                .interfaces
                .pipelines
                .contains_key(&descriptor.layout.id()),
            "unknown pipeline layout",
        )?;
        if descriptor.stencil_format.is_some() {
            return Err(RhiError::unsupported_feature(
                "stencil pipeline state is not implemented",
            ));
        }
        rhi_validate_sample_count(descriptor.sample_count)?;
        let during_frame = state.current_slot.is_some();
        let label = descriptor.label.map(str::to_owned);
        let started = std::time::Instant::now();
        let key = GraphicsPipelineKey::new(&descriptor);
        let mut backend = self.backend();
        let lease = state.pipeline_store.graphics.acquire(key, || {
            let handle = GraphicsPipeline::new(self.identifiers.next());
            backend.create_graphics_pipeline(handle, descriptor.clone())?;
            Ok(handle)
        })?;
        drop(backend);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        if let Some(duplicate) = lease.discarded {
            self.backend().destroy_graphics_pipeline(duplicate);
        }
        state.pipeline_store.record_graphics(
            nanoseconds,
            lease.created || lease.discarded.is_some(),
            during_frame,
        );
        if (lease.created || lease.discarded.is_some())
            && let Some(slot) = state.current_slot
        {
            self.frame_ring.note_pipeline_creation(slot, nanoseconds);
        }
        if lease.created {
            state.interfaces.uses.insert(
                lease.handle.id(),
                PipelineUse::new(descriptor.layout, PipelineKind::Graphics),
            );
            self.log_pipeline_creation("graphics", label.as_deref(), nanoseconds);
            if during_frame {
                self.warn_pipeline_creation_during_frame("graphics", label.as_deref());
            }
        }
        self.track_prepared(PreparedResource::Graphics(lease.handle));
        Ok(lease.handle)
    }

    pub fn make_mesh_pipeline(
        &self,
        descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<MeshPipeline> {
        let mut state = self.lock();
        if !self.capabilities.mesh_shading.mesh {
            return Err(RhiError::unsupported_feature(
                "mesh pipelines are not implemented by this backend",
            ));
        }
        rhi_require(
            state
                .interfaces
                .pipelines
                .contains_key(&descriptor.layout.id()),
            "unknown pipeline layout",
        )?;
        let during_frame = state.current_slot.is_some();
        let label = descriptor.label.map(str::to_owned);
        let started = std::time::Instant::now();
        let key = MeshPipelineKey::new(&descriptor);
        let mut backend = self.backend();
        let lease = state.pipeline_store.mesh.acquire(key, || {
            let handle = MeshPipeline::new(self.identifiers.next());
            backend.create_mesh_pipeline(handle, descriptor.clone())?;
            Ok(handle)
        })?;
        drop(backend);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        if let Some(duplicate) = lease.discarded {
            self.backend().destroy_mesh_pipeline(duplicate);
        }
        state.pipeline_store.record_mesh(
            nanoseconds,
            lease.created || lease.discarded.is_some(),
            during_frame,
        );
        if (lease.created || lease.discarded.is_some())
            && let Some(slot) = state.current_slot
        {
            self.frame_ring.note_pipeline_creation(slot, nanoseconds);
        }
        if lease.created {
            state.interfaces.uses.insert(
                lease.handle.id(),
                PipelineUse::new(descriptor.layout, PipelineKind::Mesh),
            );
            self.log_pipeline_creation("mesh", label.as_deref(), nanoseconds);
            if during_frame {
                self.warn_pipeline_creation_during_frame("mesh", label.as_deref());
            }
        }
        self.track_prepared(PreparedResource::Mesh(lease.handle));
        Ok(lease.handle)
    }

    pub fn make_compute_pipeline(
        &self,
        descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<ComputePipeline> {
        let mut state = self.lock();
        if !self.capabilities.compute {
            return Err(RhiError::unsupported_feature(
                "compute pipelines are not implemented by this backend",
            ));
        }
        rhi_require(
            state
                .interfaces
                .pipelines
                .contains_key(&descriptor.layout.id()),
            "unknown pipeline layout",
        )?;
        let during_frame = state.current_slot.is_some();
        let label = descriptor.label.map(str::to_owned);
        let started = std::time::Instant::now();
        let key = ComputePipelineKey::new(&descriptor);
        let mut backend = self.backend();
        let lease = state.pipeline_store.compute.acquire(key, || {
            let handle = ComputePipeline::new(self.identifiers.next());
            backend.create_compute_pipeline(handle, descriptor.clone())?;
            Ok(handle)
        })?;
        drop(backend);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        if let Some(duplicate) = lease.discarded {
            self.backend().destroy_compute_pipeline(duplicate);
        }
        state.pipeline_store.record_compute(
            nanoseconds,
            lease.created || lease.discarded.is_some(),
            during_frame,
        );
        if (lease.created || lease.discarded.is_some())
            && let Some(slot) = state.current_slot
        {
            self.frame_ring.note_pipeline_creation(slot, nanoseconds);
        }
        if lease.created {
            state.interfaces.uses.insert(
                lease.handle.id(),
                PipelineUse::new(descriptor.layout, PipelineKind::Compute),
            );
            self.log_pipeline_creation("compute", label.as_deref(), nanoseconds);
            if during_frame {
                self.warn_pipeline_creation_during_frame("compute", label.as_deref());
            }
        }
        self.track_prepared(PreparedResource::Compute(lease.handle));
        Ok(lease.handle)
    }

    pub fn destroy_mesh_pipeline(&self, pipeline: MeshPipeline) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, pipeline.id());
        }
        let mut state = self.lock();
        if !state.pipeline_store.mesh.release(pipeline) {
            return;
        }
        state.interfaces.uses.remove(&pipeline.id());
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_mesh_pipeline(pipeline);
        }));
    }

    pub fn destroy_graphics_pipeline(&self, pipeline: GraphicsPipeline) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, pipeline.id());
        }
        let mut state = self.lock();
        if !state.pipeline_store.graphics.release(pipeline) {
            return;
        }
        state.interfaces.uses.remove(&pipeline.id());
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_graphics_pipeline(pipeline);
        }));
    }

    pub fn destroy_compute_pipeline(&self, pipeline: ComputePipeline) {
        if let Some(context) = ResourcePreparationContext::current() {
            context.released(self.id, pipeline.id());
        }
        let mut state = self.lock();
        if !state.pipeline_store.compute.release(pipeline) {
            return;
        }
        state.interfaces.uses.remove(&pipeline.id());
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .destroy_compute_pipeline(pipeline);
        }));
    }

    // MARK: Binding layouts / sets / pipeline layouts

    pub fn make_binding_layout(
        &self,
        descriptor: BindingLayoutDescriptor,
    ) -> RhiResult<BindingLayout> {
        let mut state = self.lock();
        let mut slots: Vec<u32> = descriptor.entries.iter().map(|entry| entry.slot).collect();
        let count = slots.len();
        slots.sort_unstable();
        slots.dedup();
        rhi_require(
            count == slots.len(),
            "binding layout contains duplicate slots",
        )?;
        if descriptor.entries.iter().any(|entry| entry.array_size != 1) {
            return Err(RhiError::unsupported_feature(
                "binding arrays are not implemented",
            ));
        }
        rhi_require(
            descriptor.entries.iter().all(|entry| {
                entry.binding_type != super::BindingType::AccelerationStructure
                    || entry.visibility == super::ShaderVisibility::COMPUTE
            }),
            "acceleration structure bindings currently support compute ray queries",
        )?;
        if descriptor
            .entries
            .iter()
            .any(|entry| entry.binding_type == super::BindingType::AccelerationStructure)
            && !self.capabilities.ray_tracing.acceleration_structures
        {
            return Err(RhiError::unsupported_feature(
                "acceleration structure bindings are unavailable",
            ));
        }
        rhi_require(
            descriptor.entries.iter().all(|entry| {
                !entry.visibility.is_empty()
                    && (entry.visibility & !super::ShaderVisibility::ALL).is_empty()
            }),
            "invalid shader visibility",
        )?;
        if descriptor
            .entries
            .iter()
            .any(|entry| entry.visibility.contains(super::ShaderVisibility::MESH))
            && !self.capabilities.mesh_shading.mesh
        {
            return Err(RhiError::unsupported_feature(
                "mesh visibility is unavailable",
            ));
        }
        if descriptor
            .entries
            .iter()
            .any(|entry| entry.visibility.contains(super::ShaderVisibility::TASK))
            && !self.capabilities.mesh_shading.task
        {
            return Err(RhiError::unsupported_feature(
                "task visibility is unavailable",
            ));
        }
        rhi_require(
            descriptor
                .entries
                .iter()
                .all(|entry| entry.buffer.element_stride >= 0),
            "negative storage element stride",
        )?;
        let handle = BindingLayout::new(self.identifiers.next());
        self.backend()
            .register_binding_layout(handle, descriptor.clone())?;
        state
            .caches
            .store_binding_layout(handle.id(), descriptor.entries.clone());
        state.interfaces.bindings.insert(handle.id(), descriptor);
        Ok(handle)
    }

    pub fn make_pipeline_layout(
        &self,
        descriptor: PipelineLayoutDescriptor,
    ) -> RhiResult<PipelineLayout> {
        let mut state = self.lock();
        state.interfaces.validate(&descriptor)?;
        let set_ids: Vec<u32> = descriptor
            .set_layouts
            .iter()
            .map(|layout| layout.id())
            .collect();
        for set_id in &set_ids {
            if state.caches.binding_layout_entries(*set_id).is_none() {
                return Err(RhiError::invalid_argument(
                    "pipeline layout references an unknown binding layout",
                ));
            }
        }
        let key = super::PipelineInterfaceKey {
            set_layouts: set_ids.clone(),
            push_constants: descriptor.push_constants.clone(),
        };
        if let Some(existing) = state.caches.pipeline_layouts.layout(&key) {
            return Ok(PipelineLayout::new(existing));
        }
        let handle = PipelineLayout::new(self.identifiers.next());
        self.backend()
            .register_pipeline_layout(handle, descriptor.clone())?;
        state.interfaces.pipelines.insert(handle.id(), descriptor);
        state.caches.pipeline_layouts.insert(handle.id(), key);
        state
            .caches
            .store_pipeline_layout_definition(handle.id(), set_ids);
        Ok(handle)
    }

    pub fn make_binding_set(
        &self,
        layout: BindingLayout,
        descriptor: BindingSetDescriptor,
    ) -> RhiResult<BindingSet> {
        let mut state = self.lock();
        let Some(layout_entries) = state
            .caches
            .binding_layout_entries(layout.id())
            .map(<[_]>::to_vec)
        else {
            return Err(RhiError::invalid_argument("unknown binding layout"));
        };
        if let Some(existing) = state
            .caches
            .binding_sets
            .existing(layout.id(), &descriptor.entries)
        {
            return Ok(BindingSet::new(existing));
        }
        // Layouts and cached sets are immutable. A full-value cache hit already
        // passed this validation when the set was registered.
        Self::validate_binding_set(&descriptor, &layout_entries)?;
        if let Some(evicted) = state.caches.binding_sets.evict_if_full() {
            self.teardown_binding_set(&mut state, evicted);
        }

        let handle = BindingSet::new(self.identifiers.next());
        state
            .caches
            .binding_sets
            .insert(handle.id(), layout.id(), &descriptor.entries);
        state.caches.store_binding_set(handle.id(), layout.id());
        let read_only: HashSet<u32> = layout_entries
            .iter()
            .filter(|entry| entry.buffer.read_only)
            .map(|entry| entry.slot)
            .collect();
        state
            .planner
            .register_binding_set_entries(handle.id(), &descriptor.entries, &read_only);

        if let Err(error) = self.backend().register_binding_set(
            handle,
            layout,
            layout_entries,
            descriptor.entries.clone(),
        ) {
            // Roll back the frontend so we never hold a set the backend lacks
            // (the Zig prototype silently swallowed this and mis-bound).
            state
                .caches
                .binding_sets
                .remove(layout.id(), &descriptor.entries);
            state.caches.remove_binding_set_owner(handle.id());
            state.planner.remove_binding_set_entries(handle.id());
            return Err(error);
        }
        Ok(handle)
    }

    /// Whether `set` is still registered. The binding-set cache evicts its least
    /// recently created sets when full, so callers that keep a set across frames
    /// must check it (or ask again) before binding it.
    pub fn is_binding_set_live(&self, set: BindingSet) -> bool {
        self.lock().caches.binding_set_layout_id(set.id()).is_some()
    }

    fn teardown_binding_set(&self, state: &mut DeviceState, handle_id: u32) {
        state.caches.remove_binding_set_owner(handle_id);
        state.planner.remove_binding_set_entries(handle_id);
        let handle = BindingSet::new(handle_id);
        let backend = Arc::clone(&self.backend);
        self.retire_current(Box::new(move || {
            backend
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .unregister_binding_set(handle);
        }));
    }

    fn validate_binding_set(
        descriptor: &BindingSetDescriptor,
        layout_entries: &[BindingLayoutEntry],
    ) -> RhiResult<()> {
        if descriptor.entries.len() != layout_entries.len() {
            return Err(RhiError::layout_mismatch(format!(
                "binding set has {} entries; layout expects {}",
                descriptor.entries.len(),
                layout_entries.len()
            )));
        }
        for layout_entry in layout_entries {
            let Some(entry) = descriptor
                .entries
                .iter()
                .find(|entry| entry.slot == layout_entry.slot)
            else {
                return Err(RhiError::layout_mismatch(format!(
                    "binding set is missing entry for slot {}",
                    layout_entry.slot
                )));
            };
            if !resource_matches(&entry.resource, layout_entry.binding_type) {
                return Err(RhiError::layout_mismatch(format!(
                    "resource at slot {} does not match the declared binding type",
                    layout_entry.slot
                )));
            }
            if layout_entry.buffer.has_dynamic_offset && entry.resource.buffer_offset() != 0 {
                // Dynamic bindings bind the base buffer (offset 0); the per-frame
                // offset is supplied when the set is bound.
                return Err(RhiError::layout_mismatch(format!(
                    "dynamic binding at slot {} must use base offset 0",
                    layout_entry.slot
                )));
            }
        }
        Ok(())
    }

    // MARK: Frame lifecycle

    /// Begins a frame, blocking the CPU only if the chosen slot's prior frame is
    /// still executing on the GPU.
    pub fn begin_frame(&self) -> RhiResult<()> {
        let mut state = self.lock();
        rhi_require(state.current_slot.is_none(), "a frame is already active")?;
        let (slot, ready) = if self.backend_api == super::GraphicsApi::WebGpu {
            self.frame_ring.try_begin().ok_or(RhiError::GpuBusy)?
        } else {
            self.frame_ring.begin()
        };
        self.drain_retirements(ready);
        match self.backend().make_frame_uploader(slot) {
            Ok(mut uploader) => {
                uploader.reset();
                state.current_slot = Some(slot);
                state.current_uploader = Some(uploader);
                state.frame_sequence += 1;
                Ok(())
            }
            Err(error) => {
                self.drain_retirements(self.frame_ring.end());
                Err(error)
            }
        }
    }

    /// Requests timestamps for submissions recorded after this call in the active frame.
    pub fn measure_gpu_frame(&self) -> Option<super::GpuFrameTiming> {
        let state = self.lock();
        let slot = state.current_slot?;
        Some(self.frame_ring.measure(slot))
    }

    /// Ends the frame and releases the per-frame upload context.
    pub fn end_frame(&self) {
        let mut state = self.lock();
        // Returning abandoned acquisitions consumes Vulkan's binary wait and
        // prevents a failed recording from stranding a window image.
        let abandoned: Vec<SwapchainImage> = state
            .swapchains
            .values()
            .filter_map(|window| window.acquired)
            .collect();
        for image in abandoned {
            if let Err(error) = self.present_locked(&mut state, image) {
                state.submission_fault = Some(RhiError::present_failed(format!(
                    "abandoned image release failed: {error}"
                )));
            }
        }
        self.drain_retirements(self.frame_ring.end());
        state.current_slot = None;
        state.current_uploader = None;
    }

    /// Suballocates transient per-frame storage (for push constants / small
    /// constants) and returns the buffer + offset to bind.
    pub fn upload_transient(&self, data: &[u8], alignment: usize) -> RhiResult<UploadLocation> {
        let mut state = self.lock();
        let Some(uploader) = state.current_uploader.as_mut() else {
            return Err(RhiError::FrameNotActive);
        };
        uploader.write(data, alignment)
    }

    // MARK: Resource preparation

    /// Whether this backend implements the snapshot/compile/publish split that
    /// makes non-blocking resource preparation possible.
    pub fn supports_background_compilation(&self) -> bool {
        self.backend().supports_background_compilation()
    }

    /// Runs one declared preparation on the calling thread.
    ///
    /// Rust has no `async`/`await` continuation here: the Swift version hopped
    /// to a preparation queue and resumed a continuation, which only buys
    /// concurrency. Callers that want it run this on their own worker and
    /// collect the progress through `on_progress`.
    pub fn prepare_resources<T>(
        &self,
        label: impl Into<String>,
        total_steps: Option<usize>,
        on_progress: impl Fn(ResourcePreparationProgress) + Send + Sync + 'static,
        operation: impl FnOnce(&ResourcePreparationContext) -> RhiResult<T>,
    ) -> RhiResult<T> {
        if !self.supports_background_compilation() {
            return Err(RhiError::unsupported_feature(
                "backend does not support background compilation",
            ));
        }
        let context = ResourcePreparationContext::new(label, total_steps, on_progress);
        let context = std::rc::Rc::new(context);
        let probe = context.clone();
        context.run(move || operation(&probe))
    }

    /// One-step shader preparation, mirroring the Swift `AsyncResources` entry.
    pub fn make_shader_module_prepared(
        &self,
        descriptor: ShaderModuleDescriptor,
        on_progress: impl Fn(ResourcePreparationProgress) + Send + Sync + 'static,
    ) -> RhiResult<ShaderModule> {
        let label = descriptor.entry_point.clone();
        self.prepare_resources(label, Some(1), on_progress, |context| {
            context.step("Shader", || self.make_shader_module(descriptor))
        })
    }

    /// One-step graphics-pipeline preparation.
    pub fn make_graphics_pipeline_prepared(
        &self,
        descriptor: GraphicsPipelineDescriptor,
        on_progress: impl Fn(ResourcePreparationProgress) + Send + Sync + 'static,
    ) -> RhiResult<GraphicsPipeline> {
        let label = descriptor.label.unwrap_or("Graphics pipeline").to_string();
        self.prepare_resources(label, Some(1), on_progress, |context| {
            context.step("Graphics pipeline", || {
                self.make_graphics_pipeline(descriptor)
            })
        })
    }

    /// One-step compute-pipeline preparation.
    pub fn make_compute_pipeline_prepared(
        &self,
        descriptor: ComputePipelineDescriptor,
        on_progress: impl Fn(ResourcePreparationProgress) + Send + Sync + 'static,
    ) -> RhiResult<ComputePipeline> {
        let label = descriptor.label.unwrap_or("Compute pipeline").to_string();
        self.prepare_resources(label, Some(1), on_progress, |context| {
            context.step("Compute pipeline", || {
                self.make_compute_pipeline(descriptor)
            })
        })
    }

    /// One-step mesh-pipeline preparation.
    pub fn make_mesh_pipeline_prepared(
        &self,
        descriptor: MeshPipelineDescriptor,
        on_progress: impl Fn(ResourcePreparationProgress) + Send + Sync + 'static,
    ) -> RhiResult<MeshPipeline> {
        let label = descriptor.label.unwrap_or("Mesh pipeline").to_string();
        self.prepare_resources(label, Some(1), on_progress, |context| {
            context.step("Mesh pipeline", || self.make_mesh_pipeline(descriptor))
        })
    }

    // MARK: Submission

    /// Whether this backend can timestamp GPU execution for queued submissions.
    pub fn supports_gpu_timestamps(&self) -> bool {
        self.backend().supports_gpu_timestamps()
    }

    /// Attaches a GPU timing profile, or `None` to stop recording. Sampling
    /// starts with the next submission; results arrive asynchronously on the
    /// backend's completion thread, so read them once the sampled frames have
    /// retired.
    pub fn set_gpu_timestamp_profile(&self, profile: Option<Arc<GpuTimestampProfile>>) {
        self.backend().set_gpu_timestamp_profile(profile);
    }

    /// The profile attached right now, if any.
    ///
    /// Several readers can share one profile: each drains its own cursor, so a
    /// telemetry reporter and a DevTools session see the same samples without
    /// consuming each other's.
    pub fn gpu_timestamp_profile(&self) -> Option<Arc<GpuTimestampProfile>> {
        self.backend().gpu_timestamp_profile()
    }

    /// Plans and submits a recorded command buffer. Submission is asynchronous;
    /// completion is tracked against the current frame slot.
    pub fn submit(
        &self,
        command_buffer: &CommandBuffer,
        queue: QueueClass,
        waits: &[TimelineSemaphore],
        signals: &[TimelineSemaphore],
        mut cpu_profile: Option<&mut SubmissionCpuProfile>,
    ) -> RhiResult<()> {
        let mut state = self.lock();
        let Some(slot) = state.current_slot else {
            return Err(RhiError::FrameNotActive);
        };
        if let Some(fault) = state.submission_fault.clone() {
            return Err(fault);
        }
        let queue_count = match queue {
            QueueClass::Graphics => self.capabilities.max_queues.graphics,
            QueueClass::Compute => self.capabilities.max_queues.compute,
            QueueClass::Transfer => self.capabilities.max_queues.transfer,
        };
        if queue_count == 0 {
            return Err(RhiError::unsupported_feature(
                "requested queue is unavailable",
            ));
        }

        let validation_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
        self.validate_commands(command_buffer, queue, &state)?;
        if let Some(profile) = cpu_profile.as_deref_mut() {
            profile.end(super::CpuProfilePhase::Validation, validation_start);
        }

        let planning_start = cpu_profile.as_deref().and_then(SubmissionCpuProfile::begin);
        let rollback = state.planner.checkpoint();
        let external = SubmitDescriptor {
            wait_semaphores: waits.to_vec(),
            signal_semaphores: signals.to_vec(),
        };
        let plan = match state
            .planner
            .build_plan(queue, command_buffer.commands(), &external)
        {
            Ok(plan) => plan,
            Err(error) => {
                state.planner.restore(rollback.clone());
                return Err(error);
            }
        };
        if let Some(profile) = cpu_profile.as_deref_mut() {
            profile.end(super::CpuProfilePhase::Planning, planning_start);
        }

        let mut submitted = false;
        for planned_submit in &plan.submits {
            self.frame_ring.register_command_buffer(slot);
            let frame_ring = Arc::clone(&self.frame_ring);
            let ready = Arc::clone(&self.ready_retirements);
            let completion: super::Completion = Box::new(move || {
                let completed = frame_ring.command_buffer_completed(slot);
                ready
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .extend(completed);
            });
            let epoch = self.frame_ring.measurement_epoch(slot);
            let result = if let Some(epoch) = epoch {
                let ring = self.frame_ring.clone();
                self.backend().submit_timed(
                    planned_submit,
                    cpu_profile.as_deref_mut(),
                    Box::new(move |nanos| {
                        ring.note_gpu_sample(slot, epoch, nanos);
                        completion();
                    }),
                )
            } else {
                self.backend()
                    .submit(planned_submit, cpu_profile.as_deref_mut(), completion)
            };
            match result {
                Ok(()) => submitted = true,
                Err(error) => {
                    if let Some(epoch) = epoch {
                        self.frame_ring.note_gpu_sample(slot, epoch, None);
                    }
                    self.drain_retirements(self.frame_ring.unregister_command_buffer(slot));
                    if submitted {
                        state.submission_fault = Some(RhiError::submit_failed(
                            "partial submission failed; recreate the device before further submissions",
                        ));
                    } else {
                        state.planner.restore(rollback.clone());
                    }
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn validate_commands(
        &self,
        command_buffer: &CommandBuffer,
        queue: QueueClass,
        state: &DeviceState,
    ) -> RhiResult<()> {
        // Pass-level capability checks first; per-command capability checks ride
        // along with the layout walk so each pass body is traversed once.
        for command in command_buffer.commands() {
            match command {
                super::RecordedCommand::AccelerationStructureBuild(_) => {
                    if queue == QueueClass::Transfer {
                        return Err(RhiError::invalid_argument(
                            "acceleration builds require graphics or compute queue",
                        ));
                    }
                    if !self.capabilities.ray_tracing.acceleration_structures {
                        return Err(RhiError::unsupported_feature(
                            "acceleration structure builds are not implemented by this backend",
                        ));
                    }
                }
                super::RecordedCommand::ComputePass(_) => {
                    if !self.capabilities.compute {
                        return Err(RhiError::unsupported_feature(
                            "compute commands are not implemented by this backend",
                        ));
                    }
                    if queue == QueueClass::Transfer {
                        return Err(RhiError::invalid_argument(
                            "compute passes cannot execute on a transfer queue",
                        ));
                    }
                }
                super::RecordedCommand::RenderPass(_) => {
                    rhi_require(
                        queue == QueueClass::Graphics,
                        "render passes require the graphics queue",
                    )?;
                    if !self.capabilities.graphics {
                        return Err(RhiError::unsupported_feature(
                            "graphics commands are not implemented by this backend",
                        ));
                    }
                }
                _ => {}
            }
        }
        state.interfaces.validate_commands(
            command_buffer,
            &state.caches,
            self.capabilities.mesh_shading.mesh,
        )
    }

    // MARK: Swapchain

    pub fn acquire_swapchain_image(&self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        let mut state = self.lock();
        if state.current_slot.is_none() {
            return Err(RhiError::FrameNotActive);
        }
        let sequence = state.frame_sequence;
        let Some(window) = state.swapchains.get(&handle.id()) else {
            return Err(RhiError::SwapchainAcquireFailed(
                "acquire each live swapchain at most once per frame".into(),
            ));
        };
        if window.acquisition_frame == Some(sequence) {
            return Err(RhiError::SwapchainAcquireFailed(
                "acquire each live swapchain at most once per frame".into(),
            ));
        }
        let image = self.backend().acquire_swapchain_image(handle)?;
        rhi_require(
            image.swapchain == handle
                && image.generation > 0
                && image.texture.id() != 0
                && image.width > 0
                && image.height > 0,
            "backend returned an invalid swapchain image",
        )?;
        let mut window = state
            .swapchains
            .remove(&handle.id())
            .expect("checked above");
        if window.generation != Some(image.generation) {
            for texture in &window.textures {
                state
                    .planner
                    .forget_resource(super::ResourceRef::texture(texture.id()));
            }
            window.textures.clear();
            window.generation = Some(image.generation);
        }
        window.acquired = Some(image);
        window.acquisition_frame = Some(sequence);
        window.textures.insert(image.texture);
        state.swapchains.insert(handle.id(), window);
        Ok(image)
    }

    pub fn present(&self, image: SwapchainImage) -> RhiResult<()> {
        let mut state = self.lock();
        self.present_locked(&mut state, image)
    }

    fn present_locked(&self, state: &mut DeviceState, image: SwapchainImage) -> RhiResult<()> {
        let Some(slot) = state.current_slot else {
            return Err(RhiError::FrameNotActive);
        };
        let current = state
            .swapchains
            .get(&image.swapchain.id())
            .and_then(|window| window.acquired);
        if current != Some(image) {
            return Err(RhiError::present_failed(
                "image is stale, already presented or belongs to another swapchain",
            ));
        }
        self.frame_ring.register_command_buffer(slot);
        let frame_ring = Arc::clone(&self.frame_ring);
        let ready = Arc::clone(&self.ready_retirements);
        let completion: super::Completion = Box::new(move || {
            let completed = frame_ring.command_buffer_completed(slot);
            ready
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend(completed);
        });
        if let Err(error) = self.backend().present(image, completion) {
            self.drain_retirements(self.frame_ring.unregister_command_buffer(slot));
            return Err(error);
        }
        if let Some(window) = state.swapchains.get_mut(&image.swapchain.id()) {
            window.acquired = None;
        }
        state
            .planner
            .record_presentation(super::ResourceRef::texture(image.texture.id()));
        Ok(())
    }

    // MARK: Immediate data transfer

    pub fn upload_buffer_data(&self, buffer: Buffer, offset: usize, data: &[u8]) -> RhiResult<()> {
        let mut state = self.lock();
        self.backend().upload_buffer_data(buffer, offset, data)?;
        if !data.is_empty() {
            state
                .planner
                .record_immediate_write(super::ResourceRef::buffer(buffer.id()));
        }
        Ok(())
    }

    pub fn read_buffer_data(
        &self,
        buffer: Buffer,
        offset: usize,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        let state = self.lock();
        rhi_require(
            state.current_slot.is_none(),
            "end_frame must precede buffer readback",
        )?;
        self.backend().read_buffer_data(buffer, offset, destination)
    }

    pub fn upload_texture_data(
        &self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()> {
        let mut state = self.lock();
        self.backend()
            .upload_texture_data(texture, data, region, bytes_per_row, subresource)?;
        state
            .planner
            .record_immediate_write(super::ResourceRef::texture(texture.id()));
        Ok(())
    }

    pub fn read_texture_data(
        &self,
        texture: Texture,
        width: usize,
        height: usize,
        bytes_per_row: usize,
        subresource: TextureSubresource,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        let _state = self.lock();
        self.backend().read_texture_data(
            texture,
            width,
            height,
            bytes_per_row,
            subresource,
            destination,
        )
    }

    // MARK: Caches / diagnostics

    pub fn binding_set_cache_stats(&self) -> CacheStats {
        self.lock().caches.binding_sets.stats()
    }

    /// Cold/warm pipeline creation counters, per kind. Reading this is
    /// lock-protected and does not touch the GPU.
    pub fn pipeline_statistics(&self) -> PipelineStatistics {
        self.lock().pipeline_store.snapshot()
    }

    /// Clears the counters but keeps interned pipelines alive.
    pub fn reset_pipeline_statistics(&self) {
        self.lock().pipeline_store.reset_statistics();
    }

    /// Blocks until all queued GPU work has completed.
    pub fn wait_until_idle(&self) -> RhiResult<()> {
        let state = self.lock();
        rhi_require(
            state.current_slot.is_none(),
            "end_frame must precede wait_until_idle",
        )?;
        self.wait_for_frame_retirements()?;
        self.backend().wait_until_idle()
    }

    fn wait_for_frame_retirements(&self) -> RhiResult<()> {
        if self.backend_api == super::GraphicsApi::WebGpu && !self.frame_ring.is_idle() {
            return Err(RhiError::GpuBusy);
        }
        self.drain_retirements(self.frame_ring.wait_until_idle());
        Ok(())
    }

    fn retire_current(&self, action: super::Completion) {
        let id = self.next_retirement.fetch_add(1, Ordering::Relaxed);
        assert!(id != 0 && id != u64::MAX, "retirement sequence exhausted");
        self.retirements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, action);
        self.drain_retirements(self.frame_ring.register_retirement(id));
    }

    /// GPU callbacks only enqueue identifiers; native destruction stays on the API thread.
    fn drain_retirements(&self, mut ready: Vec<u64>) {
        ready.extend(std::mem::take(
            &mut *self
                .ready_retirements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        ));
        let actions = {
            let mut pending = self
                .retirements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            ready
                .into_iter()
                .filter_map(|id| pending.remove(&id))
                .collect::<Vec<_>>()
        };
        for action in actions {
            action();
        }
    }

    // MARK: Diagnostics

    fn log_pipeline_creation(&self, kind: &str, label: Option<&str>, nanoseconds: u64) {
        self.log(
            Level::Debug,
            "Pipeline created",
            [
                ("kind", kind.to_owned()),
                ("label", label.unwrap_or("<unnamed>").to_owned()),
                ("ms", format!("{}", nanoseconds as f64 / 1_000_000.0)),
            ],
        );
    }

    /// Pipeline creation belongs on a load/prewarm path, not between
    /// `begin_frame` and `end_frame`. This is a diagnostic rather than a hard
    /// failure so a future background compiler can still run while frames are in
    /// flight.
    fn warn_pipeline_creation_during_frame(&self, kind: &str, label: Option<&str>) {
        self.log(
            Level::Warning,
            "Pipeline created during an active frame",
            [
                ("kind", kind.to_owned()),
                ("label", label.unwrap_or("<unnamed>").to_owned()),
            ],
        );
    }

    fn log<const N: usize>(&self, level: Level, message: &str, metadata: [(&str, String); N]) {
        let logger = self
            .logger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(logger) = logger.as_ref() else {
            return;
        };
        let metadata: std::collections::BTreeMap<String, LogValue> = metadata
            .into_iter()
            .map(|(key, value)| (key.to_owned(), LogValue::from(value.as_str())))
            .collect();
        let _ = logger.log(
            level,
            "com.cove.engine.rhi",
            message,
            metadata,
            Location {
                source: module_path!().to_owned(),
                file: file!().to_owned(),
                function: "Device".to_owned(),
                line: u64::from(line!()),
            },
        );
    }
}

#[derive(Clone, Copy)]
enum PreparedResource {
    Buffer(Buffer),
    Texture(Texture),
    Sampler(Sampler),
    Shader(ShaderModule),
    Acceleration(AccelerationStructure),
    Graphics(GraphicsPipeline),
    Mesh(MeshPipeline),
    Compute(ComputePipeline),
}

impl PreparedResource {
    fn id(self) -> u32 {
        match self {
            Self::Buffer(v) => v.id(),
            Self::Texture(v) => v.id(),
            Self::Sampler(v) => v.id(),
            Self::Shader(v) => v.id(),
            Self::Acceleration(v) => v.id(),
            Self::Graphics(v) => v.id(),
            Self::Mesh(v) => v.id(),
            Self::Compute(v) => v.id(),
        }
    }

    fn release(self, state: &mut DeviceState) -> bool {
        match self {
            Self::Buffer(v) => state
                .planner
                .forget_resource(super::ResourceRef::buffer(v.id())),
            Self::Texture(v) => state
                .planner
                .forget_resource(super::ResourceRef::texture(v.id())),
            Self::Acceleration(v) => {
                state.acceleration_build_inputs.remove(&v.id());
                state
                    .planner
                    .forget_resource(super::ResourceRef::acceleration_structure(v.id()));
            }
            Self::Shader(v) => return state.pipeline_store.shaders.release(v),
            Self::Graphics(v) => {
                if !state.pipeline_store.graphics.release(v) {
                    return false;
                }
                state.interfaces.uses.remove(&v.id());
            }
            Self::Mesh(v) => {
                if !state.pipeline_store.mesh.release(v) {
                    return false;
                }
                state.interfaces.uses.remove(&v.id());
            }
            Self::Compute(v) => {
                if !state.pipeline_store.compute.release(v) {
                    return false;
                }
                state.interfaces.uses.remove(&v.id());
            }
            Self::Sampler(_) => {}
        }
        true
    }

    fn destroy(self, backend: &mut dyn RhiBackend) {
        match self {
            Self::Buffer(v) => backend.destroy_buffer(v),
            Self::Texture(v) => backend.destroy_texture(v),
            Self::Sampler(v) => backend.destroy_sampler(v),
            Self::Shader(v) => backend.destroy_shader_module(v),
            Self::Acceleration(v) => backend.destroy_acceleration_structure(v),
            Self::Graphics(v) => backend.destroy_graphics_pipeline(v),
            Self::Mesh(v) => backend.destroy_mesh_pipeline(v),
            Self::Compute(v) => backend.destroy_compute_pipeline(v),
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let active = { self.lock().current_slot.is_some() };
        if active {
            self.end_frame();
        }
        let _ = self.wait_until_idle();
        self.drain_retirements(Vec::new());
    }
}

fn resource_matches(resource: &BindingResource, binding_type: super::BindingType) -> bool {
    use super::BindingType;
    matches!(
        (resource, binding_type),
        (BindingResource::Sampler(_), BindingType::Sampler)
            | (BindingResource::Texture(_), BindingType::Texture)
            | (
                BindingResource::StorageTexture(_),
                BindingType::StorageTexture
            )
            | (
                BindingResource::UniformBuffer { .. },
                BindingType::UniformBuffer
            )
            | (
                BindingResource::StorageBuffer { .. },
                BindingType::StorageBuffer
            )
            | (
                BindingResource::AccelerationStructure(_),
                BindingType::AccelerationStructure
            )
    )
}

/// Allocates a process-unique identity for each constructed device. Resource
/// preparation uses it to key per-device ownership without a reference cycle.
pub fn next_device_id() -> u64 {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    u64::from(NEXT.fetch_add(1, Ordering::SeqCst))
}
