//! Translation of NativeRHI/RHIBackend.swift: the backend trait (the vtable
//! equivalent).
//!
//! A concrete backend (Metal / Vulkan / DX12) owns the GPU device and the
//! concrete resource objects. The frontend `Device` owns handle allocation,
//! binding/pipeline-layout caches, and the submission planner; it hands each
//! newly allocated handle to the backend, which stores the concrete object under
//! that handle. All backend failures are returned — nothing is silently
//! truncated.
//!
//! Destruction methods are invoked by the `FrameRing` *after* the frame in which
//! the resource was released has completed on the GPU, so they never free an
//! object that is still in use.
//!
//! Swift's protocol-extension defaults become default trait methods, so a
//! backend that cannot implement an optional operation inherits the
//! `unsupportedFeature` error rather than silently no-op'ing.
use std::sync::Arc;

use super::{
    AccelerationStructure, AccelerationStructureDescriptor, AdapterCapabilities, BindingLayout,
    BindingLayoutDescriptor, BindingLayoutEntry, BindingSet, BindingSetEntry, Buffer,
    BufferDescriptor, Capabilities, ComputePipeline, ComputePipelineDescriptor, FrameUploader,
    GpuTimestampProfile, GraphicsApi, GraphicsPipeline, GraphicsPipelineDescriptor, MeshPipeline,
    MeshPipelineDescriptor, PipelineLayout, PipelineLayoutDescriptor, PlannedSubmit,
    PresentationMode, RhiError, RhiResult, Sampler, SamplerDescriptor, ShaderModule,
    ShaderModuleDescriptor, SubmissionCpuProfile, SurfaceDescriptor, Swapchain, SwapchainImage,
    Texture, TextureAllocation, TextureDescriptor, TextureSubresource, TextureUploadRegion,
};

/// Completion callback run once the GPU has retired the work.
pub type Completion = Box<dyn FnOnce() + Send + 'static>;

pub trait RhiBackend: Send {
    /// The graphics API this backend implements.
    fn api(&self) -> GraphicsApi;
    /// Human-readable physical device / adapter name.
    fn device_name(&self) -> &str;

    /// Implemented features intersected with device support. An advertised
    /// extension alone must never enable an unimplemented RHI operation.
    fn query_capabilities(&self) -> Capabilities;

    /// Diagnostic information about the adapter, not the implemented RHI API.
    fn query_adapter_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::default()
    }

    // MARK: Swapchain

    /// Creates or resizes one window's swapchain. The frontend waits for idle
    /// before resizing; the platform handle must outlive deferred destruction.
    fn configure_swapchain(
        &mut self,
        handle: Swapchain,
        descriptor: SurfaceDescriptor,
    ) -> RhiResult<()>;
    fn presentation_mode(&self, handle: Swapchain) -> RhiResult<PresentationMode>;
    fn destroy_swapchain(&mut self, handle: Swapchain);

    /// Acquires the next presentable image for `handle`.
    fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage>;
    /// Completion includes the final GPU transition/presentation command.
    /// Returns an error only before queuing work; queued failures reach device
    /// status.
    fn present(&mut self, image: SwapchainImage, completion: Completion) -> RhiResult<()>;

    // MARK: Resource creation

    fn create_buffer(&mut self, handle: Buffer, descriptor: BufferDescriptor) -> RhiResult<()>;
    fn create_texture(&mut self, handle: Texture, descriptor: TextureDescriptor) -> RhiResult<()>;
    fn create_sampler(&mut self, handle: Sampler, descriptor: SamplerDescriptor) -> RhiResult<()>;
    fn create_shader_module(
        &mut self,
        handle: ShaderModule,
        descriptor: ShaderModuleDescriptor,
    ) -> RhiResult<()>;

    fn create_acceleration_structure(
        &mut self,
        _handle: AccelerationStructure,
        _descriptor: AccelerationStructureDescriptor,
    ) -> RhiResult<()> {
        Err(RhiError::unsupported_feature(
            "acceleration structures are not implemented by this backend",
        ))
    }
    fn destroy_acceleration_structure(&mut self, _handle: AccelerationStructure) {}

    /// Backend-reported allocation for one texture, or `None` when the backend
    /// cannot report it. Missing support must stay unknown, rather than zero.
    fn texture_allocation(&self, _texture: Texture) -> Option<TextureAllocation> {
        None
    }

    // MARK: Pipelines

    fn create_graphics_pipeline(
        &mut self,
        handle: GraphicsPipeline,
        descriptor: GraphicsPipelineDescriptor,
    ) -> RhiResult<()>;
    fn create_mesh_pipeline(
        &mut self,
        _handle: MeshPipeline,
        _descriptor: MeshPipelineDescriptor,
    ) -> RhiResult<()> {
        Err(RhiError::unsupported_feature(
            "mesh pipelines are not implemented by this backend",
        ))
    }
    fn destroy_mesh_pipeline(&mut self, _handle: MeshPipeline) {}
    fn create_compute_pipeline(
        &mut self,
        handle: ComputePipeline,
        descriptor: ComputePipelineDescriptor,
    ) -> RhiResult<()>;

    // MARK: Binding sets

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

    /// Maps a binding-set handle to the concrete GPU resources it references.
    /// Must return an error (never silently no-op) if backend allocation fails.
    fn register_binding_set(
        &mut self,
        handle: BindingSet,
        layout: BindingLayout,
        layout_entries: Vec<BindingLayoutEntry>,
        set_entries: Vec<BindingSetEntry>,
    ) -> RhiResult<()>;
    fn unregister_binding_set(&mut self, handle: BindingSet);

    // MARK: Immediate data transfer (host-visible / shared storage)

    fn upload_buffer_data(&mut self, buffer: Buffer, offset: usize, data: &[u8]) -> RhiResult<()>;

    /// Synchronous readback of previously submitted buffer contents.
    fn read_buffer_data(
        &mut self,
        _buffer: Buffer,
        _offset: usize,
        _destination: &mut [u8],
    ) -> RhiResult<()> {
        Err(RhiError::unsupported_feature(
            "buffer readback is not implemented by this backend",
        ))
    }

    fn upload_texture_data(
        &mut self,
        texture: Texture,
        data: &[u8],
        region: TextureUploadRegion,
        bytes_per_row: usize,
        subresource: TextureSubresource,
    ) -> RhiResult<()>;

    fn read_texture_data(
        &mut self,
        texture: Texture,
        width: usize,
        height: usize,
        bytes_per_row: usize,
        subresource: TextureSubresource,
        destination: &mut [u8],
    ) -> RhiResult<()>;

    // MARK: Submission

    /// Vends (and resets) the per-slot transient uploader for an upcoming frame.
    fn make_frame_uploader(&mut self, slot: usize) -> RhiResult<Box<dyn FrameUploader>>;

    /// Executes one planned submission. Must be asynchronous: it returns once
    /// the work is queued and invokes `completion` when the GPU finishes. No
    /// `waitUntilCompleted` / `vkQueueWaitIdle` on the hot path.
    /// Optional CPU profiling must never wait for GPU completion.
    fn submit(
        &mut self,
        submit: &PlannedSubmit,
        cpu_profile: Option<&mut SubmissionCpuProfile>,
        completion: Completion,
    ) -> RhiResult<()>;

    // MARK: Deferred destruction (called by FrameRing)

    fn destroy_buffer(&mut self, handle: Buffer);
    fn destroy_texture(&mut self, handle: Texture);
    fn destroy_sampler(&mut self, handle: Sampler);
    fn destroy_shader_module(&mut self, handle: ShaderModule);
    fn destroy_graphics_pipeline(&mut self, handle: GraphicsPipeline);
    fn destroy_compute_pipeline(&mut self, handle: ComputePipeline);

    // MARK: Synchronization

    /// Blocks until all previously queued GPU work has completed.
    fn wait_until_idle(&mut self) -> RhiResult<()>;

    // MARK: Optional capabilities

    /// Whether this backend implements the snapshot/compile/publish split that
    /// makes non-blocking resource preparation possible.
    fn supports_background_compilation(&self) -> bool {
        false
    }

    /// Whether this backend can timestamp GPU execution for queued submissions.
    fn supports_gpu_timestamps(&self) -> bool {
        false
    }

    // MARK: Optional timing

    /// The profile receiving GPU timestamps, when this backend supports them.
    fn gpu_timestamp_profile(&self) -> Option<Arc<GpuTimestampProfile>> {
        None
    }

    /// Attaches a GPU timestamp profile. Setting it does no GPU work on its
    /// own; recording starts with the next submission.
    fn set_gpu_timestamp_profile(&mut self, _profile: Option<Arc<GpuTimestampProfile>>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{TextureFormat, TextureUsage};

    /// A backend that implements only what it can and inherits every default.
    struct MinimalBackend;

    impl RhiBackend for MinimalBackend {
        fn api(&self) -> GraphicsApi {
            GraphicsApi::Vulkan
        }
        fn device_name(&self) -> &str {
            "test adapter"
        }
        fn query_capabilities(&self) -> Capabilities {
            Capabilities::default()
        }
        fn configure_swapchain(
            &mut self,
            _handle: Swapchain,
            _descriptor: SurfaceDescriptor,
        ) -> RhiResult<()> {
            Ok(())
        }
        fn presentation_mode(&self, _handle: Swapchain) -> RhiResult<PresentationMode> {
            Ok(PresentationMode::Fifo)
        }
        fn destroy_swapchain(&mut self, _handle: Swapchain) {}
        fn acquire_swapchain_image(&mut self, handle: Swapchain) -> RhiResult<SwapchainImage> {
            Ok(SwapchainImage::new(handle, 1, Texture::new(1), 1, 1))
        }
        fn present(&mut self, _image: SwapchainImage, completion: Completion) -> RhiResult<()> {
            completion();
            Ok(())
        }
        fn create_buffer(
            &mut self,
            _handle: Buffer,
            _descriptor: BufferDescriptor,
        ) -> RhiResult<()> {
            Ok(())
        }
        fn create_texture(
            &mut self,
            _handle: Texture,
            _descriptor: TextureDescriptor,
        ) -> RhiResult<()> {
            Ok(())
        }
        fn create_sampler(
            &mut self,
            _handle: Sampler,
            _descriptor: SamplerDescriptor,
        ) -> RhiResult<()> {
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
            Err(RhiError::unsupported_feature("no uploader"))
        }
        fn submit(
            &mut self,
            _submit: &PlannedSubmit,
            _cpu_profile: Option<&mut SubmissionCpuProfile>,
            completion: Completion,
        ) -> RhiResult<()> {
            completion();
            Ok(())
        }
        fn destroy_buffer(&mut self, _handle: Buffer) {}
        fn destroy_texture(&mut self, _handle: Texture) {}
        fn destroy_sampler(&mut self, _handle: Sampler) {}
        fn destroy_shader_module(&mut self, _handle: ShaderModule) {}
        fn destroy_graphics_pipeline(&mut self, _handle: GraphicsPipeline) {}
        fn destroy_compute_pipeline(&mut self, _handle: ComputePipeline) {}
        fn wait_until_idle(&mut self) -> RhiResult<()> {
            Ok(())
        }
    }

    #[test]
    fn unsupported_operations_inherit_an_error_instead_of_silently_succeeding() {
        let mut backend = MinimalBackend;
        assert!(
            backend
                .read_buffer_data(Buffer::new(1), 0, &mut [])
                .is_err()
        );
        assert!(
            backend
                .create_acceleration_structure(
                    AccelerationStructure::new(1),
                    AccelerationStructureDescriptor::BottomLevel(Vec::new()),
                )
                .is_err()
        );
        assert!(backend.texture_allocation(Texture::new(1)).is_none());
        assert!(backend.gpu_timestamp_profile().is_none());
        assert_eq!(
            backend.query_adapter_capabilities(),
            AdapterCapabilities::default()
        );
        // Binding-layout registration is a no-op that still reports success.
        assert!(
            backend
                .register_binding_layout(BindingLayout::new(1), BindingLayoutDescriptor::default())
                .is_ok()
        );
        assert!(backend.wait_until_idle().is_ok());
        assert_eq!(
            TextureDescriptor::new(4, 4, TextureFormat::Rgba8Unorm, TextureUsage::SAMPLED).width,
            4
        );
    }
}
