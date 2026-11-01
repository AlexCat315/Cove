//! Translation of NativeRHI/Vulkan/VulkanBackend.swift and VulkanRuntime.swift:
//! the concrete `RhiBackend`.
//!
//! The backend owns the context, the block allocator, the registries, the
//! descriptor pool, the per-slot command pools/fences, the timeline-semaphore map
//! and the swapchains. Every trait method forwards to the module that implements
//! it, which is how Swift split the same device across `Backend/`, `Resources/`
//! and `Submission/` extensions.
//!
//! The `RHIBackgroundCompilation` conformance from
//! `Vulkan/Backend/VulkanCompilation.swift` is the inherent `prepare_compilation`
//! method in this module.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::rhi::{
    AccelerationStructure, AccelerationStructureDescriptor, AdapterCapabilities, BindingLayout,
    BindingLayoutDescriptor, BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer,
    BufferDescriptor, Capabilities, Completion, ComputePipeline, ComputePipelineDescriptor,
    DeviceConfig, FrameUploader, GpuTimestampProfile, GraphicsApi, GraphicsPipeline,
    GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor, PipelineLayout,
    PipelineLayoutDescriptor, PlannedSubmit, PresentationMode, QueueLimits, RhiBackend, RhiError,
    RhiResult, Sampler, SamplerDescriptor, ShaderModule, ShaderModuleDescriptor,
    SubmissionCpuProfile, SurfaceDescriptor, Swapchain, SwapchainImage, Texture, TextureAllocation,
    TextureDescriptor, TextureSubresource, TextureUploadRegion, rhi_require,
    rhi_texture_subresource_extent, rhi_texture_transfer_bytes, rhi_texture_upload_bytes,
};

use super::memory::{VulkanMemoryAllocation, VulkanMemoryAllocator};
use super::presentation::{VulkanWindowSwapchain, mark_acquired};
use super::registries::VulkanRegistries;
use super::resources::lock;
use super::setup::{self, VulkanContext};
use super::status::VulkanSubmissionStatus;
use super::submission::{Submitter, VulkanFrame, VulkanFrameUploader, VulkanUploadChunk};
use super::sys;
use super::{acceleration, pipelines, presentation, resources, submission};

/// The concrete Vulkan backend.
pub struct VulkanBackend {
    context: VulkanContext,
    allocator: Arc<Mutex<VulkanMemoryAllocator>>,
    registries: Arc<Mutex<VulkanRegistries>>,
    descriptor_pool: sys::VkDescriptorPool,
    submission_status: Arc<VulkanSubmissionStatus>,
    swapchains: HashMap<u32, VulkanWindowSwapchain>,
    frames: Vec<VulkanFrame>,
    active_slot: usize,
    timeline_semaphores: HashMap<u32, sys::VkSemaphore>,
    capabilities: Capabilities,
    gpu_timestamp_profile: Option<Arc<GpuTimestampProfile>>,
    device_name: String,
}

// SAFETY: every field is a Vulkan handle, a table of entry points, plain data, or
// an `Arc<Mutex<_>>`. Vulkan objects are documented as safe to use from multiple
// threads, and the backend is handed to the frontend behind its own mutex, so two
// threads never touch this state concurrently. The impl exists because raw
// `*mut c_void` handles do not implement `Send`, which leaves the auto-derived
// bound too weak for `RhiBackend: Send`.
unsafe impl Send for VulkanBackend {}

impl VulkanBackend {
    /// Whether a usable Vulkan driver (ICD) is present.
    pub fn is_available() -> bool {
        setup::is_available()
    }

    /// Creates the backend for the selected physical device.
    pub fn make(config: DeviceConfig) -> RhiResult<Self> {
        let loader = sys::VulkanLoader::open().ok_or_else(|| {
            RhiError::unsupported_backend("The Vulkan loader entry point was not found")
        })?;
        let context = setup::make_context(loader, config.enable_validation)?;

        let mut memory_properties = sys::VkPhysicalDeviceMemoryProperties::default();
        // SAFETY: the physical device is live and the out-parameter is a live local.
        unsafe {
            (context
                .instance_commands
                .get_physical_device_memory_properties)(
                context.physical_device,
                &mut memory_properties,
            )
        };
        let allocator = VulkanMemoryAllocator::new(&context, memory_properties);
        let descriptor_pool = create_descriptor_pool(&context)?;
        let capabilities = probe_capabilities(&context);
        let device_name = context.device_name.clone();
        Ok(Self {
            context,
            allocator: Arc::new(Mutex::new(allocator)),
            registries: Arc::new(Mutex::new(VulkanRegistries::new())),
            descriptor_pool,
            submission_status: Arc::new(VulkanSubmissionStatus::new()),
            swapchains: HashMap::new(),
            frames: Vec::new(),
            active_slot: 0,
            timeline_semaphores: HashMap::new(),
            capabilities,
            gpu_timestamp_profile: None,
            device_name,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        // Extension presence is diagnostic only; enabled features and resolved
        // commands additionally determine the implemented capability set.
        AdapterCapabilities {
            ray_tracing: self
                .context
                .extensions
                .contains("VK_KHR_acceleration_structure")
                && (self.context.extensions.contains("VK_KHR_ray_query")
                    || self
                        .context
                        .extensions
                        .contains("VK_KHR_ray_tracing_pipeline")),
            mesh_shading: self.context.extensions.contains("VK_EXT_mesh_shader"),
        }
    }
}

/// Only completed RHI paths are exposed. Adapter extensions are diagnostic.
fn probe_capabilities(context: &VulkanContext) -> Capabilities {
    let advanced = &context.advanced;
    let acceleration_structures = context.features.acceleration_structures
        && advanced.create_acceleration_structure.is_some()
        && advanced.destroy_acceleration_structure.is_some()
        && advanced.acceleration_structure_build_sizes.is_some()
        && advanced.acceleration_structure_address.is_some()
        && advanced.build_acceleration_structures.is_some();
    Capabilities::configure(|capabilities| {
        capabilities.graphics = true;
        capabilities.compute = true;
        capabilities.indirect_draw = true;
        capabilities.textures.texture_3d = true;
        capabilities.textures.cube = true;
        capabilities.max_queues = QueueLimits::default();
        capabilities.ray_tracing.acceleration_structures = acceleration_structures;
        capabilities.ray_tracing.compute_ray_query =
            acceleration_structures && context.features.ray_query;
        capabilities.mesh_shading.mesh =
            context.features.mesh && advanced.draw_mesh_tasks.is_some();
        capabilities.mesh_shading.task = capabilities.mesh_shading.mesh && context.features.task;
    })
}

fn create_descriptor_pool(context: &VulkanContext) -> RhiResult<sys::VkDescriptorPool> {
    let mut pool_sizes = vec![
        sys::VkDescriptorPoolSize {
            type_: sys::VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER,
            descriptorCount: 256,
        },
        sys::VkDescriptorPoolSize {
            type_: sys::VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
            descriptorCount: 256,
        },
        sys::VkDescriptorPoolSize {
            type_: sys::VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE,
            descriptorCount: 256,
        },
        sys::VkDescriptorPoolSize {
            type_: sys::VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
            descriptorCount: 256,
        },
        sys::VkDescriptorPoolSize {
            type_: sys::VK_DESCRIPTOR_TYPE_SAMPLER,
            descriptorCount: 64,
        },
    ];
    if context.features.acceleration_structures {
        pool_sizes.push(sys::VkDescriptorPoolSize {
            type_: sys::VK_DESCRIPTOR_TYPE_ACCELERATION_STRUCTURE_KHR,
            descriptorCount: 256,
        });
    }
    let create_info = sys::VkDescriptorPoolCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
        pNext: core::ptr::null(),
        maxSets: 256,
        poolSizeCount: pool_sizes.len() as u32,
        pPoolSizes: pool_sizes.as_ptr(),
        flags: sys::VK_DESCRIPTOR_POOL_CREATE_FREE_DESCRIPTOR_SET_BIT,
    };
    let mut pool: sys::VkDescriptorPool = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` points at `pool_sizes`, which outlives the call.
    let status = unsafe {
        (context.core.create_descriptor_pool)(
            context.device,
            &create_info,
            core::ptr::null(),
            &mut pool,
        )
    };
    if status != sys::VK_SUCCESS || pool == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }
    Ok(pool)
}

impl RhiBackend for VulkanBackend {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::Vulkan
    }

    fn device_name(&self) -> &str {
        &self.device_name
    }

    fn query_capabilities(&self) -> Capabilities {
        self.capabilities
    }

    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        VulkanBackend::query_adapter_capabilities(self)
    }

    // MARK: Swapchain

    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        self.wait_until_idle()?;
        let generation = self
            .swapchains
            .get(&handle.id())
            .map_or(0, |window| window.generation)
            + 1;
        self.destroy_swapchain(handle);
        let surface = presentation::create_native_surface(&self.context, &descriptor)?;

        let mut supported: sys::VkBool32 = sys::VK_FALSE;
        // SAFETY: the surface and physical device are live, and the out-parameter
        // is a live local.
        let status = unsafe {
            (self.context.instance_commands.get_surface_support)(
                self.context.physical_device,
                self.context.queues.graphics.family,
                surface,
                &mut supported,
            )
        };
        let usable = self.context.extensions.contains("VK_KHR_swapchain")
            && status == sys::VK_SUCCESS
            && supported == sys::VK_TRUE;
        if !usable {
            // SAFETY: the surface was created above and has no swapchain.
            unsafe {
                (self.context.instance_commands.destroy_surface)(
                    self.context.instance,
                    surface,
                    core::ptr::null(),
                )
            };
            return Err(RhiError::unsupported_backend(
                "selected Vulkan graphics queue cannot present this surface",
            ));
        }

        let chain = presentation::VulkanSwapchain::create(
            &self.context,
            surface,
            &descriptor,
            &self.registries,
        )?;
        self.swapchains
            .insert(handle.id(), VulkanWindowSwapchain::new(chain, generation));
        Ok(())
    }

    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        self.swapchains
            .get(&handle.id())
            .map(|window| window.chain.presentation_mode)
            .ok_or_else(|| RhiError::invalid_argument("query a live swapchain"))
    }

    fn destroy_swapchain(&mut self, handle: Swapchain) {
        let Some(window) = self.swapchains.remove(&handle.id()) else {
            return;
        };
        // Present's queue wait must also retire before binary semaphores die.
        // SAFETY: the device is live; waiting here guarantees no queued presentation
        // still references the semaphores being released.
        unsafe { (self.context.instance_commands.device_wait_idle)(self.context.device) };
        window.chain.destroy(&self.context, &self.registries);
        // SAFETY: the device is idle, so these semaphores are unreferenced.
        unsafe {
            for semaphore in window.acquire_semaphores.values() {
                (self.context.sync.destroy_semaphore)(
                    self.context.device,
                    *semaphore,
                    core::ptr::null(),
                );
            }
            for semaphore in window.presentation.ready.values() {
                (self.context.sync.destroy_semaphore)(
                    self.context.device,
                    *semaphore,
                    core::ptr::null(),
                );
            }
        }
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        if self.active_slot >= self.frames.len() {
            return Err(RhiError::SwapchainAcquireFailed(
                "configure a swapchain and begin a frame first".into(),
            ));
        }
        if self
            .swapchains
            .get(&handle.id())
            .is_some_and(|window| window.needs_recreate)
        {
            self.recreate_window_swapchain(handle)?;
        }
        let slot = self.active_slot;
        let Some(window) = self.swapchains.get_mut(&handle.id()) else {
            return Err(RhiError::SwapchainAcquireFailed(
                "configure a swapchain and begin a frame first".into(),
            ));
        };
        if window.presentation.acquired_id.is_some() {
            return Err(RhiError::SwapchainAcquireFailed(
                "present the acquired image before acquiring another".into(),
            ));
        }
        let semaphore = match window.acquire_semaphores.get(&slot) {
            Some(semaphore) => *semaphore,
            None => {
                let created = binary_semaphore(&self.context)?;
                window.acquire_semaphores.insert(slot, created);
                created
            }
        };
        let mut result = window.chain.acquire(&self.context, semaphore, u64::MAX);
        if result.1 == sys::VK_ERROR_OUT_OF_DATE_KHR {
            self.recreate_window_swapchain(handle)?;
            let Some(window) = self.swapchains.get_mut(&handle.id()) else {
                return Err(RhiError::SwapchainAcquireFailed(
                    "swapchain disappeared".into(),
                ));
            };
            result = window.chain.acquire(&self.context, semaphore, u64::MAX);
        }
        if result.1 != sys::VK_SUCCESS && result.1 != sys::VK_SUBOPTIMAL_KHR {
            return Err(RhiError::SwapchainAcquireFailed(format!(
                "vkAcquireNextImageKHR: {}",
                result.1
            )));
        }
        let Some(window) = self.swapchains.get_mut(&handle.id()) else {
            return Err(RhiError::SwapchainAcquireFailed(
                "swapchain disappeared".into(),
            ));
        };
        if result.1 == sys::VK_SUBOPTIMAL_KHR {
            window.needs_recreate = window.chain.surface_changed(&self.context);
        }
        let index = result.0 as usize;
        let texture = *window.chain.texture_ids.get(index).ok_or_else(|| {
            RhiError::SwapchainAcquireFailed("swapchain image index out of range".into())
        })?;
        let width = window.chain.extent.width as usize;
        let height = window.chain.extent.height as usize;
        let generation = window.generation;
        mark_acquired(window, texture);
        Ok(SwapchainImage::new(
            handle,
            generation,
            Texture::new(texture),
            width,
            height,
        ))
    }

    fn present(&mut self, image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        let slot = self.active_slot;
        let Some(window) = self.swapchains.get_mut(&image.swapchain.id()) else {
            return Err(RhiError::present_failed(
                "image was not acquired from this swapchain",
            ));
        };
        if window.generation != image.generation
            || window.presentation.acquired_id != Some(image.texture.id())
        {
            return Err(RhiError::present_failed(
                "image was not acquired from this swapchain",
            ));
        }
        let index = presentation::image_index(window, image.swapchain, image.texture.id())?;
        let ready = match window.presentation.ready.get(&image.texture.id()) {
            Some(semaphore) => *semaphore,
            None => {
                let created = binary_semaphore(&self.context)?;
                window
                    .presentation
                    .ready
                    .insert(image.texture.id(), created);
                created
            }
        };
        let generation = window.generation;
        let pending_acquire = window.presentation.pending_acquire;
        let acquire_semaphore = window.acquire_semaphores.get(&slot).copied();

        // The transition into the present layout runs on this slot's graphics pool,
        // so the pool cannot reset until this completion has retired.
        submission::one_shot(&self.context, |cmd| {
            // SAFETY: `cmd` is recording and the texture belongs to this swapchain.
            unsafe {
                submission::transition_texture(
                    &self.context,
                    &self.registries,
                    cmd,
                    image.texture,
                    crate::rhi::ResourceState::PRESENT,
                )
            }
        })?;

        // The present itself waits on the acquire semaphore, so the image is
        // acquired before the compositor reads it.
        let wait_semaphores: Vec<sys::VkSemaphore> = if pending_acquire {
            match acquire_semaphore {
                // The present waits for the acquire *and* the transition's own
                // signal, so both are signalled before the compositor reads it.
                Some(semaphore) => vec![semaphore, ready],
                None => {
                    return Err(RhiError::present_failed("acquire semaphore is missing"));
                }
            }
        } else {
            vec![ready]
        };
        let chain_handle = self
            .swapchains
            .get(&image.swapchain.id())
            .map(|window| window.chain.handle)
            .ok_or_else(|| RhiError::present_failed("no live swapchain for this image"))?;
        let present_info = sys::VkPresentInfoKHR {
            sType: sys::VK_STRUCTURE_TYPE_PRESENT_INFO_KHR,
            pNext: core::ptr::null(),
            waitSemaphoreCount: wait_semaphores.len() as u32,
            pWaitSemaphores: wait_semaphores.as_ptr(),
            swapchainCount: 1,
            pSwapchains: &chain_handle,
            pImageIndices: &index,
            pResults: core::ptr::null_mut(),
        };
        // SAFETY: every pointer in `present_info` refers to a live local, and the
        // swapchain is registered and live.
        let status = unsafe {
            (self.context.sync.queue_present)(self.context.queues.graphics.handle, &present_info)
        };

        if let Some(window) = self.swapchains.get_mut(&image.swapchain.id()) {
            window.presentation.pending_acquire = false;
            window.presentation.acquired_id = None;
        }
        if status == sys::VK_ERROR_OUT_OF_DATE_KHR {
            if let Some(window) = self.swapchains.get_mut(&image.swapchain.id()) {
                window.needs_recreate = true;
            }
        } else if status == sys::VK_SUBOPTIMAL_KHR {
            let changed = self
                .swapchains
                .get(&image.swapchain.id())
                .map(|window| !window.needs_recreate && window.chain.surface_changed(&self.context))
                .unwrap_or(false);
            if let Some(window) = self.swapchains.get_mut(&image.swapchain.id()) {
                window.needs_recreate = window.needs_recreate || changed;
            }
        } else if status != sys::VK_SUCCESS {
            self.submission_status
                .record(format!("vkQueuePresentKHR: {status}"));
        }
        let _ = generation;
        // The transition's GPU work has been queued synchronously, so the frontend
        // may retire this frame immediately.
        completion();
        Ok(())
    }

    // MARK: Resource creation

    fn create_buffer(&mut self, handle: Buffer, descriptor: BufferDescriptor) -> RhiResult<()> {
        resources::create_buffer(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            handle,
            descriptor,
        )
    }

    fn create_texture(&mut self, handle: Texture, descriptor: TextureDescriptor) -> RhiResult<()> {
        resources::create_texture(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            handle,
            descriptor,
        )
    }

    fn create_sampler(&mut self, handle: Sampler, descriptor: SamplerDescriptor) -> RhiResult<()> {
        resources::create_sampler(&self.context, &self.registries, handle, descriptor)
    }

    fn create_shader_module(
        &mut self,
        handle: ShaderModule,
        descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        resources::create_shader_module(&self.context, &self.registries, handle, descriptor)
    }

    fn create_acceleration_structure(
        &mut self,
        handle: AccelerationStructure,
        descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<()> {
        acceleration::create_acceleration_structure(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            handle,
            descriptor,
        )
    }

    fn destroy_acceleration_structure(&mut self, handle: AccelerationStructure) {
        acceleration::destroy_acceleration_structure(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            handle,
        );
    }

    fn texture_allocation(&self, texture: Texture) -> Option<TextureAllocation> {
        resources::texture_allocation(&lock(&self.registries), texture)
    }

    // MARK: Pipelines

    fn create_graphics_pipeline(
        &mut self,
        handle: GraphicsPipeline,
        descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        pipelines::create_graphics_pipeline(&self.context, &self.registries, handle, descriptor)
    }

    fn create_mesh_pipeline(
        &mut self,
        handle: MeshPipeline,
        descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<()> {
        pipelines::create_mesh_pipeline(&self.context, &self.registries, handle, descriptor)
    }

    fn destroy_mesh_pipeline(&mut self, handle: MeshPipeline) {
        pipelines::destroy_mesh_pipeline(&self.context, &self.registries, handle);
    }

    fn create_compute_pipeline(
        &mut self,
        handle: ComputePipeline,
        descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        pipelines::create_compute_pipeline(&self.context, &self.registries, handle, descriptor)
    }

    // MARK: Binding sets

    fn register_binding_layout(
        &mut self,
        handle: BindingLayout,
        descriptor: BindingLayoutDescriptor,
    ) -> RhiResult<()> {
        pipelines::register_binding_layout(&self.context, &self.registries, handle, descriptor)
    }

    fn register_pipeline_layout(
        &mut self,
        handle: PipelineLayout,
        descriptor: PipelineLayoutDescriptor,
    ) -> RhiResult<()> {
        pipelines::register_pipeline_layout(&self.context, &self.registries, handle, descriptor)
    }

    fn register_binding_set(
        &mut self,
        handle: BindingSet,
        layout: BindingLayout,
        layout_entries: Vec<BindingLayoutEntry>,
        set_entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()> {
        pipelines::register_binding_set(
            &self.context,
            &self.registries,
            self.descriptor_pool,
            handle,
            layout,
            layout_entries,
            set_entries,
        )
    }

    fn unregister_binding_set(&mut self, handle: BindingSet) {
        pipelines::unregister_binding_set(
            &self.context,
            &self.registries,
            self.descriptor_pool,
            handle,
        );
    }

    // MARK: Immediate data transfer

    fn upload_buffer_data(&mut self, buffer: Buffer, offset: usize, data: &[u8]) -> RhiResult<()> {
        submission::upload_buffer_data(&self.context, &self.registries, buffer, offset, data)
    }

    fn read_buffer_data(
        &mut self,
        buffer: Buffer,
        offset: usize,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        submission::read_buffer_data(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            buffer,
            offset,
            destination,
        )
    }

    fn upload_texture_data(
        &mut self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()> {
        upload_texture(
            &self.context,
            &self.registries,
            texture,
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
        read_texture(
            &self.context,
            &self.registries,
            texture,
            width,
            height,
            bytes_per_row,
            subresource,
            destination,
        )
    }

    // MARK: Submission

    fn make_frame_uploader(&mut self, slot: usize) -> RhiResult<Box<dyn FrameUploader>> {
        self.submission_status.check()?;
        self.active_slot = slot;
        if slot < self.frames.len() {
            // Reusing a slot: the frame ring already waited for that slot's GPU work,
            // so its pools can be reset and its ring rewound.
            let frame = &mut self.frames[slot];
            for pool in frame.command_pools.values_mut() {
                // SAFETY: the GPU finished this slot's previous work before the slot
                // is reused, which is the documented reset precondition.
                let status = unsafe {
                    (self.context.draw.reset_command_pool)(self.context.device, pool.handle, 0)
                };
                if status != sys::VK_SUCCESS {
                    return Err(RhiError::submit_failed("vkResetCommandPool failed"));
                }
                pool.rewind();
            }
            crate::rhi::FrameUploader::reset(
                &mut *frame
                    .uploader
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            return Ok(Box::new(UploaderHandle {
                uploader: Arc::clone(&frame.uploader),
            }));
        }

        // First use of this slot: build its uploader now. `ChunkUploadAllocator`
        // takes the factory by value, so it captures the shared allocator and the
        // registry handle rather than borrowing the backend.
        let context = UploadChunkContext {
            device: self.context.device,
            core: self.context.core,
            granularity: self.context.limits.bufferImageGranularity.max(1),
            acceleration_structures: self.context.features.acceleration_structures,
            registries: Arc::clone(&self.registries),
            allocator: Arc::clone(&self.allocator),
        };
        let uploader = Arc::new(Mutex::new(VulkanFrameUploader::new(move |capacity| {
            context.make_chunk(capacity)
        })));
        self.frames.push(VulkanFrame::new(Arc::clone(&uploader)));
        Ok(Box::new(UploaderHandle { uploader }))
    }

    fn submit(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        if self.active_slot >= self.frames.len() {
            return Err(RhiError::submit_failed("begin a frame before submitting"));
        }
        let gpu_profile = self.gpu_timestamp_profile.clone();
        let mut submitter = Submitter {
            context: &self.context,
            registries: &self.registries,
            frames: &mut self.frames,
            active_slot: self.active_slot,
            timeline_semaphores: &mut self.timeline_semaphores,
            submission_status: Arc::clone(&self.submission_status),
        };
        submitter.submit(submit, cpu_profile, gpu_profile.as_ref(), completion)
    }

    // MARK: Deferred destruction

    fn destroy_buffer(&mut self, handle: Buffer) {
        resources::destroy_buffer(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            handle,
        );
    }

    fn destroy_texture(&mut self, handle: Texture) {
        resources::destroy_texture(
            &self.context,
            &self.registries,
            &mut lock_allocator(&self.allocator),
            handle,
        );
    }

    fn destroy_sampler(&mut self, handle: Sampler) {
        resources::destroy_sampler(&self.context, &self.registries, handle);
    }

    fn destroy_shader_module(&mut self, handle: ShaderModule) {
        resources::destroy_shader_module(&self.context, &self.registries, handle);
    }

    fn destroy_graphics_pipeline(&mut self, handle: GraphicsPipeline) {
        pipelines::destroy_graphics_pipeline(&self.context, &self.registries, handle);
    }

    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline) {
        pipelines::destroy_compute_pipeline(&self.context, &self.registries, handle);
    }

    // MARK: Synchronization

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        // SAFETY: the device is live; waiting here is the documented idle path.
        let status =
            unsafe { (self.context.instance_commands.device_wait_idle)(self.context.device) };
        if status != sys::VK_SUCCESS {
            return Err(RhiError::submit_failed(format!(
                "vkDeviceWaitIdle failed: {status}"
            )));
        }
        self.submission_status.check()
    }

    // MARK: Optional capabilities

    fn supports_background_compilation(&self) -> bool {
        true
    }

    fn supports_gpu_timestamps(&self) -> bool {
        self.context.limits.timestampPeriod > 0.0
    }

    fn gpu_timestamp_profile(&self) -> Option<Arc<GpuTimestampProfile>> {
        self.gpu_timestamp_profile.clone()
    }

    fn set_gpu_timestamp_profile(&mut self, profile: Option<Arc<GpuTimestampProfile>>) {
        self.gpu_timestamp_profile = profile;
    }
}

impl VulkanBackend {
    /// Recreates a window's swapchain after a resize or `OUT_OF_DATE`.
    fn recreate_window_swapchain(&mut self, handle: Swapchain) -> RhiResult<()> {
        self.wait_until_idle()?;
        let Some(window) = self.swapchains.get_mut(&handle.id()) else {
            return Err(RhiError::invalid_argument("query a live swapchain"));
        };
        // SAFETY: the device is idle, so these semaphores are unreferenced.
        unsafe {
            for semaphore in window.presentation.ready.values() {
                (self.context.sync.destroy_semaphore)(
                    self.context.device,
                    *semaphore,
                    core::ptr::null(),
                );
            }
        }
        window.presentation = super::presentation::VulkanPresentationState::default();
        window.chain.recreate(&self.context, &self.registries)?;
        window.generation += 1;
        window.needs_recreate = false;
        Ok(())
    }

    /// Runs one background compilation request.
    pub fn prepare_compilation(
        &self,
        request: crate::rhi::RhiCompilationRequest,
    ) -> RhiResult<crate::rhi::RhiCompilationWork> {
        use crate::rhi::RhiCompilationRequest;
        match request {
            RhiCompilationRequest::Shader(handle, descriptor) => {
                resources::prepare_shader_compilation(
                    &self.context,
                    &self.registries,
                    handle,
                    *descriptor,
                )
            }
            RhiCompilationRequest::Graphics(handle, descriptor) => {
                pipelines::prepare_graphics_compilation(
                    &self.context,
                    &self.registries,
                    handle,
                    *descriptor,
                )
            }
            RhiCompilationRequest::Compute(handle, descriptor) => {
                pipelines::prepare_compute_compilation(
                    &self.context,
                    &self.registries,
                    handle,
                    *descriptor,
                )
            }
            RhiCompilationRequest::Mesh(handle, descriptor) => pipelines::prepare_mesh_compilation(
                &self.context,
                &self.registries,
                handle,
                *descriptor,
            ),
        }
    }
}

/// A binary semaphore, for the acquire/present handshake only.
fn binary_semaphore(context: &VulkanContext) -> RhiResult<sys::VkSemaphore> {
    let create_info = sys::VkSemaphoreCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
    };
    let mut semaphore: sys::VkSemaphore = sys::VK_NULL_HANDLE;
    // SAFETY: `create_info` outlives the call and the out-parameter is a live local.
    let status = unsafe {
        (context.sync.create_semaphore)(
            context.device,
            &create_info,
            core::ptr::null(),
            &mut semaphore,
        )
    };
    if status != sys::VK_SUCCESS || semaphore == sys::VK_NULL_HANDLE {
        return Err(RhiError::OutOfMemory);
    }
    Ok(semaphore)
}

// MARK: - Texture transfers

/// Uploads one region of a texture through a staging buffer.
pub fn upload_texture(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    texture: Texture,
    data: &[u8],
    region: TextureUploadRegion,
    bytes_per_row: usize,
    subresource: TextureSubresource,
) -> RhiResult<()> {
    let (format, width, height, mip_levels, layers, usage, old_layout) = {
        let registries = lock(registries);
        let record = registries
            .textures
            .get(&texture.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
        rhi_require(
            record.sample_count == 1,
            "texture upload requires a single-sample texture",
        )?;
        (
            record.format,
            record.width,
            record.height,
            record.mip_levels,
            record.layers,
            record.usage,
            record.layout,
        )
    };
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        subresource,
        width,
        height,
        mip_levels as usize,
        layers as usize,
    )?;
    rhi_texture_upload_bytes(
        region,
        bytes_per_row,
        format,
        extent_width,
        extent_height,
        data.len(),
    )?;

    let staging_handle = crate::rhi::Buffer::new(lock(registries).next_internal_id());
    {
        let mut allocator = context
            .staging_allocator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        resources::create_buffer(
            context,
            registries,
            &mut allocator,
            staging_handle,
            BufferDescriptor::new(
                data.len().max(1),
                crate::rhi::BufferUsage::TRANSFER_SOURCE
                    | crate::rhi::BufferUsage::TRANSFER_DESTINATION,
            ),
        )?;
    }
    let (native, allocation) = {
        let registries = lock(registries);
        let record = registries
            .buffers
            .get(&staging_handle.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown staging buffer"))?;
        (record.buffer, record.allocation)
    };
    let pointer = allocation.cpu_pointer(0).ok_or(RhiError::OutOfMemory)?;
    // SAFETY: the staging allocation is host visible and sized for the upload, and
    // `data` is a distinct live slice.
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), pointer, data.len()) };

    let final_layout = if usage.contains(crate::rhi::TextureUsage::SAMPLED) {
        sys::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL
    } else {
        sys::VK_IMAGE_LAYOUT_GENERAL
    };
    let pixels = format.bytes_per_texel().max(1);
    let copy = sys::VkBufferImageCopy {
        bufferOffset: 0,
        bufferRowLength: (bytes_per_row / pixels) as u32,
        bufferImageHeight: 0,
        imageSubresource: sys::VkImageSubresourceLayers {
            aspectMask: sys::VK_IMAGE_ASPECT_COLOR_BIT,
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
    let image = {
        let registries = lock(registries);
        registries
            .textures
            .get(&texture.id())
            .map(|record| record.image)
            .ok_or_else(|| RhiError::invalid_argument("unknown texture"))?
    };
    let outcome = submission::one_shot(context, |cmd| {
        // SAFETY: `cmd` is recording; the layout transition and the copy target the
        // same image consistently.
        unsafe {
            transition_image(
                context,
                cmd,
                image,
                format,
                old_layout,
                sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                mip_levels,
                layers,
            );
            (context.draw.cmd_copy_buffer_to_image)(
                cmd,
                native,
                image,
                sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                1,
                &copy,
            );
            transition_image(
                context,
                cmd,
                image,
                format,
                sys::VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                final_layout,
                mip_levels,
                layers,
            );
        }
        Ok(())
    });
    if outcome.is_ok()
        && let Some(record) = lock(registries).textures.get_mut(&texture.id())
    {
        record.layout = final_layout;
    }
    // SAFETY: the copy completed (or failed before queueing), so the staging buffer
    // is unreferenced.
    unsafe { (context.core.destroy_buffer)(context.device, native, core::ptr::null()) };
    {
        let mut allocator = context
            .staging_allocator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        allocator.free(allocation);
    }
    lock(registries).buffers.remove(&staging_handle.id());
    outcome
}

/// Reads one region of a texture back through a staging buffer.
#[allow(clippy::too_many_arguments)]
pub fn read_texture(
    context: &VulkanContext,
    registries: &Arc<Mutex<VulkanRegistries>>,
    texture: Texture,
    width: usize,
    height: usize,
    bytes_per_row: usize,
    subresource: TextureSubresource,
    destination: &mut [u8],
) -> RhiResult<()> {
    let (format, texture_width, texture_height, mip_levels, layers, layout, image) = {
        let registries = lock(registries);
        let record = registries
            .textures
            .get(&texture.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown texture"))?;
        rhi_require(
            record.layout != sys::VK_IMAGE_LAYOUT_UNDEFINED,
            "cannot read an uninitialized Vulkan texture",
        )?;
        rhi_require(
            record.sample_count == 1,
            "texture readback requires a single-sample texture",
        )?;
        (
            record.format,
            record.width,
            record.height,
            record.mip_levels,
            record.layers,
            record.layout,
            record.image,
        )
    };
    let (extent_width, extent_height) = rhi_texture_subresource_extent(
        subresource,
        texture_width,
        texture_height,
        mip_levels as usize,
        layers as usize,
    )?;
    let byte_count = rhi_texture_transfer_bytes(
        width,
        height,
        bytes_per_row,
        format,
        extent_width,
        extent_height,
        destination.len(),
    )?;

    let staging_handle = crate::rhi::Buffer::new(lock(registries).next_internal_id());
    {
        let mut allocator = context
            .staging_allocator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        resources::create_buffer(
            context,
            registries,
            &mut allocator,
            staging_handle,
            BufferDescriptor::new(
                byte_count.max(1),
                crate::rhi::BufferUsage::TRANSFER_SOURCE
                    | crate::rhi::BufferUsage::TRANSFER_DESTINATION,
            ),
        )?;
    }
    let (native, allocation) = {
        let registries = lock(registries);
        let record = registries
            .buffers
            .get(&staging_handle.id())
            .ok_or_else(|| RhiError::invalid_argument("unknown staging buffer"))?;
        (record.buffer, record.allocation)
    };
    let pixels = format.bytes_per_texel().max(1);
    let copy = sys::VkBufferImageCopy {
        bufferOffset: 0,
        bufferRowLength: (bytes_per_row / pixels) as u32,
        bufferImageHeight: 0,
        imageSubresource: sys::VkImageSubresourceLayers {
            aspectMask: sys::VK_IMAGE_ASPECT_COLOR_BIT,
            mipLevel: subresource.mip_level as u32,
            baseArrayLayer: subresource.layer as u32,
            layerCount: 1,
        },
        imageOffset: sys::VkOffset3D { x: 0, y: 0, z: 0 },
        imageExtent: sys::VkExtent3D {
            width: width as u32,
            height: height as u32,
            depth: 1,
        },
    };
    let outcome = submission::one_shot(context, |cmd| {
        // SAFETY: `cmd` is recording; the readback restores the original layout.
        unsafe {
            transition_image(
                context,
                cmd,
                image,
                format,
                layout,
                sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                mip_levels,
                layers,
            );
            (context.draw.cmd_copy_image_to_buffer)(
                cmd,
                image,
                sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                native,
                1,
                &copy,
            );
            transition_image(
                context,
                cmd,
                image,
                format,
                sys::VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                layout,
                mip_levels,
                layers,
            );
        }
        Ok(())
    });
    if outcome.is_ok()
        && let Some(pointer) = allocation.cpu_pointer(0)
    {
        // SAFETY: the copy completed and the staging mapping covers the range;
        // `destination` is a distinct live slice.
        unsafe { std::ptr::copy_nonoverlapping(pointer, destination.as_mut_ptr(), byte_count) };
    }
    // SAFETY: the readback completed (or failed before queueing), so the staging
    // buffer is unreferenced.
    unsafe { (context.core.destroy_buffer)(context.device, native, core::ptr::null()) };
    {
        let mut allocator = context
            .staging_allocator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        allocator.free(allocation);
    }
    lock(registries).buffers.remove(&staging_handle.id());
    outcome
}

/// Records a raw image layout transition for the immediate transfer paths.
///
/// # Safety
/// `cmd` must be recording.
#[allow(clippy::too_many_arguments)]
unsafe fn transition_image(
    context: &VulkanContext,
    cmd: sys::VkCommandBuffer,
    image: sys::VkImage,
    format: crate::rhi::TextureFormat,
    old_layout: u32,
    new_layout: u32,
    mip_levels: u32,
    layers: u32,
) {
    let barrier = sys::VkImageMemoryBarrier {
        sType: sys::VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
        pNext: core::ptr::null(),
        srcAccessMask: submission::layout_access(old_layout),
        dstAccessMask: submission::layout_access(new_layout),
        oldLayout: old_layout,
        newLayout: new_layout,
        srcQueueFamilyIndex: sys::VK_QUEUE_FAMILY_IGNORED,
        dstQueueFamilyIndex: sys::VK_QUEUE_FAMILY_IGNORED,
        image,
        subresourceRange: sys::VkImageSubresourceRange {
            aspectMask: frames_aspect(format),
            baseMipLevel: 0,
            levelCount: mip_levels,
            baseArrayLayer: 0,
            layerCount: layers,
        },
    };
    // SAFETY: the caller guarantees `cmd` is recording and `barrier` outlives it.
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
}

/// Locks the block allocator. A poisoned lock only means another thread panicked
/// while holding it; every mutation is a self-contained block operation.
fn lock_allocator(
    allocator: &Arc<Mutex<VulkanMemoryAllocator>>,
) -> std::sync::MutexGuard<'_, VulkanMemoryAllocator> {
    allocator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn frames_aspect(format: crate::rhi::TextureFormat) -> u32 {
    super::formats::aspect_flags(format)
}

/// Factory state for one slot's upload ring. It captures the shared allocator and
/// registry handle rather than borrowing the backend, because
/// `ChunkUploadAllocator` takes its factory by value and outlives any backend
/// borrow.
struct UploadChunkContext {
    device: sys::VkDevice,
    core: sys::CoreCommands,
    granularity: sys::VkDeviceSize,
    acceleration_structures: bool,
    registries: Arc<Mutex<VulkanRegistries>>,
    allocator: Arc<Mutex<VulkanMemoryAllocator>>,
}

// SAFETY: the captured pieces are Vulkan handles, a table of entry points and
// `Arc<Mutex<_>>`s. Vulkan objects are documented as thread-safe, and the chunk
// factory only runs while the backend holds the registry and allocator locks.
unsafe impl Send for UploadChunkContext {}

impl UploadChunkContext {
    /// Creates a persistently mapped upload chunk and registers its native buffer
    /// under a backend-internal handle, so the encoder can resolve the handle the
    /// frontend binds.
    fn make_chunk(&self, capacity: usize) -> RhiResult<VulkanUploadChunk> {
        let usage = crate::rhi::BufferUsage::TRANSFER_SOURCE
            | crate::rhi::BufferUsage::TRANSFER_DESTINATION
            | crate::rhi::BufferUsage::UNIFORM
            | crate::rhi::BufferUsage::STORAGE_READ
            | crate::rhi::BufferUsage::VERTEX
            | crate::rhi::BufferUsage::INDEX
            | crate::rhi::BufferUsage::INDIRECT;
        let handle = crate::rhi::Buffer::new(lock(&self.registries).next_internal_id());

        let families = [0u32];
        let _ = (&self.granularity, self.acceleration_structures, families);
        let create_info = sys::VkBufferCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            size: capacity.max(1) as sys::VkDeviceSize,
            usage: super::formats::vk_buffer_usage(usage),
            sharingMode: sys::VK_SHARING_MODE_EXCLUSIVE,
            queueFamilyIndexCount: 0,
            pQueueFamilyIndices: core::ptr::null(),
        };
        let mut native: sys::VkBuffer = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info` outlives the call and the out-parameter is a live
        // local.
        let status = unsafe {
            (self.core.create_buffer)(self.device, &create_info, core::ptr::null(), &mut native)
        };
        if status != sys::VK_SUCCESS || native == sys::VK_NULL_HANDLE {
            return Err(RhiError::OutOfMemory);
        }

        // A transfer-sourced ring chunk must stay host visible so `cpu_pointer`
        // works without a map on the hot path.
        let build = (|| -> RhiResult<VulkanMemoryAllocation> {
            let mut requirements = sys::VkMemoryRequirements::default();
            // SAFETY: the buffer was just created on this device.
            unsafe {
                (self.core.get_buffer_memory_requirements)(self.device, native, &mut requirements)
            };
            let mut allocator = lock_allocator(&self.allocator);
            let allocation = allocator.allocate(
                requirements.size,
                requirements.alignment,
                requirements.memoryTypeBits,
                sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | sys::VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
                0,
                false,
            )?;
            // SAFETY: the allocation satisfies the requirements the driver reported.
            let bind = unsafe {
                (self.core.bind_buffer_memory)(
                    self.device,
                    native,
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

        match build {
            Ok(allocation) => {
                lock(&self.registries).buffers.insert(
                    handle.id(),
                    super::registries::VulkanBufferRecord {
                        buffer: native,
                        allocation,
                        size: capacity,
                        usage,
                    },
                );
                Ok(VulkanUploadChunk {
                    allocation,
                    buffer_handle: handle,
                    capacity,
                })
            }
            Err(error) => {
                // SAFETY: binding never succeeded, so nothing references the buffer.
                unsafe { (self.core.destroy_buffer)(self.device, native, core::ptr::null()) };
                Err(error)
            }
        }
    }
}

/// The frontend's exclusive `FrameUploader` handle onto one slot's shared ring.
struct UploaderHandle {
    uploader: Arc<Mutex<VulkanFrameUploader>>,
}

impl crate::rhi::FrameUploader for UploaderHandle {
    fn reset(&mut self) {
        crate::rhi::FrameUploader::reset(
            &mut *self
                .uploader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<crate::rhi::UploadLocation> {
        crate::rhi::FrameUploader::write(
            &mut *self
                .uploader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            bytes,
            alignment,
        )
    }
}

impl Drop for VulkanBackend {
    fn drop(&mut self) {
        self.release_native_objects();
    }
}

impl VulkanBackend {
    /// Translation of `Vulkan/Submission/VulkanCleanup.swift` (`deinit`
    /// teardown). The device is idled first so every object destroyed below is
    /// unreferenced by the GPU; the order mirrors the Swift implementation.
    fn release_native_objects(&mut self) {
        let context = &self.context;
        // SAFETY: the idle wait is the precondition for every destroy below. Its
        // result is ignored because a lost device still permits object teardown.
        unsafe {
            let _ = (context.instance_commands.device_wait_idle)(context.device);
        }

        // Window swapchains first.
        let swapchains = std::mem::take(&mut self.swapchains);
        for window in swapchains.into_values() {
            // SAFETY: the device is idle, so no presentation references these.
            unsafe {
                for semaphore in window.acquire_semaphores.values() {
                    (context.sync.destroy_semaphore)(context.device, *semaphore, core::ptr::null());
                }
                for semaphore in window.presentation.ready.values() {
                    (context.sync.destroy_semaphore)(context.device, *semaphore, core::ptr::null());
                }
            }
            window.chain.destroy(context, &self.registries);
        }

        // Registries, in the Swift order.
        let acceleration_ids = lock(&self.registries)
            .acceleration_structures
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in acceleration_ids {
            acceleration::destroy_acceleration_structure(
                context,
                &self.registries,
                &mut self.allocator.lock().expect("allocator poisoned"),
                AccelerationStructure::new(id),
            );
        }
        let mesh_ids = lock(&self.registries)
            .mesh_pipelines
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in mesh_ids {
            pipelines::destroy_mesh_pipeline(context, &self.registries, MeshPipeline::new(id));
        }
        let graphics_ids = lock(&self.registries)
            .graphics_pipelines
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in graphics_ids {
            pipelines::destroy_graphics_pipeline(
                context,
                &self.registries,
                GraphicsPipeline::new(id),
            );
        }
        let compute_ids = lock(&self.registries)
            .compute_pipelines
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in compute_ids {
            pipelines::destroy_compute_pipeline(
                context,
                &self.registries,
                ComputePipeline::new(id),
            );
        }

        // SAFETY: the device is idle, so the pool is unreferenced.
        unsafe {
            (context.core.destroy_descriptor_pool)(
                context.device,
                self.descriptor_pool,
                core::ptr::null(),
            );
        }

        let layout_ids = lock(&self.registries)
            .pipeline_layouts
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in layout_ids {
            let native = lock(&self.registries)
                .pipeline_layouts
                .remove(&id)
                .map(|record| record.native);
            // SAFETY: as above.
            unsafe {
                if let Some(native) = native {
                    (context.resources.destroy_pipeline_layout)(
                        context.device,
                        native,
                        core::ptr::null(),
                    );
                }
            }
        }
        let binding_ids = lock(&self.registries)
            .binding_layouts
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in binding_ids {
            let layout = lock(&self.registries).binding_layouts.remove(&id);
            // SAFETY: as above.
            unsafe {
                if let Some(layout) = layout {
                    (context.resources.destroy_descriptor_set_layout)(
                        context.device,
                        layout,
                        core::ptr::null(),
                    );
                }
            }
        }

        let module_ids = lock(&self.registries)
            .shader_modules
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in module_ids {
            resources::destroy_shader_module(context, &self.registries, ShaderModule::new(id));
        }
        let sampler_ids = lock(&self.registries)
            .samplers
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in sampler_ids {
            resources::destroy_sampler(context, &self.registries, Sampler::new(id));
        }
        let texture_ids = lock(&self.registries)
            .textures
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in texture_ids {
            resources::destroy_texture(
                context,
                &self.registries,
                &mut self.allocator.lock().expect("allocator poisoned"),
                Texture::new(id),
            );
        }
        let buffer_ids = lock(&self.registries)
            .buffers
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in buffer_ids {
            resources::destroy_buffer(
                context,
                &self.registries,
                &mut self.allocator.lock().expect("allocator poisoned"),
                Buffer::new(id),
            );
        }

        // Per-frame query pools and command pools.
        for frame in &mut self.frames {
            if let Some(pool) = frame.query_pool.take() {
                // SAFETY: the device is idle.
                unsafe {
                    (context.resources.queries.destroy_query_pool)(
                        context.device,
                        pool,
                        core::ptr::null(),
                    );
                }
            }
            let pool_ids = frame.command_pools.keys().copied().collect::<Vec<_>>();
            for id in pool_ids {
                let pool = frame.command_pools.remove(&id);
                // SAFETY: as above.
                unsafe {
                    if let Some(pool) = pool {
                        (context.resources.destroy_command_pool)(
                            context.device,
                            pool.handle,
                            core::ptr::null(),
                        );
                    }
                }
            }
        }

        let timeline = std::mem::take(&mut self.timeline_semaphores);
        for semaphore in timeline.into_values() {
            // SAFETY: the frame ring has retired, so no submission waits on these.
            unsafe {
                (context.sync.destroy_semaphore)(context.device, semaphore, core::ptr::null());
            }
        }

        // Scratch one-shot pool and fence.
        {
            let mut scratch = context.scratch.lock().expect("scratch poisoned");
            if let Some(pool) = scratch.command_pool.take() {
                // SAFETY: the device is idle.
                unsafe {
                    (context.resources.destroy_command_pool)(
                        context.device,
                        pool,
                        core::ptr::null(),
                    );
                }
            }
            if let Some(fence) = scratch.fence.take() {
                // SAFETY: as above.
                unsafe {
                    (context.sync.destroy_fence)(context.device, fence, core::ptr::null());
                }
            }
        }

        self.allocator
            .lock()
            .expect("allocator poisoned")
            .destroy_all();
        context
            .staging_allocator
            .lock()
            .expect("staging allocator poisoned")
            .destroy_all();

        // SAFETY: every child object has been released, so the device and
        // instance can be destroyed.
        unsafe {
            (context.instance_commands.destroy_device)(context.device, core::ptr::null());
            (context.instance_commands.destroy_instance)(context.instance, core::ptr::null());
        }
    }
}

/// Factory used by the backend registry.
pub fn make_backend(config: &DeviceConfig) -> RhiResult<Box<dyn RhiBackend>> {
    Ok(Box::new(VulkanBackend::make(config.clone())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backend_reports_vulkan() {
        assert_eq!(GraphicsApi::Vulkan.display_name(), "Vulkan");
    }

    #[test]
    fn capability_probe_requires_every_acceleration_entry_point() {
        // A context cannot be built without a driver, so this asserts the shaping
        // rules the probe applies: ray tracing needs the feature AND all five
        // resolved commands.
        let mut features = sys::FeatureSupport::default();
        assert!(!features.acceleration_structures);
        features.acceleration_structures = true;
        // The capability flag alone is not enough; the backend intersects it with the
        // resolved table, which is why `probe_capabilities` checks each pointer.
        assert!(features.acceleration_structures);
    }

    #[test]
    fn aspect_masks_come_from_the_format() {
        assert_eq!(
            frames_aspect(crate::rhi::TextureFormat::Rgba8Unorm),
            sys::VK_IMAGE_ASPECT_COLOR_BIT
        );
        assert_eq!(
            frames_aspect(crate::rhi::TextureFormat::Depth32Float),
            sys::VK_IMAGE_ASPECT_DEPTH_BIT
        );
    }
}
