//! Shared recording backend for the NativeRHI integration tests.
//!
//! The Swift suites define a local `RHIBackend` subclass per file to observe
//! interning, refcounting and presentation without a GPU. Rust's integration
//! tests are separate crates, so the equivalent lives here and each suite
//! configures the behaviour it needs through [`RecordingState`].
//!
//! Every required [`RhiBackend`] method has the cheapest honest behaviour and
//! every optional one inherits its default, so the tests also prove that a
//! backend can omit an operation instead of silently faking it.

use std::sync::{Arc, Mutex, MutexGuard};

use cove_graphics::{
    AccelerationStructure, AdapterCapabilities, BindingLayout, BindingLayoutDescriptor,
    BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer, BufferDescriptor, Capabilities,
    Completion, ComputePipeline, ComputePipelineDescriptor, Device, DeviceConfig, FrameUploader,
    GraphicsApi, GraphicsPipeline, GraphicsPipelineDescriptor, GraphicsPipelineKey,
    MeshPipeline, MeshPipelineDescriptor, MeshPipelineKey, PipelineLayout, PipelineLayoutDescriptor,
    PlannedSubmit, PresentationMode, RhiBackend, RhiError, RhiResult, Sampler, SamplerDescriptor,
    ShaderModule, ShaderModuleDescriptor, ShaderModuleKey, SubmissionCpuProfile, SurfaceDescriptor,
    Swapchain, SwapchainImage, Texture, TextureDescriptor, TextureSubresource, TextureUploadRegion,
    UploadLocation,
};

/// Everything a test can assert on after driving the frontend device.
#[derive(Default)]
pub struct RecordingState {
    pub created_shader_modules: Vec<ShaderModule>,
    pub destroyed_shader_modules: Vec<ShaderModule>,
    pub created_graphics: Vec<GraphicsPipeline>,
    pub destroyed_graphics: Vec<GraphicsPipeline>,
    pub created_compute: Vec<ComputePipeline>,
    pub destroyed_compute: Vec<ComputePipeline>,
    pub created_mesh: Vec<MeshPipeline>,
    pub destroyed_mesh: Vec<MeshPipeline>,
    /// Live window handles and the descriptor last configured for each.
    pub windows: Vec<(Swapchain, SurfaceDescriptor)>,
    /// Presentation completions held back so a test can release them by hand.
    pub pending_presentations: Vec<Completion>,
    pub submissions: usize,
    /// Fail the next `wait_until_idle`, as a completed GPU error would.
    pub reject_next_idle: bool,
    /// Fail the next `present` before it queues anything.
    pub reject_next_present: bool,
    /// Fail the next swapchain acquire.
    pub reject_next_acquire: bool,
}

impl RecordingState {
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn pending_count(&self) -> usize {
        self.pending_presentations.len()
    }

    fn descriptor_for(&self, handle: Swapchain) -> Option<SurfaceDescriptor> {
        self.windows
            .iter()
            .find(|(window, _)| *window == handle)
            .map(|(_, descriptor)| *descriptor)
    }
}

/// A backend that records instead of rendering.
pub struct RecordingBackend {
    state: Arc<Mutex<RecordingState>>,
    capabilities: Capabilities,
    /// Stand-in textures handed out by `acquire_swapchain_image`.
    next_texture: Mutex<u32>,
}

impl RecordingBackend {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState::default())),
            capabilities,
            next_texture: Mutex::new(0x8000_0000),
        }
    }

    /// Capabilities with graphics and compute, which is what most suites need.
    pub fn standard() -> Self {
        Self::new(Capabilities::configure(|capabilities| {
            capabilities.graphics = true;
            capabilities.compute = true;
        }))
    }

    /// Also advertises mesh shading, so mesh pipelines are reachable.
    pub fn with_mesh_shading() -> Self {
        Self::new(Capabilities::configure(|capabilities| {
            capabilities.graphics = true;
            capabilities.compute = true;
            capabilities.mesh_shading.mesh = true;
        }))
    }

    pub fn state(&self) -> Arc<Mutex<RecordingState>> {
        Arc::clone(&self.state)
    }

    pub fn lock(&self) -> MutexGuard<'_, RecordingState> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Releases every held presentation completion, as GPU retirement would.
pub fn complete_presentations(state: &Arc<Mutex<RecordingState>>) {
    let bodies: Vec<Completion> = {
        let mut guard = state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut guard.pending_presentations)
    };
    for body in bodies {
        body();
    }
}

pub fn reject_next_idle(state: &Arc<Mutex<RecordingState>>) {
    state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).reject_next_idle = true;
}

pub fn reject_next_present(state: &Arc<Mutex<RecordingState>>) {
    state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).reject_next_present = true;
}

/// Builds a device over a recording backend.
pub fn make_device(backend: &RecordingBackend, frames_in_flight: usize) -> Device {
    Device::new(
        Box::new(RecordingBackend {
            state: backend.state(),
            capabilities: backend.capabilities,
            next_texture: Mutex::new(0x8000_0000),
        }),
        DeviceConfig { frames_in_flight, ..Default::default() },
    )
    .expect("device")
}

/// An uploader that refuses work, so upload-ring paths are never silently taken.
struct RefusingUploader;

impl FrameUploader for RefusingUploader {
    fn reset(&mut self) {}

    fn write(&mut self, _bytes: &[u8], _alignment: usize) -> RhiResult<UploadLocation> {
        Err(RhiError::OutOfMemory)
    }
}

impl RhiBackend for RecordingBackend {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::Metal
    }

    fn device_name(&self) -> &str {
        "recording backend"
    }

    fn query_capabilities(&self) -> Capabilities {
        self.capabilities
    }

    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::default()
    }

    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        let mut state = self.lock();
        match state.windows.iter_mut().find(|(window, _)| *window == handle) {
            Some(entry) => entry.1 = descriptor,
            None => state.windows.push((handle, descriptor)),
        }
        Ok(())
    }

    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        let state = self.lock();
        let descriptor = state
            .descriptor_for(handle)
            .ok_or_else(|| RhiError::invalid_argument("unknown swapchain"))?;
        Ok(if descriptor.vsync_enabled { PresentationMode::Fifo } else { PresentationMode::Immediate })
    }

    fn destroy_swapchain(&mut self, handle: Swapchain) {
        let mut state = self.lock();
        state.windows.retain(|(window, _)| *window != handle);
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        let mut state = self.lock();
        if state.reject_next_acquire {
            state.reject_next_acquire = false;
            return Err(RhiError::SwapchainAcquireFailed("acquire rejected by the test".into()));
        }
        let descriptor = state
            .descriptor_for(handle)
            .ok_or_else(|| RhiError::invalid_argument("unknown swapchain"))?;
        let mut next = self.next_texture.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let texture = Texture::new(*next);
        *next += 1;
        Ok(SwapchainImage::new(
            handle,
            1,
            texture,
            descriptor.width,
            descriptor.height,
        ))
    }

    fn present(&mut self, _image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        let mut state = self.lock();
        if state.reject_next_present {
            state.reject_next_present = false;
            return Err(RhiError::present_failed("before queueing"));
        }
        state.pending_presentations.push(completion);
        Ok(())
    }

    fn create_buffer(&mut self, _handle: Buffer, _descriptor: BufferDescriptor) -> RhiResult<()> {
        Ok(())
    }

    fn create_texture(&mut self, _handle: Texture, _descriptor: TextureDescriptor) -> RhiResult<()> {
        Ok(())
    }

    fn create_sampler(&mut self, _handle: Sampler, _descriptor: SamplerDescriptor) -> RhiResult<()> {
        Ok(())
    }

    fn create_shader_module(
        &mut self,
        handle: ShaderModule,
        _descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        self.lock().created_shader_modules.push(handle);
        Ok(())
    }

    fn create_graphics_pipeline(
        &mut self,
        handle: GraphicsPipeline,
        _descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        self.lock().created_graphics.push(handle);
        Ok(())
    }

    fn create_compute_pipeline(
        &mut self,
        handle: ComputePipeline,
        _descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        self.lock().created_compute.push(handle);
        Ok(())
    }

    fn create_mesh_pipeline(
        &mut self,
        handle: MeshPipeline,
        _descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<()> {
        self.lock().created_mesh.push(handle);
        Ok(())
    }

    fn register_binding_set(
        &mut self,
        _handle: BindingSet,
        _layout: BindingLayout,
        _layout_entries: Vec<BindingLayoutEntry>,
        _set_entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn unregister_binding_set(&mut self, _handle: BindingSet) {}

    fn upload_buffer_data(
        &mut self,
        _buffer: Buffer,
        _offset: usize,
        _data: &[u8],
    ) -> RhiResult<()> {
        Ok(())
    }

    fn upload_texture_data(
        &mut self,
        _texture: Texture,
        _data: &[u8],
        _region: TextureUploadRegion,
        _bytes_per_row: usize,
        _subresource: TextureSubresource,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn read_texture_data(
        &mut self,
        _texture: Texture,
        _width: usize,
        _height: usize,
        _bytes_per_row: usize,
        _subresource: TextureSubresource,
        _destination: &mut [u8],
    ) -> RhiResult<()> {
        Ok(())
    }

    fn make_frame_uploader(&mut self, _slot: usize) -> RhiResult<Box<dyn FrameUploader>> {
        Ok(Box::new(RefusingUploader))
    }

    fn submit(
        &mut self,
        _submit: &PlannedSubmit,
        _cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        self.lock().submissions += 1;
        completion();
        Ok(())
    }

    fn destroy_buffer(&mut self, _handle: Buffer) {}

    fn destroy_texture(&mut self, _handle: Texture) {}

    fn destroy_sampler(&mut self, _handle: Sampler) {}

    fn destroy_shader_module(&mut self, handle: ShaderModule) {
        self.lock().destroyed_shader_modules.push(handle);
    }

    fn destroy_graphics_pipeline(&mut self, handle: GraphicsPipeline) {
        self.lock().destroyed_graphics.push(handle);
    }

    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline) {
        self.lock().destroyed_compute.push(handle);
    }

    fn destroy_mesh_pipeline(&mut self, handle: MeshPipeline) {
        self.lock().destroyed_mesh.push(handle);
    }

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        let mut state = self.lock();
        if state.reject_next_idle {
            state.reject_next_idle = false;
            return Err(RhiError::submit_failed("completed GPU work reported an error"));
        }
        Ok(())
    }

    fn destroy_acceleration_structure(&mut self, _handle: AccelerationStructure) {}
}

/// A vertex shader descriptor for the pipeline-cache suites.
pub fn shader_descriptor(stage: cove_graphics::ShaderStage, code: &[u8]) -> ShaderModuleDescriptor {
    ShaderModuleDescriptor::new(stage, cove_graphics::ShaderFormat::MslSource, code.to_vec())
}

/// Keys stay reachable so a suite can assert interning directly.
pub fn graphics_key(descriptor: &GraphicsPipelineDescriptor) -> GraphicsPipelineKey {
    GraphicsPipelineKey::new(descriptor)
}

pub fn shader_key(descriptor: &ShaderModuleDescriptor) -> ShaderModuleKey {
    ShaderModuleKey::new(descriptor)
}

pub fn mesh_key(descriptor: &MeshPipelineDescriptor) -> MeshPipelineKey {
    MeshPipelineKey::new(descriptor)
}

/// A layout with no set layouts, the shape every suite starts from.
pub fn empty_layout(device: &Device) -> PipelineLayout {
    device
        .make_pipeline_layout(PipelineLayoutDescriptor::new(Vec::new()))
        .expect("pipeline layout")
}

pub fn empty_layout_descriptor() -> BindingLayoutDescriptor {
    BindingLayoutDescriptor::default()
}
