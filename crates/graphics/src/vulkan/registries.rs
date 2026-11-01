//! Translation of NativeRHI/Vulkan/Resources/{VulkanResources,VulkanMesh,
//! VulkanAcceleration}.swift: the concrete resource records and the registry that
//! maps frontend handles to Vulkan objects.
//!
//! Kept separate from the backend so the device type stays small (one
//! responsibility per type), exactly as Swift split it.
use std::collections::HashMap;

use crate::rhi::{
    Buffer, BufferUsage, ComputePipelineDescriptor, GraphicsPipelineDescriptor,
    MeshPipelineDescriptor, PushConstantRange, ShaderSpecializationConstant, ShaderStage,
    TextureDimension, TextureFormat, TextureUsage,
};

use super::memory::VulkanMemoryAllocation;
use super::sys;

pub struct VulkanBufferRecord {
    pub buffer: sys::VkBuffer,
    pub allocation: VulkanMemoryAllocation,
    pub size: usize,
    pub usage: BufferUsage,
}

pub struct VulkanTextureRecord {
    pub image: sys::VkImage,
    pub allocation: Option<VulkanMemoryAllocation>,
    pub view: sys::VkImageView,
    pub format: TextureFormat,
    pub width: usize,
    pub height: usize,
    pub dimension: TextureDimension,
    pub layers: u32,
    pub mip_levels: u32,
    pub usage: TextureUsage,
    /// True for images acquired from the swapchain (memory owned by Vulkan).
    pub is_swapchain: bool,
    /// Current layout, tracked so barriers can be emitted.
    pub layout: u32,
    pub sample_count: usize,
    pub transient_attachment: bool,
    /// First-mip view used for attachment binding when the texture has mips.
    pub attachment_view: Option<sys::VkImageView>,
}

pub struct VulkanSamplerRecord {
    pub sampler: sys::VkSampler,
}

pub struct VulkanShaderModuleRecord {
    pub module: sys::VkShaderModule,
    pub stage: ShaderStage,
    pub entry_point: String,
    pub specialization_constants: Vec<ShaderSpecializationConstant>,
}

pub struct VulkanGraphicsPipelineRecord {
    pub pipeline: sys::VkPipeline,
    pub descriptor: GraphicsPipelineDescriptor,
}

pub struct VulkanComputePipelineRecord {
    pub pipeline: sys::VkPipeline,
    pub descriptor: ComputePipelineDescriptor,
}

pub struct VulkanMeshPipelineRecord {
    pub pipeline: sys::VkPipeline,
    pub descriptor: MeshPipelineDescriptor,
}

pub struct VulkanBindingSetRecord {
    pub descriptor_set: sys::VkDescriptorSet,
    pub layout_id: u32,
    pub dynamic_offset_count: u32,
}

/// One push-constant declaration plus the byte offset its range starts at.
/// Each shader stage's single SPIR-V push block begins at byte zero, and
/// overlapping ranges with disjoint stage masks are legal in Vulkan.
pub struct VulkanPushRange {
    pub declaration: PushConstantRange,
    pub offset: u32,
}

pub struct VulkanPipelineLayoutRecord {
    pub native: sys::VkPipelineLayout,
    pub descriptor: crate::rhi::PipelineLayoutDescriptor,
    pub push_ranges: Vec<VulkanPushRange>,
}

pub struct VulkanAccelerationRecord {
    pub native: sys::VkAccelerationStructureKHR,
    pub result: Buffer,
    pub scratch: Buffer,
    pub instance_buffer: Option<Buffer>,
    pub scratch_address: sys::VkDeviceAddress,
    pub geometry: Vec<sys::VkAccelerationStructureGeometryKHR>,
    pub ranges: Vec<sys::VkAccelerationStructureBuildRangeInfoKHR>,
    pub top_level: bool,
}

/// The backend's own handle space.
#[derive(Default)]
pub struct VulkanRegistries {
    pub buffers: HashMap<u32, VulkanBufferRecord>,
    pub textures: HashMap<u32, VulkanTextureRecord>,
    pub samplers: HashMap<u32, VulkanSamplerRecord>,
    pub shader_modules: HashMap<u32, VulkanShaderModuleRecord>,
    pub graphics_pipelines: HashMap<u32, VulkanGraphicsPipelineRecord>,
    pub compute_pipelines: HashMap<u32, VulkanComputePipelineRecord>,
    pub binding_sets: HashMap<u32, VulkanBindingSetRecord>,
    pub binding_layouts: HashMap<u32, sys::VkDescriptorSetLayout>,
    pub pipeline_layouts: HashMap<u32, VulkanPipelineLayoutRecord>,
    pub mesh_pipelines: HashMap<u32, VulkanMeshPipelineRecord>,
    pub acceleration_structures: HashMap<u32, VulkanAccelerationRecord>,
    /// Internal-handle counter for backend-owned resources (upload chunks,
    /// swapchain images); starts above the frontend's low IDs.
    internal_id: u32,
}

// SAFETY: the records are driver handles plus plain data. Vulkan objects may be
// used from any thread under external synchronization, which the owning `Mutex`
// provides.
unsafe impl Send for VulkanRegistries {}

/// First backend-internal handle. Frontend handles grow from 1, so the two spaces
/// never overlap for any realistic session.
pub const INTERNAL_ID_BASE: u32 = 0x8000_0000;

impl VulkanRegistries {
    pub fn new() -> Self {
        Self {
            internal_id: INTERNAL_ID_BASE,
            ..Self::default()
        }
    }

    pub fn next_internal_id(&mut self) -> u32 {
        let id = self.internal_id;
        self.internal_id = self.internal_id.wrapping_add(1);
        id
    }

    /// A texture's first-mip view, which is what an attachment binds.
    pub fn attachment_view(&self, texture: &VulkanTextureRecord) -> sys::VkImageView {
        texture.attachment_view.unwrap_or(texture.view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_ids_start_above_the_frontend_space() {
        let mut registries = VulkanRegistries::new();
        assert_eq!(registries.next_internal_id(), INTERNAL_ID_BASE);
        assert_eq!(registries.next_internal_id(), INTERNAL_ID_BASE + 1);
    }

    #[test]
    fn an_attachment_prefers_the_first_mip_view() {
        let texture = VulkanTextureRecord {
            image: 1,
            allocation: None,
            view: 10,
            format: TextureFormat::Rgba8Unorm,
            width: 4,
            height: 4,
            dimension: TextureDimension::Texture2D,
            layers: 1,
            mip_levels: 4,
            usage: TextureUsage::COLOR_TARGET,
            is_swapchain: false,
            layout: sys::VK_IMAGE_LAYOUT_UNDEFINED,
            sample_count: 1,
            transient_attachment: false,
            attachment_view: Some(11),
        };
        let registries = VulkanRegistries::new();
        assert_eq!(registries.attachment_view(&texture), 11);
        let single_mip = VulkanTextureRecord {
            attachment_view: None,
            ..texture
        };
        assert_eq!(registries.attachment_view(&single_mip), 10);
    }
}
