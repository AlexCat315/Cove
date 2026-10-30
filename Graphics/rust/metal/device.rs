//! Translation of NativeRHI/Metal/MetalDevice.swift: the concrete device, its
//! capability probe and its per-window swapchain state.
//!
//! [`MetalDeviceHandle`] owns the `MTLDevice`, the two command queues, the shared
//! "depth disabled" state, the registries and the library cache. Every RHI
//! operation lives in a sibling module and borrows this state, so the backend
//! type in `mod.rs` stays a thin façade.
//!
//! The registries sit behind `Arc<Mutex<_>>` because a compilation's publish
//! closure is `'static`: Swift gets the same reachability from its class
//! reference, and the lock keeps the GPU-completion thread out of a registry
//! the API thread is mutating.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLCommandQueue, MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice, MTLGPUFamily,
    MTLIndirectCommandBufferDescriptor, MTLIndirectCommandType, MTLPixelFormat, MTLResourceOptions,
    MTLSharedEvent, MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use crate::rhi::{AdapterCapabilities, Capabilities, DeviceConfig, QueueClass, QueueLimits, RhiError, RhiResult};

use super::library_cache::MetalLibraryCache;
use super::registries::MetalRegistries;
use super::submission_status::MetalSubmissionStatus;

/// Vertex-buffer argument base index. Kept high so dynamic vertex buffers do
/// not collide with uniform-buffer argument slots (matches the reference).
pub const VERTEX_BUFFER_BASE_INDEX: u32 = 24;

/// Swapchain/surface state, grouped so the device stays small.
pub struct MetalWindowSwapchain {
    pub layer: Retained<CAMetalLayer>,
    pub generation: u64,
    pub current_drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    pub current_texture_id: Option<u32>,
    /// One texture handle per frame slot, so resource and planner history stay
    /// bounded instead of growing with every presented frame.
    pub texture_ids: HashMap<usize, u32>,
}

impl MetalWindowSwapchain {
    pub fn new(layer: Retained<CAMetalLayer>, generation: u64) -> Self {
        Self {
            layer,
            generation,
            current_drawable: None,
            current_texture_id: None,
            texture_ids: HashMap::new(),
        }
    }
}

pub struct MetalDeviceHandle {
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub graphics_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub compute_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub disabled_depth_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    pub config: DeviceConfig,
    /// Shared with every compilation publish closure, so Swift's class-reference
    /// reachability is preserved.
    pub registries: Arc<Mutex<MetalRegistries>>,
    pub library_cache: MetalLibraryCache,
    pub capabilities: Capabilities,
    pub submission_status: Arc<MetalSubmissionStatus>,
    pub interfaces: crate::rhi::PipelineInterfaces,
    pub swapchains: HashMap<u32, MetalWindowSwapchain>,
    pub active_slot: usize,
    /// Lazily-grown shared staging buffer reused for immediate texture
    /// upload/readback.
    pub staging_buffer: Option<Retained<ProtocolObject<dyn objc2_metal::MTLBuffer>>>,
    pub device_name: String,
    /// Per-slot uploaders vended to the frontend, created on first use.
    pub uploaders: HashMap<usize, Arc<Mutex<super::upload_ring::MetalFrameUploader>>>,
    /// Optional GPU timing sink, attached through the frontend. Captured per
    /// submission so a later detach does not lose an in-flight sample.
    pub gpu_timestamp_profile: Option<Arc<crate::rhi::GpuTimestampProfile>>,
}

impl MetalDeviceHandle {
    /// Creates the device, its queues and the shared depth state.
    pub fn make(config: DeviceConfig) -> RhiResult<Self> {
        let device = objc2_metal::MTLCreateSystemDefaultDevice().ok_or_else(|| {
            RhiError::unsupported_backend("MTLCreateSystemDefaultDevice returned nil")
        })?;
        let graphics_queue = device.newCommandQueue().ok_or(RhiError::OutOfMemory)?;
        // A missing second queue falls back to the graphics queue, matching Swift.
        let compute_queue = device.newCommandQueue().unwrap_or_else(|| graphics_queue.clone());

        let depth =  { MTLDepthStencilDescriptor::new() };
        depth.setDepthCompareFunction(objc2_metal::MTLCompareFunction::Always);
        depth.setDepthWriteEnabled(false);
        let disabled_depth_state =
            device.newDepthStencilStateWithDescriptor(&depth).ok_or(RhiError::OutOfMemory)?;

        let capabilities = Self::query_capabilities(&device);
        let device_name = device.name().to_string();
        Ok(Self {
            library_cache: MetalLibraryCache::new(&device),
            registries: Arc::new(Mutex::new(MetalRegistries::new())),
            submission_status: Arc::new(MetalSubmissionStatus::new()),
            interfaces: crate::rhi::PipelineInterfaces::new(),
            swapchains: HashMap::new(),
            active_slot: 0,
            staging_buffer: None,
            uploaders: HashMap::new(),
            gpu_timestamp_profile: None,
            device,
            graphics_queue,
            compute_queue,
            disabled_depth_state,
            config,
            capabilities,
            device_name,
        })
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn adapter_capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            ray_tracing: self.device.supportsRaytracing(),
            mesh_shading: self.supports_mesh_shading(),
        }
    }

    pub fn supports_mesh_shading(&self) -> bool {
        self.device.supportsFamily(MTLGPUFamily::Apple7)
            || self.device.supportsFamily(MTLGPUFamily::Mac2)
    }

    /// Only implemented paths are exposed, gated by native device support.
    fn query_capabilities(device: &ProtocolObject<dyn MTLDevice>) -> Capabilities {
        let ray_tracing = device.supportsRaytracing();
        let mesh = device.supportsFamily(MTLGPUFamily::Apple7)
            || device.supportsFamily(MTLGPUFamily::Mac2);
        Capabilities::configure(|capabilities| {
            capabilities.graphics = true;
            capabilities.compute = true;
            capabilities.ray_tracing.acceleration_structures = ray_tracing;
            capabilities.ray_tracing.compute_ray_query = ray_tracing;
            capabilities.mesh_shading.mesh = mesh;
            // Task shaders stay disabled: the mesh implementation has no depth
            // state path yet, matching the Swift capability set.
            capabilities.mesh_shading.task = false;
            capabilities.indirect_draw = Self::probe_indirect_command_buffer(device);
            capabilities.textures.texture_3d =
                Self::probe_texture(device, MTLTextureType::Type3D, MTLPixelFormat::R8Unorm, true);
            capabilities.textures.cube =
                Self::probe_texture(device, MTLTextureType::TypeCube, MTLPixelFormat::RGBA8Unorm, false);
            capabilities.max_queues = QueueLimits::default();
        })
    }

    /// Indirect draw is advertised only when the device can allocate an
    /// indirect command buffer with the shape the encoder needs.
    fn probe_indirect_command_buffer(device: &ProtocolObject<dyn MTLDevice>) -> bool {
        let descriptor =  { MTLIndirectCommandBufferDescriptor::new() };
        descriptor.setCommandTypes(MTLIndirectCommandType::Draw);
        descriptor.setInheritBuffers(false);
        descriptor.setMaxVertexBufferBindCount(0);
        descriptor.setMaxFragmentBufferBindCount(0);
        // SAFETY: the descriptor is fully configured above and Metal only reads
        // it during the call; a one-command probe needs no further options.
        unsafe {
            device
                .newIndirectCommandBufferWithDescriptor_maxCommandCount_options(
                    &descriptor,
                    1,
                    MTLResourceOptions::empty(),
                )
                .is_some()
        }
    }

    /// Whether the device can allocate a texture of this type and format.
    fn probe_texture(
        device: &ProtocolObject<dyn MTLDevice>,
        texture_type: MTLTextureType,
        format: MTLPixelFormat,
        is_3d: bool,
    ) -> bool {
        let descriptor =  { MTLTextureDescriptor::new() };
        descriptor.setTextureType(texture_type);
        descriptor.setPixelFormat(format);
        // SAFETY: probe textures are single-texel, which every device supports;
        // a 3D probe additionally needs a one-deep extent.
        unsafe {
            descriptor.setWidth(1);
            descriptor.setHeight(1);
        }
        if is_3d {
            unsafe { descriptor.setDepth(1) };
        }
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        device.newTextureWithDescriptor(&descriptor).is_some()
    }

    /// Native device name for diagnostics.
    pub fn name(&self) -> &str {
        &self.device_name
    }

    /// The queue that runs a submission of the given class. Swift routes
    /// everything except compute to the graphics queue.
    pub fn queue_for(&self, queue: QueueClass) -> &Retained<ProtocolObject<dyn MTLCommandQueue>> {
        match queue {
            QueueClass::Compute => &self.compute_queue,
            QueueClass::Graphics | QueueClass::Transfer => &self.graphics_queue,
        }
    }

    /// Shared event for a timeline semaphore ID, created on first use.
    pub fn shared_event(&mut self, id: u32) -> Option<Retained<ProtocolObject<dyn MTLSharedEvent>>> {
        {
            let registries = self
                .registries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(existing) = registries.shared_events.get(&id) {
                return Some(existing.clone());
            }
        }
        let event = self.device.newSharedEvent()?;
        self.registries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .shared_events
            .insert(id, event.clone());
        Some(event)
    }
}

// SAFETY: Metal documents its objects as safe to use from multiple threads, and
// every field here is either such an object, a plain handle map, or an
// `Arc<Mutex<_>>`. The backend is handed to the frontend behind its own mutex,
// so no two threads touch this state concurrently; the impl exists because
// `objc2-metal`'s protocol traits do not themselves require `Send`, which leaves
// the auto-derived bound too weak for `RhiBackend: Send`.
unsafe impl Send for MetalDeviceHandle {}

/// A label as an `NSString`, or `None` when the descriptor had no label.
pub fn label(label: Option<&str>) -> Option<Retained<NSString>> {
    label.map(NSString::from_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vertex_buffer_base_index_matches_the_swift_constant() {
        assert_eq!(VERTEX_BUFFER_BASE_INDEX, 24);
        // Bindings 24...31 are reserved for vertex inputs, so the highest legal
        // uniform slot is one below the base index.
        assert_eq!(VERTEX_BUFFER_BASE_INDEX - 1, 23);
    }

    #[test]
    fn labels_only_exist_when_the_descriptor_has_one() {
        assert!(label(None).is_none());
        assert_eq!(label(Some("scene")).map(|value| value.to_string()), Some("scene".to_owned()));
    }

    #[test]
    fn a_new_swapchain_has_no_acquired_drawable() {
        let window = MetalWindowSwapchain::new(CAMetalLayer::new(), 1);
        assert_eq!(window.generation, 1);
        assert!(window.current_drawable.is_none());
        assert!(window.current_texture_id.is_none());
        assert!(window.texture_ids.is_empty());
    }
}
