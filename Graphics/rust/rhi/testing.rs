//! A minimal in-process backend used by the frontend unit tests.
//!
//! It implements every required [`RhiBackend`] method with the cheapest honest
//! behaviour — allocate nothing, succeed — so the frontend's handle allocation,
//! validation, frame ring and deferred destruction can be exercised without a
//! GPU. It deliberately inherits every optional default, which also proves that
//! a backend can omit an operation instead of silently faking it.
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    AccelerationStructure, AdapterCapabilities, BindingLayout, BindingLayoutDescriptor,
    BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer, BufferDescriptor, Capabilities,
    ComputePipeline, ComputePipelineDescriptor, FrameUploader, GraphicsApi, GraphicsPipeline,
    GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor, PipelineLayout,
    PipelineLayoutDescriptor, PlannedSubmit, PresentationMode, RhiBackend, RhiError, RhiResult,
    Sampler, SamplerDescriptor, ShaderModule, ShaderModuleDescriptor, SubmissionCpuProfile,
    SurfaceDescriptor, Swapchain, SwapchainImage, Texture, TextureDescriptor, TextureSubresource,
    TextureUploadRegion, UploadLocation,
};

/// Counters a test can assert on.
#[derive(Debug, Default)]
pub struct BackendCounters {
    pub created_buffers: AtomicUsize,
    pub destroyed_buffers: AtomicUsize,
    pub configured_swapchains: AtomicUsize,
    pub destroyed_swapchains: AtomicUsize,
    pub submissions: AtomicUsize,
    pub uploaders: AtomicUsize,
}

#[derive(Debug, Default)]
pub struct NullBackend {
    pub counters: BackendCounters,
    next_texture: AtomicUsize,
}

impl NullBackend {
    pub fn new() -> Self {
        Self { counters: BackendCounters::default(), next_texture: AtomicUsize::new(1) }
    }
}

struct NullUploader {
    storage: Vec<u8>,
    cursor: usize,
}

impl FrameUploader for NullUploader {
    fn reset(&mut self) {
        self.cursor = 0;
    }

    fn write(&mut self, bytes: &[u8], alignment: usize) -> RhiResult<UploadLocation> {
        let alignment = alignment.max(1);
        let start = self.cursor.div_ceil(alignment) * alignment;
        let end = start + bytes.len();
        if end > self.storage.len() {
            return Err(RhiError::OutOfMemory);
        }
        self.storage[start..end].copy_from_slice(bytes);
        self.cursor = end;
        Ok(UploadLocation::new(Buffer::new(1), start))
    }
}

impl RhiBackend for NullBackend {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::Vulkan
    }

    fn device_name(&self) -> &str {
        "null adapter"
    }

    fn query_capabilities(&self) -> Capabilities {
        Capabilities::configure(|capabilities| {
            capabilities.graphics = true;
            capabilities.compute = true;
            capabilities.indirect_draw = true;
            capabilities.max_queues.graphics = 1;
            capabilities.max_queues.compute = 1;
            capabilities.max_queues.transfer = 1;
        })
    }

    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::default()
    }

    fn configure_swapchain(
        &mut self,
        _handle: Swapchain,
        _descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        self.counters.configured_swapchains.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn presentation_mode(&self, _handle: Swapchain) -> RhiResult<PresentationMode> {
        Ok(PresentationMode::Fifo)
    }

    fn destroy_swapchain(&mut self, _handle: Swapchain) {
        self.counters.destroyed_swapchains.fetch_add(1, Ordering::SeqCst);
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        let texture = Texture::new(self.next_texture.fetch_add(1, Ordering::SeqCst) as u32);
        Ok(SwapchainImage::new(handle, 1, texture, 64, 64))
    }

    fn present(&mut self, _image: SwapchainImage, completion: super::Completion) -> RhiResult<()> {
        completion();
        Ok(())
    }

    fn create_buffer(&mut self, _handle: Buffer, _descriptor: BufferDescriptor) -> RhiResult<()> {
        self.counters.created_buffers.fetch_add(1, Ordering::SeqCst);
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
        _handle: ShaderModule,
        _descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn create_graphics_pipeline(
        &mut self,
        _handle: GraphicsPipeline,
        _descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn create_compute_pipeline(
        &mut self,
        _handle: ComputePipeline,
        _descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn create_mesh_pipeline(
        &mut self,
        _handle: MeshPipeline,
        _descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn register_binding_layout(
        &mut self,
        _handle: BindingLayout,
        _descriptor: BindingLayoutDescriptor,
    ) -> RhiResult<()> {
        Ok(())
    }

    fn register_pipeline_layout(
        &mut self,
        _handle: PipelineLayout,
        _descriptor: PipelineLayoutDescriptor,
    ) -> RhiResult<()> {
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
        self.counters.uploaders.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(NullUploader { storage: vec![0; 64 * 1024], cursor: 0 }))
    }

    fn submit(
        &mut self,
        _submit: &PlannedSubmit,
        _cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: super::Completion,
    ) -> RhiResult<()> {
        self.counters.submissions.fetch_add(1, Ordering::SeqCst);
        completion();
        Ok(())
    }

    fn destroy_buffer(&mut self, _handle: Buffer) {
        self.counters.destroyed_buffers.fetch_add(1, Ordering::SeqCst);
    }

    fn destroy_texture(&mut self, _handle: Texture) {}
    fn destroy_sampler(&mut self, _handle: Sampler) {}
    fn destroy_shader_module(&mut self, _handle: ShaderModule) {}
    fn destroy_graphics_pipeline(&mut self, _handle: GraphicsPipeline) {}
    fn destroy_compute_pipeline(&mut self, _handle: ComputePipeline) {}
    fn destroy_acceleration_structure(&mut self, _handle: AccelerationStructure) {}

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{Device, DeviceConfig};

    #[test]
    fn the_null_backend_supports_a_full_frame() {
        let device = Device::new(Box::new(NullBackend::new()), DeviceConfig::default())
            .expect("device");
        assert_eq!(device.device_name(), "null adapter");
        let buffer = device.make_buffer(BufferDescriptor::new(64, super::super::BufferUsage::VERTEX)).expect("buffer");
        device.begin_frame().expect("begin");
        let location = device.upload_transient(&[1, 2, 3, 4], 4).expect("upload");
        assert_eq!(location.offset, 0);
        let commands = crate::rhi::CommandBuffer::new();
        device
            .submit(&commands, crate::rhi::QueueClass::Graphics, &[], &[], None)
            .expect("submit");
        device.end_frame();
        device.destroy_buffer(buffer);
        device.wait_until_idle().expect("idle");
    }
}
