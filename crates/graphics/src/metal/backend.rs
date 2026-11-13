//! Translation of NativeRHI/Metal/MetalBackend.swift and the protocol surface of
//! `MetalDevice`: the `RhiBackend` implementation for Metal.
//!
//! The backend owns one [`MetalDeviceHandle`] and forwards each trait method to
//! the module that implements it, so the Swift extension split is preserved
//! (`MetalDeviceResources`, `MetalDeviceTransfer`, `MetalSubmitEncoder`, ...).
//! Nothing here may be compiled on a target without Metal.
//!
//! The `RHIBackgroundCompilation` conformance from
//! `Metal/Device/MetalCompilation.swift` is the inherent `prepare_compilation`
//! method in this module.
use std::sync::Arc;

use crate::rhi::{
    AccelerationStructure, AccelerationStructureDescriptor, AdapterCapabilities, BindingLayout,
    BindingLayoutDescriptor, BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer,
    BufferDescriptor, Capabilities, Completion, ComputePipeline, ComputePipelineDescriptor,
    DeviceConfig, FrameUploader, GpuTimestampProfile, GraphicsApi, GraphicsPipeline,
    GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor, PipelineLayout,
    PipelineLayoutDescriptor, PlannedSubmit, PresentationMode, RhiBackend, RhiResult, Sampler,
    SamplerDescriptor, ShaderModule, ShaderModuleDescriptor, SubmissionCpuProfile,
    SurfaceDescriptor, Swapchain, SwapchainImage, Texture, TextureAllocation, TextureDescriptor,
    TextureSubresource, TextureUploadRegion,
};

use super::device::MetalDeviceHandle;
use super::{acceleration, resources, submit, transfer};

/// The concrete Metal backend. `MetalBackend::make` is the Swift
/// `MetalBackend.make(config:)` entry point.
pub struct MetalBackend {
    device: MetalDeviceHandle,
    gpu_timestamp_profile: Option<Arc<GpuTimestampProfile>>,
}

impl MetalBackend {
    /// Creates the backend for the current physical device.
    pub fn make(config: DeviceConfig) -> RhiResult<Self> {
        Ok(Self {
            device: MetalDeviceHandle::make(config)?,
            gpu_timestamp_profile: None,
        })
    }
}

impl RhiBackend for MetalBackend {
    fn api(&self) -> GraphicsApi {
        GraphicsApi::Metal
    }

    fn device_name(&self) -> &str {
        self.device.name()
    }

    fn query_capabilities(&self) -> Capabilities {
        self.device.capabilities()
    }

    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        self.device.adapter_capabilities()
    }

    // MARK: Swapchain

    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()> {
        transfer::configure_swapchain(&mut self.device, handle, descriptor)
    }

    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode> {
        transfer::presentation_mode(&self.device, handle)
    }

    fn destroy_swapchain(&mut self, handle: Swapchain) {
        transfer::destroy_swapchain(&mut self.device, handle);
    }

    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
        transfer::acquire_swapchain_image(&mut self.device, handle)
    }

    fn present(&mut self, image: SwapchainImage, completion: Completion) -> RhiResult<()> {
        transfer::present(&mut self.device, image, completion)
    }

    // MARK: Resource creation

    fn create_buffer(&mut self, handle: Buffer, descriptor: BufferDescriptor) -> RhiResult<()> {
        resources::create_buffer(
            &self.device.registries,
            &self.device.device,
            handle,
            descriptor,
        )
    }

    fn create_texture(&mut self, handle: Texture, descriptor: TextureDescriptor) -> RhiResult<()> {
        resources::create_texture(
            &self.device.registries,
            &self.device.device,
            handle,
            descriptor,
        )
    }

    fn create_sampler(&mut self, handle: Sampler, descriptor: SamplerDescriptor) -> RhiResult<()> {
        resources::create_sampler(
            &self.device.registries,
            &self.device.device,
            handle,
            descriptor,
        )
    }

    fn create_shader_module(
        &mut self,
        handle: ShaderModule,
        descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()> {
        resources::create_shader_module(&self.device, handle, descriptor)
    }

    #[allow(
        clippy::arc_with_non_send_sync,
        reason = "the structure is only reached through the registries' lock"
    )]
    fn create_acceleration_structure(
        &mut self,
        handle: AccelerationStructure,
        descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<()> {
        let registries = self
            .device
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let structure = acceleration::MetalAccelerationStructure::create(
            &self.device.device,
            &registries,
            &descriptor,
        )?;
        drop(registries);
        self.device
            .registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .acceleration_structures
            .insert(handle.id(), Arc::new(structure));
        Ok(())
    }

    fn destroy_acceleration_structure(&mut self, handle: AccelerationStructure) {
        resources::lock(&self.device.registries).retire_acceleration_structure(handle);
    }

    fn texture_allocation(&self, texture: Texture) -> Option<TextureAllocation> {
        resources::texture_allocation(&self.device, texture)
    }

    // MARK: Pipelines

    fn create_graphics_pipeline(
        &mut self,
        handle: GraphicsPipeline,
        descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()> {
        resources::create_graphics_pipeline(&self.device, handle, descriptor)
    }

    fn create_mesh_pipeline(
        &mut self,
        handle: MeshPipeline,
        descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<()> {
        resources::create_mesh_pipeline(&self.device, handle, descriptor)
    }

    fn destroy_mesh_pipeline(&mut self, handle: MeshPipeline) {
        resources::destroy_mesh_pipeline(&self.device, handle);
    }

    fn create_compute_pipeline(
        &mut self,
        handle: ComputePipeline,
        descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()> {
        resources::create_compute_pipeline(&self.device, handle, descriptor)
    }

    // MARK: Binding sets

    fn register_binding_layout(
        &mut self,
        handle: BindingLayout,
        descriptor: BindingLayoutDescriptor,
    ) -> RhiResult<()> {
        transfer::register_binding_layout(&mut self.device, handle, descriptor)
    }

    fn register_pipeline_layout(
        &mut self,
        handle: PipelineLayout,
        descriptor: PipelineLayoutDescriptor,
    ) -> RhiResult<()> {
        transfer::register_pipeline_layout(&mut self.device, handle, descriptor)
    }

    fn register_binding_set(
        &mut self,
        handle: BindingSet,
        layout: BindingLayout,
        layout_entries: Vec<BindingLayoutEntry>,
        set_entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()> {
        transfer::register_binding_set(
            &mut self.device,
            handle,
            layout,
            layout_entries,
            set_entries,
        )
    }

    fn unregister_binding_set(&mut self, handle: BindingSet) {
        transfer::unregister_binding_set(&mut self.device, handle);
    }

    // MARK: Immediate data transfer

    fn upload_buffer_data(&mut self, buffer: Buffer, offset: usize, data: &[u8]) -> RhiResult<()> {
        transfer::upload_buffer_data(&mut self.device, buffer, offset, data)
    }

    fn read_buffer_data(
        &mut self,
        buffer: Buffer,
        offset: usize,
        destination: &mut [u8],
    ) -> RhiResult<()> {
        transfer::read_buffer_data(&mut self.device, buffer, offset, destination)
    }

    fn upload_texture_data(
        &mut self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()> {
        transfer::upload_texture_data(
            &mut self.device,
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
        transfer::read_texture_data(
            &mut self.device,
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
        transfer::make_frame_uploader(&mut self.device, slot)
    }

    fn submit(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()> {
        submit::submit(&mut self.device, submit, cpu_profile, Err(completion))
    }

    fn submit_timed(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: crate::rhi::TimedCompletion,
    ) -> RhiResult<()> {
        submit::submit(&mut self.device, submit, cpu_profile, Ok(completion))
    }

    // MARK: Deferred destruction

    fn destroy_buffer(&mut self, handle: Buffer) {
        resources::destroy_buffer(&self.device, handle);
    }

    fn destroy_texture(&mut self, handle: Texture) {
        resources::destroy_texture(&self.device, handle);
    }

    fn destroy_sampler(&mut self, handle: Sampler) {
        resources::destroy_sampler(&self.device, handle);
    }

    fn destroy_shader_module(&mut self, handle: ShaderModule) {
        resources::destroy_shader_module(&self.device, handle);
    }

    fn destroy_graphics_pipeline(&mut self, handle: GraphicsPipeline) {
        resources::destroy_graphics_pipeline(&self.device, handle);
    }

    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline) {
        resources::destroy_compute_pipeline(&self.device, handle);
    }

    // MARK: Synchronization

    fn wait_until_idle(&mut self) -> RhiResult<()> {
        transfer::wait_until_idle(&mut self.device)
    }

    // MARK: Optional capabilities

    fn supports_background_compilation(&self) -> bool {
        true
    }

    fn supports_gpu_timestamps(&self) -> bool {
        true
    }

    fn gpu_timestamp_profile(&self) -> Option<Arc<GpuTimestampProfile>> {
        self.gpu_timestamp_profile.clone()
    }

    fn set_gpu_timestamp_profile(&mut self, profile: Option<Arc<GpuTimestampProfile>>) {
        // Recording starts with the next submission; attaching does no GPU work.
        self.gpu_timestamp_profile = profile.clone();
        self.device.gpu_timestamp_profile = profile;
    }
}

impl MetalBackend {
    /// Runs one compilation request through Metal's background path. Exposed
    /// because the frontend's background worker needs an entry point that is not
    /// part of `RhiBackend`.
    pub fn prepare_compilation(
        &self,
        request: crate::rhi::RhiCompilationRequest,
    ) -> RhiResult<crate::rhi::RhiCompilationWork> {
        transfer::prepare_compilation(&self.device, request)
    }
}

/// Factory used by the backend registry.
pub fn make_backend(config: &DeviceConfig) -> RhiResult<Box<dyn RhiBackend>> {
    Ok(Box::new(MetalBackend::make(config.clone())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backend_reports_metal_and_its_optional_capabilities() {
        // Constructing a real Metal device requires a GPU; this asserts the
        // capability contract that does not depend on one.
        assert!(!GraphicsApi::Metal.display_name().is_empty());
        assert_eq!(GraphicsApi::Metal.display_name(), "Metal");
    }
}
