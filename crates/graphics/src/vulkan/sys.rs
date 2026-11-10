//! Handwritten Vulkan FFI: the types, constants, entry-point table and native
//! loader the backend needs, with no external binding crate.
//!
//! Swift reached Vulkan through the pinned `CVulkanHeaders` module and a small C
//! shim (`grhi_vulkan_get_instance_proc_addr`, `grhi_vulkan_pack_instance`,
//! `grhi_vulkan_create_native_surface`). The Rust port replaces that shim with
//! this module: the loader opens the platform driver library at runtime, the
//! handle/struct layouts are declared here, and surface creation is written per
//! platform. Nothing is linked at build time, so a host without a Vulkan driver
//! simply reports "unsupported backend" instead of failing to load.
//!
//! Every struct below is `#[repr(C)]` and mirrors the Vulkan C declaration
//! field-for-field, in order, because the driver reads them by layout.
//!
//! Translation of `Vulkan/VulkanFunctions.swift`: the pointer interop helpers
//! (`vkNull`, `vkWithOutHandle`, `vkCString`, `vkExtName`), every PFN signature
//! and the grouped command tables are the null-handle constants, `CStr`
//! helpers, PFN types and command structs in this module.
//!
//! Translation of `Vulkan/VulkanAdvanced.swift`: the optional entry points and
//! scratch alignment it resolves are [`AdvancedCommands`] in this module.
// The declarations mirror the Vulkan headers, so constants and entry points the
// backend does not call yet stay declared.
#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]

use core::ffi::{c_char, c_void};

// MARK: - Scalar aliases

pub type VkResult = i32;
pub type VkFlags = u32;
pub type VkBool32 = u32;
pub type VkDeviceSize = u64;
pub type VkDeviceAddress = u64;
pub type VkSampleMask = u32;

pub const VK_TRUE: VkBool32 = 1;
pub const VK_FALSE: VkBool32 = 0;

pub const VK_SUCCESS: VkResult = 0;
pub const VK_NOT_READY: VkResult = 1;
pub const VK_TIMEOUT: VkResult = 2;
pub const VK_INCOMPLETE: VkResult = 5;
pub const VK_SUBOPTIMAL_KHR: VkResult = 1_000_001_003;
pub const VK_ERROR_OUT_OF_DATE_KHR: VkResult = -1_000_001_004;
pub const VK_ERROR_INITIALIZATION_FAILED: VkResult = -3;
pub const VK_ERROR_EXTENSION_NOT_PRESENT: VkResult = -7;

pub const VK_WHOLE_SIZE: VkDeviceSize = !0;
pub const VK_REMAINING_MIP_LEVELS: u32 = !0;
pub const VK_REMAINING_ARRAY_LAYERS: u32 = !0;
pub const VK_LOD_CLAMP_NONE: f32 = 1000.0;
pub const VK_QUEUE_FAMILY_IGNORED: u32 = !0;
pub const VK_MAX_EXTENSION_NAME_SIZE: usize = 256;
pub const VK_MAX_DESCRIPTION_SIZE: usize = 256;
pub const VK_MAX_DEVICE_NAME_SIZE: usize = 256;
pub const VK_UUID_SIZE: usize = 16;
pub const VK_MAX_MEMORY_TYPES: usize = 32;
pub const VK_MAX_MEMORY_HEAPS: usize = 16;

// MARK: - Handles
//
// Vulkan handles are either dispatchable (pointer to an internal struct) or
// non-dispatchable (a 64-bit integer, or a pointer on 32-bit hosts). Modelling
// them as `*mut c_void` is ABI-correct for every handle the backend touches and
// keeps null-checking explicit.

pub type VkInstance = *mut c_void;
pub type VkPhysicalDevice = *mut c_void;
pub type VkDevice = *mut c_void;
pub type VkQueue = *mut c_void;
pub type VkCommandBuffer = *mut c_void;
pub type VkBuffer = u64;
pub type VkImage = u64;
pub type VkDeviceMemory = u64;
pub type VkImageView = u64;
pub type VkSampler = u64;
pub type VkShaderModule = u64;
pub type VkDescriptorSetLayout = u64;
pub type VkDescriptorPool = u64;
pub type VkDescriptorSet = u64;
pub type VkPipelineLayout = u64;
pub type VkPipeline = u64;
pub type VkRenderPass = u64;
pub type VkFramebuffer = u64;
pub type VkCommandPool = u64;
pub type VkFence = u64;
pub type VkSemaphore = u64;
pub type VkEvent = u64;
pub type VkQueryPool = u64;
pub type VkSurfaceKHR = u64;
pub type VkSwapchainKHR = u64;
pub type VkAccelerationStructureKHR = u64;
pub type VkDeferredOperationKHR = u64;
pub type VkPipelineCache = u64;
pub type VkAllocationCallbacks = c_void;

pub const VK_NULL_HANDLE: u64 = 0;
pub const VK_NULL_INSTANCE: VkInstance = core::ptr::null_mut();

// MARK: - Enumerations

pub const VK_STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
pub const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
pub const VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO: u32 = 2;
pub const VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO: u32 = 3;
pub const VK_STRUCTURE_TYPE_SUBMIT_INFO: u32 = 4;
pub const VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO: u32 = 5;
pub const VK_STRUCTURE_TYPE_FENCE_CREATE_INFO: u32 = 8;
pub const VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO: u32 = 9;
pub const VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO: u32 = 11;
pub const VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO: u32 = 12;
pub const VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO: u32 = 14;
pub const VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO: u32 = 15;
pub const VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO: u32 = 16;
pub const VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO: u32 = 31;
// MARK: - Pipeline structure types (Vulkan core numbering)
pub const VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO: u32 = 18;
pub const VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO: u32 = 19;
pub const VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO: u32 = 20;
pub const VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO: u32 = 22;
pub const VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO: u32 = 23;
pub const VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO: u32 = 24;
pub const VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO: u32 = 26;
pub const VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO: u32 = 25;
pub const VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO: u32 = 28;
pub const VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO: u32 = 29;
pub const VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO: u32 = 27;
pub const VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO: u32 = 30;
pub const VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO: u32 = 38;
pub const VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO: u32 = 32;
pub const VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO: u32 = 33;
pub const VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO: u32 = 34;
pub const VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET: u32 = 35;
pub const VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO: u32 = 37;
pub const VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO: u32 = 39;
pub const VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO: u32 = 40;
pub const VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO: u32 = 42;
pub const VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO: u32 = 43;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2: u32 = 1000059000;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2: u32 = 1000059001;
pub const VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO: u32 = 1000207002;
pub const VK_STRUCTURE_TYPE_TIMELINE_SEMAPHORE_SUBMIT_INFO: u32 = 1000207003;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_FEATURES: u32 = 1000207000;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_BUFFER_DEVICE_ADDRESS_FEATURES: u32 = 1000257000;
pub const VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO: u32 = 1_000_060_000;
pub const VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO: u32 = 1_000_244_001;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ACCELERATION_STRUCTURE_FEATURES_KHR: u32 =
    1_000_159_001;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR: u32 = 1000150000;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_DEVICE_ADDRESS_INFO_KHR: u32 = 1000150002;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_SIZES_INFO_KHR: u32 = 1000150020;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_TRIANGLES_DATA_KHR: u32 = 1000150005;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_INSTANCES_DATA_KHR: u32 = 1000150004;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_AABBS_DATA_KHR: u32 = 1000150003;
pub const VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET_ACCELERATION_STRUCTURE_KHR: u32 = 1000150007;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ACCELERATION_STRUCTURE_PROPERTIES_KHR: u32 =
    1_000_159_006;
pub const VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_CREATE_INFO_KHR: u32 = 1000150017;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_RAY_QUERY_FEATURES_KHR: u32 = 1000348013;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MESH_SHADER_FEATURES_EXT: u32 = 1000328000;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MAINTENANCE_4_FEATURES: u32 = 1000413000;
pub const VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_DRAW_PARAMETERS_FEATURES: u32 = 1000063000;
pub const VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR: u32 = 1_000_001_000;
pub const VK_STRUCTURE_TYPE_PRESENT_INFO_KHR: u32 = 1_000_001_001;
pub const VK_STRUCTURE_TYPE_MEMORY_BARRIER: u32 = 46;
pub const VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER: u32 = 44;
pub const VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER: u32 = 45;

pub const VK_PIPELINE_BIND_POINT_GRAPHICS: u32 = 0;
pub const VK_PIPELINE_BIND_POINT_COMPUTE: u32 = 1;

pub const VK_SHADER_STAGE_VERTEX_BIT: u32 = 0x0000_0001;
pub const VK_SHADER_STAGE_FRAGMENT_BIT: u32 = 0x0000_0010;
pub const VK_SHADER_STAGE_COMPUTE_BIT: u32 = 0x0000_0020;
pub const VK_SHADER_STAGE_TASK_BIT_EXT: u32 = 0x0000_0040;
pub const VK_SHADER_STAGE_MESH_BIT_EXT: u32 = 0x0000_0080;

pub const VK_QUEUE_GRAPHICS_BIT: u32 = 0x0000_0001;
pub const VK_QUEUE_COMPUTE_BIT: u32 = 0x0000_0002;
pub const VK_QUEUE_TRANSFER_BIT: u32 = 0x0000_0004;

pub const VK_PHYSICAL_DEVICE_TYPE_OTHER: u32 = 0;
pub const VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU: u32 = 1;
pub const VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU: u32 = 2;

pub const VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT: u32 = 0x0000_0001;
pub const VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT: u32 = 0x0000_0002;
pub const VK_MEMORY_PROPERTY_HOST_COHERENT_BIT: u32 = 0x0000_0004;
pub const VK_MEMORY_PROPERTY_HOST_CACHED_BIT: u32 = 0x0000_0008;
pub const VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT: u32 = 0x0000_0010;

pub const VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT: u32 = 0x0000_0002;

pub const VK_BUFFER_USAGE_TRANSFER_SRC_BIT: u32 = 0x0000_0001;
pub const VK_BUFFER_USAGE_TRANSFER_DST_BIT: u32 = 0x0000_0002;
pub const VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT: u32 = 0x0000_0010;
pub const VK_BUFFER_USAGE_STORAGE_BUFFER_BIT: u32 = 0x0000_0020;
pub const VK_BUFFER_USAGE_INDEX_BUFFER_BIT: u32 = 0x0000_0040;
pub const VK_BUFFER_USAGE_VERTEX_BUFFER_BIT: u32 = 0x0000_0080;
pub const VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT: u32 = 0x0000_0100;
pub const VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT: u32 = 0x0002_0000;
pub const VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_BIT_KHR: u32 = 0x0008_0000;
pub const VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_STORAGE_BIT_KHR: u32 = 0x0010_0000;

pub const VK_IMAGE_USAGE_TRANSFER_SRC_BIT: u32 = 0x0000_0001;
pub const VK_IMAGE_USAGE_TRANSFER_DST_BIT: u32 = 0x0000_0002;
pub const VK_IMAGE_USAGE_SAMPLED_BIT: u32 = 0x0000_0004;
pub const VK_IMAGE_USAGE_STORAGE_BIT: u32 = 0x0000_0008;
pub const VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT: u32 = 0x0000_0010;
pub const VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT: u32 = 0x0000_0020;
pub const VK_IMAGE_USAGE_TRANSIENT_ATTACHMENT_BIT: u32 = 0x0000_0040;

pub const VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT: u32 = 0x0000_0010;

pub const VK_IMAGE_TILING_OPTIMAL: u32 = 0;
pub const VK_IMAGE_TILING_LINEAR: u32 = 1;

pub const VK_IMAGE_TYPE_1D: u32 = 0;
pub const VK_IMAGE_TYPE_2D: u32 = 1;
pub const VK_IMAGE_TYPE_3D: u32 = 2;

pub const VK_IMAGE_VIEW_TYPE_1D: u32 = 0;
pub const VK_IMAGE_VIEW_TYPE_2D: u32 = 1;
pub const VK_IMAGE_VIEW_TYPE_3D: u32 = 2;
pub const VK_IMAGE_VIEW_TYPE_CUBE: u32 = 3;
pub const VK_IMAGE_VIEW_TYPE_1D_ARRAY: u32 = 4;
pub const VK_IMAGE_VIEW_TYPE_2D_ARRAY: u32 = 5;
pub const VK_IMAGE_VIEW_TYPE_CUBE_ARRAY: u32 = 6;

pub const VK_IMAGE_ASPECT_COLOR_BIT: u32 = 0x0000_0001;
pub const VK_IMAGE_ASPECT_DEPTH_BIT: u32 = 0x0000_0002;
pub const VK_IMAGE_ASPECT_STENCIL_BIT: u32 = 0x0000_0004;

pub const VK_IMAGE_LAYOUT_UNDEFINED: u32 = 0;
pub const VK_IMAGE_LAYOUT_GENERAL: u32 = 1;
pub const VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL: u32 = 2;
pub const VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL: u32 = 3;
pub const VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL: u32 = 4;
pub const VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL: u32 = 5;
pub const VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL: u32 = 6;
pub const VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL: u32 = 7;
pub const VK_IMAGE_LAYOUT_PRESENT_SRC_KHR: u32 = 1_000_001_002;

pub const VK_SHARING_MODE_EXCLUSIVE: u32 = 0;
pub const VK_SHARING_MODE_CONCURRENT: u32 = 1;

pub const VK_ATTACHMENT_LOAD_OP_LOAD: u32 = 0;
pub const VK_ATTACHMENT_LOAD_OP_CLEAR: u32 = 1;
pub const VK_ATTACHMENT_LOAD_OP_DONT_CARE: u32 = 2;
pub const VK_ATTACHMENT_STORE_OP_STORE: u32 = 0;
pub const VK_ATTACHMENT_STORE_OP_DONT_CARE: u32 = 1;
pub const VK_ATTACHMENT_UNUSED: u32 = u32::MAX;
pub const VK_SUBPASS_CONTENTS_INLINE: u32 = 0;

pub const VK_RESOLVE_MODE_NONE: u32 = 0;
pub const VK_RESOLVE_MODE_AVERAGE_BIT: u32 = 2;

pub const VK_DYNAMIC_STATE_VIEWPORT: u32 = 0;
pub const VK_DYNAMIC_STATE_SCISSOR: u32 = 1;

pub const VK_FRONT_FACE_COUNTER_CLOCKWISE: u32 = 0;
pub const VK_FRONT_FACE_CLOCKWISE: u32 = 1;

pub const VK_CULL_MODE_NONE: u32 = 0;
pub const VK_CULL_MODE_FRONT_BIT: u32 = 0x1;
pub const VK_CULL_MODE_BACK_BIT: u32 = 0x2;

pub const VK_POLYGON_MODE_FILL: u32 = 0;
pub const VK_POLYGON_MODE_LINE: u32 = 1;

pub const VK_PRIMITIVE_TOPOLOGY_POINT_LIST: u32 = 0;
pub const VK_PRIMITIVE_TOPOLOGY_LINE_LIST: u32 = 1;
pub const VK_PRIMITIVE_TOPOLOGY_LINE_STRIP: u32 = 2;
pub const VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST: u32 = 3;
pub const VK_PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP: u32 = 4;

pub const VK_VERTEX_INPUT_RATE_VERTEX: u32 = 0;
pub const VK_VERTEX_INPUT_RATE_INSTANCE: u32 = 1;

pub const VK_COMPARE_OP_NEVER: u32 = 0;
pub const VK_COMPARE_OP_LESS: u32 = 1;
pub const VK_COMPARE_OP_EQUAL: u32 = 2;
pub const VK_COMPARE_OP_LESS_OR_EQUAL: u32 = 3;
pub const VK_COMPARE_OP_GREATER: u32 = 4;
pub const VK_COMPARE_OP_NOT_EQUAL: u32 = 5;
pub const VK_COMPARE_OP_GREATER_OR_EQUAL: u32 = 6;
pub const VK_COMPARE_OP_ALWAYS: u32 = 7;

pub const VK_BLEND_FACTOR_ZERO: u32 = 0;
pub const VK_BLEND_FACTOR_ONE: u32 = 1;
pub const VK_BLEND_FACTOR_SRC_COLOR: u32 = 2;
pub const VK_BLEND_FACTOR_ONE_MINUS_SRC_COLOR: u32 = 3;
pub const VK_BLEND_FACTOR_DST_COLOR: u32 = 4;
pub const VK_BLEND_FACTOR_ONE_MINUS_DST_COLOR: u32 = 5;
pub const VK_BLEND_FACTOR_SRC_ALPHA: u32 = 6;
pub const VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA: u32 = 7;
pub const VK_BLEND_FACTOR_DST_ALPHA: u32 = 8;
pub const VK_BLEND_FACTOR_ONE_MINUS_DST_ALPHA: u32 = 9;
pub const VK_BLEND_FACTOR_CONSTANT_COLOR: u32 = 10;
pub const VK_BLEND_FACTOR_ONE_MINUS_CONSTANT_COLOR: u32 = 11;
pub const VK_BLEND_FACTOR_SRC_ALPHA_SATURATE: u32 = 14;

pub const VK_BLEND_OP_ADD: u32 = 0;
pub const VK_BLEND_OP_SUBTRACT: u32 = 1;
pub const VK_BLEND_OP_REVERSE_SUBTRACT: u32 = 2;
pub const VK_BLEND_OP_MIN: u32 = 3;
pub const VK_BLEND_OP_MAX: u32 = 4;

pub const VK_FILTER_NEAREST: u32 = 0;
pub const VK_FILTER_LINEAR: u32 = 1;
pub const VK_SAMPLER_MIPMAP_MODE_NEAREST: u32 = 0;
pub const VK_SAMPLER_MIPMAP_MODE_LINEAR: u32 = 1;
pub const VK_SAMPLER_ADDRESS_MODE_REPEAT: u32 = 0;
pub const VK_SAMPLER_ADDRESS_MODE_MIRRORED_REPEAT: u32 = 1;
pub const VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE: u32 = 2;
pub const VK_BORDER_COLOR_FLOAT_OPAQUE_BLACK: u32 = 2;

pub const VK_INDEX_TYPE_UINT16: u32 = 0;
pub const VK_INDEX_TYPE_UINT32: u32 = 1;
pub const VK_INDEX_TYPE_NONE_KHR: u32 = 1_000_165_000;

pub const VK_DESCRIPTOR_TYPE_SAMPLER: u32 = 0;
pub const VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE: u32 = 2;
pub const VK_DESCRIPTOR_TYPE_STORAGE_IMAGE: u32 = 3;
pub const VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER: u32 = 6;
pub const VK_DESCRIPTOR_TYPE_STORAGE_BUFFER: u32 = 7;
pub const VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER_DYNAMIC: u32 = 8;
pub const VK_DESCRIPTOR_TYPE_STORAGE_BUFFER_DYNAMIC: u32 = 9;
pub const VK_DESCRIPTOR_TYPE_ACCELERATION_STRUCTURE_KHR: u32 = 1000150000;

pub const VK_DESCRIPTOR_POOL_CREATE_FREE_DESCRIPTOR_SET_BIT: u32 = 0x1;

pub const VK_COMMAND_POOL_CREATE_TRANSIENT_BIT: u32 = 0x1;
pub const VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT: u32 = 0x2;
pub const VK_COMMAND_BUFFER_LEVEL_PRIMARY: u32 = 0;
pub const VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT: u32 = 0x1;

pub const VK_QUERY_TYPE_TIMESTAMP: u32 = 2;
pub const VK_QUERY_RESULT_64_BIT: u32 = 0x1;
pub const VK_QUERY_RESULT_WAIT_BIT: u32 = 0x2;

pub const VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT: u32 = 0x0000_0001;
pub const VK_PIPELINE_STAGE_TRANSFER_BIT: u32 = 0x0000_1000;
pub const VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT: u32 = 0x0000_2000;
pub const VK_PIPELINE_STAGE_HOST_BIT: u32 = 0x0000_4000;
pub const VK_PIPELINE_STAGE_ALL_COMMANDS_BIT: u32 = 0x0001_0000;

pub const VK_ACCESS_TRANSFER_READ_BIT: u32 = 0x0000_0800;
pub const VK_ACCESS_TRANSFER_WRITE_BIT: u32 = 0x0000_1000;
pub const VK_ACCESS_SHADER_READ_BIT: u32 = 0x0000_0020;
pub const VK_ACCESS_SHADER_WRITE_BIT: u32 = 0x0000_0040;
pub const VK_ACCESS_COLOR_ATTACHMENT_READ_BIT: u32 = 0x0000_0080;
pub const VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT: u32 = 0x0000_0100;
pub const VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT: u32 = 0x0000_0200;
pub const VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT: u32 = 0x0000_0400;
pub const VK_ACCESS_UNIFORM_READ_BIT: u32 = 8;
pub const VK_ACCESS_VERTEX_ATTRIBUTE_READ_BIT: u32 = 4;
pub const VK_ACCESS_INDEX_READ_BIT: u32 = 0x0000_0002;
pub const VK_ACCESS_INDIRECT_COMMAND_READ_BIT: u32 = 0x0000_0001;
pub const VK_ACCESS_HOST_WRITE_BIT: u32 = 0x0000_4000;
pub const VK_ACCESS_HOST_READ_BIT: u32 = 8192;
pub const VK_ACCESS_MEMORY_READ_BIT: u32 = 0x0000_8000;
pub const VK_ACCESS_MEMORY_WRITE_BIT: u32 = 0x0001_0000;
pub const VK_ACCESS_ACCELERATION_STRUCTURE_READ_BIT_KHR: u32 = 0x0020_0000;
pub const VK_ACCESS_ACCELERATION_STRUCTURE_WRITE_BIT_KHR: u32 = 0x0040_0000;

pub const VK_FORMAT_UNDEFINED: u32 = 0;
pub const VK_FORMAT_R8_UNORM: u32 = 9;
pub const VK_FORMAT_R8G8B8A8_UNORM: u32 = 37;
pub const VK_FORMAT_R8G8B8A8_SRGB: u32 = 43;
pub const VK_FORMAT_B8G8R8A8_UNORM: u32 = 44;
pub const VK_FORMAT_B8G8R8A8_SRGB: u32 = 50;
pub const VK_FORMAT_R32_UINT: u32 = 98;
pub const VK_FORMAT_R32_SFLOAT: u32 = 100;
pub const VK_FORMAT_R32G32_SFLOAT: u32 = 103;
pub const VK_FORMAT_R32G32B32_SFLOAT: u32 = 106;
pub const VK_FORMAT_R32G32B32A32_SFLOAT: u32 = 109;
pub const VK_FORMAT_R16G16B16A16_SFLOAT: u32 = 97;
pub const VK_FORMAT_D24_UNORM_S8_UINT: u32 = 129;
pub const VK_FORMAT_X8_D24_UNORM_PACK32: u32 = 125;
pub const VK_FORMAT_D32_SFLOAT: u32 = 126;

pub const VK_SAMPLE_COUNT_1_BIT: u32 = 1;
pub const VK_SAMPLE_COUNT_2_BIT: u32 = 2;
pub const VK_SAMPLE_COUNT_4_BIT: u32 = 4;
pub const VK_SAMPLE_COUNT_8_BIT: u32 = 8;
pub const VK_SAMPLE_COUNT_16_BIT: u32 = 16;
pub const VK_SAMPLE_COUNT_32_BIT: u32 = 32;
pub const VK_SAMPLE_COUNT_64_BIT: u32 = 64;

pub const VK_COLOR_COMPONENT_R_BIT: u32 = 0x1;
pub const VK_COLOR_COMPONENT_G_BIT: u32 = 0x2;
pub const VK_COLOR_COMPONENT_B_BIT: u32 = 0x4;
pub const VK_COLOR_COMPONENT_A_BIT: u32 = 0x8;
/// All four channels, which is the write mask Swift passes.
pub const VK_COLOR_COMPONENT_ALL: u32 = VK_COLOR_COMPONENT_R_BIT
    | VK_COLOR_COMPONENT_G_BIT
    | VK_COLOR_COMPONENT_B_BIT
    | VK_COLOR_COMPONENT_A_BIT;

pub const VK_PRESENT_MODE_IMMEDIATE_KHR: u32 = 0;
pub const VK_PRESENT_MODE_MAILBOX_KHR: u32 = 1;
pub const VK_PRESENT_MODE_FIFO_KHR: u32 = 2;

pub const VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR: u32 = 0x1;
pub const VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR: u32 = 0x1;

pub const VK_GEOMETRY_TYPE_TRIANGLES_KHR: u32 = 0;
pub const VK_GEOMETRY_TYPE_INSTANCES_KHR: u32 = 2;
pub const VK_GEOMETRY_OPAQUE_BIT_KHR: u32 = 0x1;
pub const VK_GEOMETRY_INSTANCE_TRIANGLE_FACING_CULL_DISABLE_BIT_KHR: u32 = 0x1;

pub const VK_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL_KHR: u32 = 0;
pub const VK_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL_KHR: u32 = 1;
pub const VK_ACCELERATION_STRUCTURE_BUILD_TYPE_DEVICE_KHR: u32 = 1;
pub const VK_BUILD_ACCELERATION_STRUCTURE_MODE_BUILD_KHR: u32 = 0;
pub const VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_KHR: u32 = 4;

pub const VK_SEMAPHORE_TYPE_TIMELINE: u32 = 1;
pub const VK_VENDOR_ID_UNKNOWN: u32 = 0xFFFF;

pub const VK_DRIVER_ID_UNKNOWN: u32 = 0;

// MARK: - Plain structs

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkExtent2D {
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkExtent3D {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkOffset2D {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkOffset3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkRect2D {
    pub offset: VkOffset2D,
    pub extent: VkExtent2D,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub minDepth: f32,
    pub maxDepth: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkClearColorValue {
    pub float32: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkClearDepthStencilValue {
    pub depth: f32,
    pub stencil: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageSubresourceRange {
    pub aspectMask: u32,
    pub baseMipLevel: u32,
    pub levelCount: u32,
    pub baseArrayLayer: u32,
    pub layerCount: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageSubresourceLayers {
    pub aspectMask: u32,
    pub mipLevel: u32,
    pub baseArrayLayer: u32,
    pub layerCount: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAttachmentReference {
    pub attachment: u32,
    pub layout: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorPoolSize {
    pub type_: u32,
    pub descriptorCount: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorSetLayoutBinding {
    pub binding: u32,
    pub descriptorType: u32,
    pub descriptorCount: u32,
    pub stageFlags: u32,
    pub pImmutableSamplers: *const VkSampler,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorBufferInfo {
    pub buffer: VkBuffer,
    pub offset: VkDeviceSize,
    pub range: VkDeviceSize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorImageInfo {
    pub sampler: VkSampler,
    pub imageView: VkImageView,
    pub imageLayout: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkVertexInputBindingDescription {
    pub binding: u32,
    pub stride: u32,
    pub inputRate: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkVertexInputAttributeDescription {
    pub location: u32,
    pub binding: u32,
    pub format: u32,
    pub offset: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineColorBlendAttachmentState {
    pub blendEnable: u32,
    pub srcColorBlendFactor: u32,
    pub dstColorBlendFactor: u32,
    pub colorBlendOp: u32,
    pub srcAlphaBlendFactor: u32,
    pub dstAlphaBlendFactor: u32,
    pub alphaBlendOp: u32,
    pub colorWriteMask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSpecializationMapEntry {
    pub constantID: u32,
    pub offset: u32,
    pub size: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkMemoryType {
    pub propertyFlags: u32,
    pub heapIndex: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkMemoryHeap {
    pub size: VkDeviceSize,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceMemoryProperties {
    pub memoryTypeCount: u32,
    pub memoryTypes: [VkMemoryType; VK_MAX_MEMORY_TYPES],
    pub memoryHeapCount: u32,
    pub memoryHeaps: [VkMemoryHeap; VK_MAX_MEMORY_HEAPS],
}

impl Default for VkPhysicalDeviceMemoryProperties {
    fn default() -> Self {
        Self {
            memoryTypeCount: 0,
            memoryTypes: [VkMemoryType::default(); VK_MAX_MEMORY_TYPES],
            memoryHeapCount: 0,
            memoryHeaps: [VkMemoryHeap { size: 0, flags: 0 }; VK_MAX_MEMORY_HEAPS],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceLimits {
    pub maxImageDimension1D: u32,
    pub maxImageDimension2D: u32,
    pub maxImageDimension3D: u32,
    pub maxImageDimensionCube: u32,
    pub maxImageArrayLayers: u32,
    pub maxTexelBufferElements: u32,
    pub maxUniformBufferRange: u32,
    pub maxStorageBufferRange: u32,
    pub maxPushConstantsSize: u32,
    pub maxMemoryAllocationCount: u32,
    pub maxSamplerAllocationCount: u32,
    pub bufferImageGranularity: VkDeviceSize,
    pub sparseAddressSpaceSize: VkDeviceSize,
    pub maxBoundDescriptorSets: u32,
    pub maxPerStageDescriptorSamplers: u32,
    pub maxPerStageDescriptorUniformBuffers: u32,
    pub maxPerStageDescriptorStorageBuffers: u32,
    pub maxPerStageDescriptorSampledImages: u32,
    pub maxPerStageDescriptorStorageImages: u32,
    pub maxPerStageDescriptorInputAttachments: u32,
    pub maxPerStageResources: u32,
    pub maxDescriptorSetSamplers: u32,
    pub maxDescriptorSetUniformBuffers: u32,
    pub maxDescriptorSetUniformBuffersDynamic: u32,
    pub maxDescriptorSetStorageBuffers: u32,
    pub maxDescriptorSetStorageBuffersDynamic: u32,
    pub maxDescriptorSetSampledImages: u32,
    pub maxDescriptorSetStorageImages: u32,
    pub maxVertexInputAttributes: u32,
    pub maxVertexInputBindings: u32,
    pub maxVertexInputAttributeOffset: u32,
    pub maxVertexInputBindingStride: u32,
    pub maxVertexOutputComponents: u32,
    pub maxTessellationGenerationLevel: u32,
    pub maxTessellationPatchSize: u32,
    pub maxTessellationControlPerVertexInputComponents: u32,
    pub maxTessellationControlPerVertexOutputComponents: u32,
    pub maxTessellationControlPerPatchOutputComponents: u32,
    pub maxTessellationControlTotalOutputComponents: u32,
    pub maxTessellationEvaluationInputComponents: u32,
    pub maxTessellationEvaluationOutputComponents: u32,
    pub maxGeometryShaderInvocations: u32,
    pub maxGeometryInputComponents: u32,
    pub maxGeometryOutputComponents: u32,
    pub maxGeometryOutputVertices: u32,
    pub maxGeometryTotalOutputComponents: u32,
    pub maxFragmentInputComponents: u32,
    pub maxFragmentOutputAttachments: u32,
    pub maxFragmentDualSrcAttachments: u32,
    pub maxFragmentCombinedOutputResources: u32,
    pub maxComputeSharedMemorySize: u32,
    pub maxComputeWorkGroupCount: [u32; 3],
    pub maxComputeWorkGroupInvocations: u32,
    pub maxComputeWorkGroupSize: [u32; 3],
    pub subPixelPrecisionBits: u32,
    pub subTexelPrecisionBits: u32,
    pub mipmapPrecisionBits: u32,
    pub maxDrawIndexedIndexValue: u32,
    pub maxDrawIndirectCount: u32,
    pub maxSamplerLodBias: f32,
    pub maxSamplerAnisotropy: f32,
    pub maxViewports: u32,
    pub maxViewportDimensions: [u32; 2],
    pub viewportBoundsRange: [f32; 2],
    pub viewportSubPixelBits: u32,
    pub minMemoryMapAlignment: usize,
    pub minTexelBufferOffsetAlignment: VkDeviceSize,
    pub minUniformBufferOffsetAlignment: VkDeviceSize,
    pub minStorageBufferOffsetAlignment: VkDeviceSize,
    pub minTexelOffset: i32,
    pub maxTexelOffset: u32,
    pub minTexelGatherOffset: i32,
    pub maxTexelGatherOffset: u32,
    pub minInterpolationOffset: f32,
    pub maxInterpolationOffset: f32,
    pub subPixelInterpolationOffsetBits: u32,
    pub maxFramebufferWidth: u32,
    pub maxFramebufferHeight: u32,
    pub maxFramebufferLayers: u32,
    pub framebufferColorSampleCounts: u32,
    pub framebufferDepthSampleCounts: u32,
    pub framebufferStencilSampleCounts: u32,
    pub framebufferNoAttachmentsSampleCounts: u32,
    pub maxColorAttachments: u32,
    pub sampledImageColorSampleCounts: u32,
    pub sampledImageIntegerSampleCounts: u32,
    pub sampledImageDepthSampleCounts: u32,
    pub sampledImageStencilSampleCounts: u32,
    pub storageImageSampleCounts: u32,
    pub maxSampleMaskWords: u32,
    pub timestampComputeAndGraphics: u32,
    pub timestampPeriod: f32,
    pub maxClipDistances: u32,
    pub maxCullDistances: u32,
    pub maxCombinedClipAndCullDistances: u32,
    pub discreteQueuePriorities: u32,
    pub pointSizeRange: [f32; 2],
    pub lineWidthRange: [f32; 2],
    pub pointSizeGranularity: f32,
    pub lineWidthGranularity: f32,
    pub strictLines: u32,
    pub standardSampleLocations: u32,
    pub optimalBufferCopyOffsetAlignment: VkDeviceSize,
    pub optimalBufferCopyRowPitchAlignment: VkDeviceSize,
    pub nonCoherentAtomSize: VkDeviceSize,
}

impl Default for VkPhysicalDeviceLimits {
    fn default() -> Self {
        // SAFETY: the struct is plain integers and floats, so an all-zero value
        // is a valid (if unusable) limits block; drivers overwrite it whole.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceFeatures {
    pub robustBufferAccess: u32,
    pub fullDrawIndexUint32: u32,
    pub imageCubeArray: u32,
    pub independentBlend: u32,
    pub geometryShader: u32,
    pub tessellationShader: u32,
    pub sampleRateShading: u32,
    pub dualSrcBlend: u32,
    pub logicOp: u32,
    pub multiDrawIndirect: u32,
    pub drawIndirectFirstInstance: u32,
    pub depthClamp: u32,
    pub depthBiasClamp: u32,
    pub fillModeNonSolid: u32,
    pub depthBounds: u32,
    pub wideLines: u32,
    pub largePoints: u32,
    pub alphaToOne: u32,
    pub multiViewport: u32,
    pub samplerAnisotropy: u32,
    pub textureCompressionETC2: u32,
    pub textureCompressionASTC_LDR: u32,
    pub textureCompressionBC: u32,
    pub occlusionQueryPrecise: u32,
    pub pipelineStatisticsQuery: u32,
    pub vertexPipelineStoresAndAtomics: u32,
    pub fragmentStoresAndAtomics: u32,
    pub shaderTessellationAndGeometryPointSize: u32,
    pub shaderImageGatherExtended: u32,
    pub shaderStorageImageExtendedFormats: u32,
    pub shaderStorageImageMultisample: u32,
    pub shaderStorageImageReadWithoutFormat: u32,
    pub shaderStorageImageWriteWithoutFormat: u32,
    pub shaderUniformBufferArrayDynamicIndexing: u32,
    pub shaderSampledImageArrayDynamicIndexing: u32,
    pub shaderStorageBufferArrayDynamicIndexing: u32,
    pub shaderStorageImageArrayDynamicIndexing: u32,
    pub shaderClipDistance: u32,
    pub shaderCullDistance: u32,
    pub shaderFloat64: u32,
    pub shaderInt64: u32,
    pub shaderInt16: u32,
    pub shaderResourceResidency: u32,
    pub shaderResourceMinLod: u32,
    pub sparseBinding: u32,
    pub sparseResidencyBuffer: u32,
    pub sparseResidencyImage2D: u32,
    pub sparseResidencyImage3D: u32,
    pub sparseResidency2Samples: u32,
    pub sparseResidency4Samples: u32,
    pub sparseResidency8Samples: u32,
    pub sparseResidency16Samples: u32,
    pub sparseResidencyAliased: u32,
    pub variableMultisampleRate: u32,
    pub inheritedQueries: u32,
}

impl Default for VkPhysicalDeviceFeatures {
    fn default() -> Self {
        // SAFETY: plain integers, so all-zero is the documented "nothing
        // enabled" state.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceProperties {
    pub apiVersion: u32,
    pub driverVersion: u32,
    pub vendorID: u32,
    pub deviceID: u32,
    pub deviceType: u32,
    pub deviceName: [c_char; VK_MAX_DEVICE_NAME_SIZE],
    pub pipelineCacheUUID: [u8; VK_UUID_SIZE],
    pub limits: VkPhysicalDeviceLimits,
    pub sparseProperties: [u32; 20],
}

impl Default for VkPhysicalDeviceProperties {
    fn default() -> Self {
        // SAFETY: plain data; the driver fills the whole struct.
        unsafe { core::mem::zeroed() }
    }
}

impl VkPhysicalDeviceProperties {
    /// The NUL-terminated device name as a Rust string.
    pub fn device_name(&self) -> String {
        let bytes: Vec<u8> = self
            .deviceName
            .iter()
            .take_while(|value| **value != 0)
            .map(|value| *value as u8)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkExtensionProperties {
    pub extensionName: [c_char; VK_MAX_EXTENSION_NAME_SIZE],
    pub specVersion: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerProperties {
    pub layerName: [c_char; VK_MAX_EXTENSION_NAME_SIZE],
    pub specVersion: u32,
    pub implementationVersion: u32,
    pub description: [c_char; VK_MAX_DESCRIPTION_SIZE],
}

impl Default for VkLayerProperties {
    fn default() -> Self {
        // SAFETY: fixed-size char buffers plus integers.
        unsafe { core::mem::zeroed() }
    }
}

impl VkLayerProperties {
    /// The layer name as a Rust string.
    pub fn name(&self) -> String {
        let bytes: Vec<u8> = self
            .layerName
            .iter()
            .take_while(|value| **value != 0)
            .map(|value| *value as u8)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Default for VkExtensionProperties {
    fn default() -> Self {
        // SAFETY: a fixed-size char buffer plus an integer.
        unsafe { core::mem::zeroed() }
    }
}

impl VkExtensionProperties {
    /// The extension name as a Rust string.
    pub fn name(&self) -> String {
        let bytes: Vec<u8> = self
            .extensionName
            .iter()
            .take_while(|value| **value != 0)
            .map(|value| *value as u8)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkQueueFamilyProperties {
    pub queueFlags: u32,
    pub queueCount: u32,
    pub timestampValidBits: u32,
    pub minImageTransferGranularity: VkExtent3D,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkMemoryRequirements {
    pub size: VkDeviceSize,
    pub alignment: VkDeviceSize,
    pub memoryTypeBits: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageFormatProperties {
    pub maxExtent: VkExtent3D,
    pub maxMipLevels: u32,
    pub maxArrayLayers: u32,
    pub sampleCounts: u32,
    pub maxResourceSize: VkDeviceSize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSurfaceFormatKHR {
    pub format: u32,
    pub colorSpace: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSurfaceCapabilitiesKHR {
    pub minImageCount: u32,
    pub maxImageCount: u32,
    pub currentExtent: VkExtent2D,
    pub minImageExtent: VkExtent2D,
    pub maxImageExtent: VkExtent2D,
    pub maxImageArrayLayers: u32,
    pub supportedTransforms: u32,
    pub currentTransform: u32,
    pub supportedCompositeAlpha: u32,
    pub supportedUsageFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkBufferCopy {
    pub srcOffset: VkDeviceSize,
    pub dstOffset: VkDeviceSize,
    pub size: VkDeviceSize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkBufferImageCopy {
    pub bufferOffset: VkDeviceSize,
    pub bufferRowLength: u32,
    pub bufferImageHeight: u32,
    pub imageSubresource: VkImageSubresourceLayers,
    pub imageOffset: VkOffset3D,
    pub imageExtent: VkExtent3D,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageCopy {
    pub srcSubresource: VkImageSubresourceLayers,
    pub srcOffset: VkOffset3D,
    pub dstSubresource: VkImageSubresourceLayers,
    pub dstOffset: VkOffset3D,
    pub extent: VkExtent3D,
}

/// The 64-byte GPU instance layout Swift's C shim packs. The transform is a
/// 3x4 row-major matrix, and the three trailing fields share one 32-bit word of
/// bitfields, which is why this is written as a single `u32` with accessors.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureInstanceKHR {
    pub transform: [f32; 12],
    /// `instanceCustomIndex:24 | mask:8`, per the spec's bitfield packing.
    pub instance_word: u32,
    /// `instanceShaderBindingTableRecordOffset:24 | flags:8`.
    pub offset_and_flags: u32,
    /// 64-bit device address; `repr(C)` puts this at offset 56.
    pub accelerationStructureReference: VkDeviceAddress,
}

impl VkAccelerationStructureInstanceKHR {
    /// Builds one instance record. `flags` carries the cull-disable bit Swift
    /// sets, `custom_index` the instance ordinal.
    pub fn new(
        transform: [f32; 12],
        custom_index: u32,
        mask: u32,
        address: VkDeviceAddress,
    ) -> Self {
        Self {
            transform,
            // Bit 0 is `triangleFacingCullDisable`, bits 1-23 are the SBT
            // record offset, and bits 24-31 are the mask<<24 shadow field. Swift
            // disables facing culling and leaves the offset at zero.
            // Low 24 bits are the custom index, high 8 bits are the visibility mask.
            instance_word: (custom_index & 0x00FF_FFFF) | ((mask & 0xFF) << 24),
            offset_and_flags: VK_GEOMETRY_INSTANCE_TRIANGLE_FACING_CULL_DISABLE_BIT_KHR,
            accelerationStructureReference: address,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureBuildRangeInfoKHR {
    pub primitiveCount: u32,
    pub primitiveOffset: u32,
    pub firstVertex: u32,
    pub transformOffset: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureGeometryTrianglesDataKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub vertexFormat: u32,
    pub vertexData: VkDeviceAddress,
    pub vertexStride: VkDeviceSize,
    pub maxVertex: u32,
    pub indexType: u32,
    pub indexData: VkDeviceAddress,
    pub transformData: VkDeviceAddress,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureGeometryInstancesDataKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub arrayOfPointers: u32,
    pub data: VkDeviceAddress,
}

/// Storage for whichever geometry payload a `VkAccelerationStructureGeometryKHR`
/// carries. The union in C is modelled as the largest member so the struct keeps
/// the driver-visible size and alignment.
#[repr(C)]
#[derive(Clone, Copy)]
pub union VkAccelerationStructureGeometryDataKHR {
    pub triangles: VkAccelerationStructureGeometryTrianglesDataKHR,
    pub instances: VkAccelerationStructureGeometryInstancesDataKHR,
}

impl Default for VkAccelerationStructureGeometryDataKHR {
    fn default() -> Self {
        Self {
            instances: VkAccelerationStructureGeometryInstancesDataKHR::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureGeometryKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub geometryType: u32,
    pub geometry: VkAccelerationStructureGeometryDataKHR,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureBuildSizesInfoKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub accelerationStructureSize: VkDeviceSize,
    pub updateScratchSize: VkDeviceSize,
    pub buildScratchSize: VkDeviceSize,
}

// MARK: - Create-info structs (each starts with sType/pNext, as Vulkan requires)

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkApplicationInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub pApplicationName: *const c_char,
    pub applicationVersion: u32,
    pub pEngineName: *const c_char,
    pub engineVersion: u32,
    pub apiVersion: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkInstanceCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub pApplicationInfo: *const VkApplicationInfo,
    pub enabledLayerCount: u32,
    pub ppEnabledLayerNames: *const *const c_char,
    pub enabledExtensionCount: u32,
    pub ppEnabledExtensionNames: *const *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDeviceQueueCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub queueFamilyIndex: u32,
    pub queueCount: u32,
    pub pQueuePriorities: *const f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDeviceCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub queueCreateInfoCount: u32,
    pub pQueueCreateInfos: *const VkDeviceQueueCreateInfo,
    pub enabledLayerCount: u32,
    pub ppEnabledLayerNames: *const *const c_char,
    pub enabledExtensionCount: u32,
    pub ppEnabledExtensionNames: *const *const c_char,
    pub pEnabledFeatures: *const VkPhysicalDeviceFeatures,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSubmitInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub waitSemaphoreCount: u32,
    pub pWaitSemaphores: *const VkSemaphore,
    pub pWaitDstStageMask: *const u32,
    pub commandBufferCount: u32,
    pub pCommandBuffers: *const VkCommandBuffer,
    pub signalSemaphoreCount: u32,
    pub pSignalSemaphores: *const VkSemaphore,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkTimelineSemaphoreSubmitInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub waitSemaphoreValueCount: u32,
    pub pWaitSemaphoreValues: *const u64,
    pub signalSemaphoreValueCount: u32,
    pub pSignalSemaphoreValues: *const u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPresentInfoKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub waitSemaphoreCount: u32,
    pub pWaitSemaphores: *const VkSemaphore,
    pub swapchainCount: u32,
    pub pSwapchains: *const VkSwapchainKHR,
    pub pImageIndices: *const u32,
    pub pResults: *mut VkResult,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSwapchainCreateInfoKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub surface: VkSurfaceKHR,
    pub minImageCount: u32,
    pub imageFormat: u32,
    pub imageColorSpace: u32,
    pub imageExtent: VkExtent2D,
    pub imageArrayLayers: u32,
    pub imageUsage: u32,
    pub imageSharingMode: u32,
    pub queueFamilyIndexCount: u32,
    pub pQueueFamilyIndices: *const u32,
    pub preTransform: u32,
    pub compositeAlpha: u32,
    pub presentMode: u32,
    pub clipped: u32,
    pub oldSwapchain: VkSwapchainKHR,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkMemoryAllocateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub allocationSize: VkDeviceSize,
    pub memoryTypeIndex: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkMemoryAllocateFlagsInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub deviceMask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkFenceCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSemaphoreCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSemaphoreTypeCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub semaphoreType: u32,
    pub initialValue: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkQueryPoolCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub queryType: u32,
    pub queryCount: u32,
    pub pipelineStatistics: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkBufferCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub size: VkDeviceSize,
    pub usage: u32,
    pub sharingMode: u32,
    pub queueFamilyIndexCount: u32,
    pub pQueueFamilyIndices: *const u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkBufferDeviceAddressInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub buffer: VkBuffer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub imageType: u32,
    pub format: u32,
    pub extent: VkExtent3D,
    pub mipLevels: u32,
    pub arrayLayers: u32,
    pub samples: u32,
    pub tiling: u32,
    pub usage: u32,
    pub sharingMode: u32,
    pub queueFamilyIndexCount: u32,
    pub pQueueFamilyIndices: *const u32,
    pub initialLayout: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageViewCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub image: VkImage,
    pub viewType: u32,
    pub format: u32,
    pub components: [u32; 4],
    pub subresourceRange: VkImageSubresourceRange,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkShaderModuleCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub codeSize: usize,
    pub pCode: *const u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineLayoutCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub setLayoutCount: u32,
    pub pSetLayouts: *const VkDescriptorSetLayout,
    pub pushConstantRangeCount: u32,
    pub pPushConstantRanges: *const VkPushConstantRange,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPushConstantRange {
    pub stageFlags: u32,
    pub offset: u32,
    pub size: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorSetLayoutCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub bindingCount: u32,
    pub pBindings: *const VkDescriptorSetLayoutBinding,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorPoolCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub maxSets: u32,
    pub poolSizeCount: u32,
    pub pPoolSizes: *const VkDescriptorPoolSize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkDescriptorSetAllocateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub descriptorPool: VkDescriptorPool,
    pub descriptorSetCount: u32,
    pub pSetLayouts: *const VkDescriptorSetLayout,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkWriteDescriptorSet {
    pub sType: u32,
    pub pNext: *const c_void,
    pub dstSet: VkDescriptorSet,
    pub dstBinding: u32,
    pub dstArrayElement: u32,
    pub descriptorCount: u32,
    pub descriptorType: u32,
    pub pImageInfo: *const VkDescriptorImageInfo,
    pub pBufferInfo: *const VkDescriptorBufferInfo,
    pub pTexelBufferView: *const VkBufferView,
}

pub type VkBufferView = u64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkWriteDescriptorSetAccelerationStructureKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub accelerationStructureCount: u32,
    pub pAccelerationStructures: *const VkAccelerationStructureKHR,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAttachmentDescription {
    pub flags: u32,
    pub format: u32,
    pub samples: u32,
    pub loadOp: u32,
    pub storeOp: u32,
    pub stencilLoadOp: u32,
    pub stencilStoreOp: u32,
    pub initialLayout: u32,
    pub finalLayout: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSubpassDescription {
    pub flags: u32,
    pub pipelineBindPoint: u32,
    pub inputAttachmentCount: u32,
    pub pInputAttachments: *const VkAttachmentReference,
    pub colorAttachmentCount: u32,
    pub pColorAttachments: *const VkAttachmentReference,
    pub pResolveAttachments: *const VkAttachmentReference,
    pub pDepthStencilAttachment: *const VkAttachmentReference,
    pub preserveAttachmentCount: u32,
    pub pPreserveAttachments: *const u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkRenderPassCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub attachmentCount: u32,
    pub pAttachments: *const VkAttachmentDescription,
    pub subpassCount: u32,
    pub pSubpasses: *const VkSubpassDescription,
    pub dependencyCount: u32,
    pub pDependencies: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkFramebufferCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub renderPass: VkRenderPass,
    pub attachmentCount: u32,
    pub pAttachments: *const VkImageView,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkCommandPoolCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub queueFamilyIndex: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkCommandBufferAllocateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub commandPool: VkCommandPool,
    pub level: u32,
    pub commandBufferCount: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkCommandBufferBeginInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub pInheritanceInfo: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkRenderPassBeginInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub renderPass: VkRenderPass,
    pub framebuffer: VkFramebuffer,
    pub renderArea: VkRect2D,
    pub clearValueCount: u32,
    pub pClearValues: *const VkClearValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VkClearValue {
    pub color: VkClearColorValue,
    pub depthStencil: VkClearDepthStencilValue,
}

impl Default for VkClearValue {
    fn default() -> Self {
        Self {
            color: VkClearColorValue::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkMemoryBarrier {
    pub sType: u32,
    pub pNext: *const c_void,
    pub srcAccessMask: u32,
    pub dstAccessMask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkBufferMemoryBarrier {
    pub sType: u32,
    pub pNext: *const c_void,
    pub srcAccessMask: u32,
    pub dstAccessMask: u32,
    pub srcQueueFamilyIndex: u32,
    pub dstQueueFamilyIndex: u32,
    pub buffer: VkBuffer,
    pub offset: VkDeviceSize,
    pub size: VkDeviceSize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkImageMemoryBarrier {
    pub sType: u32,
    pub pNext: *const c_void,
    pub srcAccessMask: u32,
    pub dstAccessMask: u32,
    pub oldLayout: u32,
    pub newLayout: u32,
    pub srcQueueFamilyIndex: u32,
    pub dstQueueFamilyIndex: u32,
    pub image: VkImage,
    pub subresourceRange: VkImageSubresourceRange,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSamplerCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub magFilter: u32,
    pub minFilter: u32,
    pub mipmapMode: u32,
    pub addressModeU: u32,
    pub addressModeV: u32,
    pub addressModeW: u32,
    pub mipLodBias: f32,
    pub anisotropyEnable: u32,
    pub maxAnisotropy: f32,
    pub compareEnable: u32,
    pub compareOp: u32,
    pub minLod: f32,
    pub maxLod: f32,
    pub borderColor: u32,
    pub unnormalizedCoordinates: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkVertexInputStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub vertexBindingDescriptionCount: u32,
    pub pVertexBindingDescriptions: *const VkVertexInputBindingDescription,
    pub vertexAttributeDescriptionCount: u32,
    pub pVertexAttributeDescriptions: *const VkVertexInputAttributeDescription,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkInputAssemblyStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub topology: u32,
    pub primitiveRestartEnable: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineRasterizationStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub depthClampEnable: u32,
    pub rasterizerDiscardEnable: u32,
    pub polygonMode: u32,
    pub cullMode: u32,
    pub frontFace: u32,
    pub depthBiasEnable: u32,
    pub depthBiasConstantFactor: f32,
    pub depthBiasClamp: f32,
    pub depthBiasSlopeFactor: f32,
    pub lineWidth: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineViewportStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub viewportCount: u32,
    pub pViewports: *const VkViewport,
    pub scissorCount: u32,
    pub pScissors: *const VkRect2D,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineMultisampleStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub rasterizationSamples: u32,
    pub sampleShadingEnable: u32,
    pub minSampleShading: f32,
    pub pSampleMask: *const VkSampleMask,
    pub alphaToCoverageEnable: u32,
    pub alphaToOneEnable: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkStencilOpState {
    pub failOp: u32,
    pub passOp: u32,
    pub depthFailOp: u32,
    pub compareOp: u32,
    pub compareMask: u32,
    pub writeMask: u32,
    pub reference: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineDepthStencilStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub depthTestEnable: u32,
    pub depthWriteEnable: u32,
    pub depthCompareOp: u32,
    pub depthBoundsTestEnable: u32,
    pub stencilTestEnable: u32,
    pub front: VkStencilOpState,
    pub back: VkStencilOpState,
    pub minDepthBounds: f32,
    pub maxDepthBounds: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineColorBlendStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub logicOpEnable: u32,
    pub logicOp: u32,
    pub attachmentCount: u32,
    pub pAttachments: *const VkPipelineColorBlendAttachmentState,
    pub blendConstants: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineDynamicStateCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub dynamicStateCount: u32,
    pub pDynamicStates: *const u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkSpecializationInfo {
    pub mapEntryCount: u32,
    pub pMapEntries: *const VkSpecializationMapEntry,
    pub dataSize: usize,
    pub pData: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkPipelineShaderStageCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub stage: u32,
    pub module: VkShaderModule,
    pub pName: *const c_char,
    pub pSpecializationInfo: *const VkSpecializationInfo,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkGraphicsPipelineCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub stageCount: u32,
    pub pStages: *const VkPipelineShaderStageCreateInfo,
    pub pVertexInputState: *const VkVertexInputStateCreateInfo,
    pub pInputAssemblyState: *const VkInputAssemblyStateCreateInfo,
    pub pTessellationState: *const c_void,
    pub pViewportState: *const VkPipelineViewportStateCreateInfo,
    pub pRasterizationState: *const VkPipelineRasterizationStateCreateInfo,
    pub pMultisampleState: *const VkPipelineMultisampleStateCreateInfo,
    pub pDepthStencilState: *const VkPipelineDepthStencilStateCreateInfo,
    pub pColorBlendState: *const VkPipelineColorBlendStateCreateInfo,
    pub pDynamicState: *const VkPipelineDynamicStateCreateInfo,
    pub layout: VkPipelineLayout,
    pub renderPass: VkRenderPass,
    pub subpass: u32,
    pub basePipelineHandle: VkPipeline,
    pub basePipelineIndex: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkComputePipelineCreateInfo {
    pub sType: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub stage: VkPipelineShaderStageCreateInfo,
    pub layout: VkPipelineLayout,
    pub basePipelineHandle: VkPipeline,
    pub basePipelineIndex: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureCreateInfoKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub createFlags: u32,
    pub buffer: VkBuffer,
    pub offset: VkDeviceSize,
    pub size: VkDeviceSize,
    pub type_: u32,
    pub deviceAddress: VkDeviceAddress,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkAccelerationStructureDeviceAddressInfoKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub accelerationStructure: VkAccelerationStructureKHR,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkAccelerationStructureBuildGeometryInfoKHR {
    pub sType: u32,
    pub pNext: *const c_void,
    pub type_: u32,
    pub flags: u32,
    pub mode: u32,
    pub srcAccelerationStructure: VkAccelerationStructureKHR,
    pub dstAccelerationStructure: VkAccelerationStructureKHR,
    pub geometryCount: u32,
    pub pGeometries: *const VkAccelerationStructureGeometryKHR,
    pub ppGeometries: *const *const VkAccelerationStructureGeometryKHR,
    pub scratchData: VkDeviceAddress,
}

impl Default for VkAccelerationStructureBuildGeometryInfoKHR {
    fn default() -> Self {
        // SAFETY: the struct is plain integers, handles and pointers; all-zero is
        // the documented "nothing set" state and the caller fills every field it
        // needs before the call.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceFeatures2 {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub features: VkPhysicalDeviceFeatures,
}

impl Default for VkPhysicalDeviceFeatures2 {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
            pNext: core::ptr::null_mut(),
            features: VkPhysicalDeviceFeatures::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceProperties2 {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub properties: VkPhysicalDeviceProperties,
}

impl Default for VkPhysicalDeviceProperties2 {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
            pNext: core::ptr::null_mut(),
            properties: VkPhysicalDeviceProperties::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceTimelineSemaphoreFeatures {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub timelineSemaphore: u32,
}

impl Default for VkPhysicalDeviceTimelineSemaphoreFeatures {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_FEATURES,
            pNext: core::ptr::null_mut(),
            timelineSemaphore: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceBufferDeviceAddressFeatures {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub bufferDeviceAddress: u32,
    pub bufferDeviceAddressCaptureReplay: u32,
    pub bufferDeviceAddressMultiDevice: u32,
}

impl Default for VkPhysicalDeviceBufferDeviceAddressFeatures {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_BUFFER_DEVICE_ADDRESS_FEATURES,
            pNext: core::ptr::null_mut(),
            bufferDeviceAddress: 0,
            bufferDeviceAddressCaptureReplay: 0,
            bufferDeviceAddressMultiDevice: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceAccelerationStructureFeaturesKHR {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub accelerationStructure: u32,
    pub accelerationStructureCaptureReplay: u32,
    pub accelerationStructureIndirectBuild: u32,
    pub accelerationStructureHostCommands: u32,
    pub descriptorBindingAccelerationStructureUpdateAfterBind: u32,
}

impl Default for VkPhysicalDeviceAccelerationStructureFeaturesKHR {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ACCELERATION_STRUCTURE_FEATURES_KHR,
            pNext: core::ptr::null_mut(),
            accelerationStructure: 0,
            accelerationStructureCaptureReplay: 0,
            accelerationStructureIndirectBuild: 0,
            accelerationStructureHostCommands: 0,
            descriptorBindingAccelerationStructureUpdateAfterBind: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceAccelerationStructurePropertiesKHR {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub maxGeometryCount: u64,
    pub maxInstanceCount: u64,
    pub maxPrimitiveCount: u64,
    pub maxPerStageDescriptorAccelerationStructures: u32,
    pub maxPerStageDescriptorUpdateAfterBindAccelerationStructures: u32,
    pub maxDescriptorSetAccelerationStructures: u32,
    pub maxDescriptorSetUpdateAfterBindAccelerationStructures: u32,
    pub minAccelerationStructureScratchOffsetAlignment: u32,
}

impl Default for VkPhysicalDeviceAccelerationStructurePropertiesKHR {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ACCELERATION_STRUCTURE_PROPERTIES_KHR,
            pNext: core::ptr::null_mut(),
            maxGeometryCount: 0,
            maxInstanceCount: 0,
            maxPrimitiveCount: 0,
            maxPerStageDescriptorAccelerationStructures: 0,
            maxPerStageDescriptorUpdateAfterBindAccelerationStructures: 0,
            maxDescriptorSetAccelerationStructures: 0,
            maxDescriptorSetUpdateAfterBindAccelerationStructures: 0,
            minAccelerationStructureScratchOffsetAlignment: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceRayQueryFeaturesKHR {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub rayQuery: u32,
}

impl Default for VkPhysicalDeviceRayQueryFeaturesKHR {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_RAY_QUERY_FEATURES_KHR,
            pNext: core::ptr::null_mut(),
            rayQuery: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceMeshShaderFeaturesEXT {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub taskShader: u32,
    pub meshShader: u32,
    pub multiviewMeshShader: u32,
    pub primitiveFragmentShadingRateMeshShader: u32,
    pub meshShaderQueries: u32,
}

impl Default for VkPhysicalDeviceMeshShaderFeaturesEXT {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MESH_SHADER_FEATURES_EXT,
            pNext: core::ptr::null_mut(),
            taskShader: 0,
            meshShader: 0,
            multiviewMeshShader: 0,
            primitiveFragmentShadingRateMeshShader: 0,
            meshShaderQueries: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceMaintenance4Features {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub maintenance4: u32,
}

impl Default for VkPhysicalDeviceMaintenance4Features {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MAINTENANCE_4_FEATURES,
            pNext: core::ptr::null_mut(),
            maintenance4: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkPhysicalDeviceShaderDrawParametersFeatures {
    pub sType: u32,
    pub pNext: *mut c_void,
    pub shaderDrawParameters: u32,
}

impl Default for VkPhysicalDeviceShaderDrawParametersFeatures {
    fn default() -> Self {
        Self {
            sType: VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_DRAW_PARAMETERS_FEATURES,
            pNext: core::ptr::null_mut(),
            shaderDrawParameters: 0,
        }
    }
}

// MARK: - Function pointer types

pub type PFN_vkGetInstanceProcAddr =
    unsafe extern "system" fn(VkInstance, *const c_char) -> PFN_vkVoidFunction;
pub type PFN_vkGetDeviceProcAddr =
    unsafe extern "system" fn(VkDevice, *const c_char) -> PFN_vkVoidFunction;
pub type PFN_vkCreateInstance = unsafe extern "system" fn(
    *const VkInstanceCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkInstance,
) -> VkResult;
pub type PFN_vkDestroyInstance =
    unsafe extern "system" fn(VkInstance, *const VkAllocationCallbacks);
pub type PFN_vkEnumerateInstanceVersion = unsafe extern "system" fn(*mut u32) -> VkResult;
pub type PFN_vkEnumerateInstanceLayerProperties =
    unsafe extern "system" fn(*mut u32, *mut VkLayerProperties) -> VkResult;
pub type PFN_vkEnumerateInstanceExtensionProperties =
    unsafe extern "system" fn(*const c_char, *mut u32, *mut VkExtensionProperties) -> VkResult;
pub type PFN_vkEnumeratePhysicalDevices =
    unsafe extern "system" fn(VkInstance, *mut u32, *mut VkPhysicalDevice) -> VkResult;
pub type PFN_vkGetPhysicalDeviceProperties =
    unsafe extern "system" fn(VkPhysicalDevice, *mut VkPhysicalDeviceProperties);
pub type PFN_vkGetPhysicalDeviceProperties2 =
    unsafe extern "system" fn(VkPhysicalDevice, *mut VkPhysicalDeviceProperties2);
pub type PFN_vkGetPhysicalDeviceFeatures2 =
    unsafe extern "system" fn(VkPhysicalDevice, *mut VkPhysicalDeviceFeatures2);
pub type PFN_vkEnumerateDeviceExtensionProperties = unsafe extern "system" fn(
    VkPhysicalDevice,
    *const c_char,
    *mut u32,
    *mut VkExtensionProperties,
) -> VkResult;
pub type PFN_vkGetPhysicalDeviceQueueFamilyProperties =
    unsafe extern "system" fn(VkPhysicalDevice, *mut u32, *mut VkQueueFamilyProperties);
pub type PFN_vkGetPhysicalDeviceMemoryProperties =
    unsafe extern "system" fn(VkPhysicalDevice, *mut VkPhysicalDeviceMemoryProperties);
pub type PFN_vkGetPhysicalDeviceImageFormatProperties = unsafe extern "system" fn(
    VkPhysicalDevice,
    u32,
    u32,
    u32,
    u32,
    u32,
    *mut VkImageFormatProperties,
) -> VkResult;
pub type PFN_vkCreateDevice = unsafe extern "system" fn(
    VkPhysicalDevice,
    *const VkDeviceCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkDevice,
) -> VkResult;
pub type PFN_vkDestroyDevice = unsafe extern "system" fn(VkDevice, *const VkAllocationCallbacks);
pub type PFN_vkGetDeviceQueue = unsafe extern "system" fn(VkDevice, u32, u32, *mut VkQueue);
pub type PFN_vkDeviceWaitIdle = unsafe extern "system" fn(VkDevice) -> VkResult;
pub type PFN_vkDestroySurfaceKHR =
    unsafe extern "system" fn(VkInstance, VkSurfaceKHR, *const VkAllocationCallbacks);
pub type PFN_vkGetPhysicalDeviceSurfaceSupportKHR =
    unsafe extern "system" fn(VkPhysicalDevice, u32, VkSurfaceKHR, *mut VkBool32) -> VkResult;
pub type PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR = unsafe extern "system" fn(
    VkPhysicalDevice,
    VkSurfaceKHR,
    *mut VkSurfaceCapabilitiesKHR,
) -> VkResult;
pub type PFN_vkGetPhysicalDeviceSurfaceFormatsKHR = unsafe extern "system" fn(
    VkPhysicalDevice,
    VkSurfaceKHR,
    *mut u32,
    *mut VkSurfaceFormatKHR,
) -> VkResult;
pub type PFN_vkGetPhysicalDeviceSurfacePresentModesKHR =
    unsafe extern "system" fn(VkPhysicalDevice, VkSurfaceKHR, *mut u32, *mut u32) -> VkResult;
pub type PFN_vkCreateBuffer = unsafe extern "system" fn(
    VkDevice,
    *const VkBufferCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkBuffer,
) -> VkResult;
pub type PFN_vkDestroyBuffer =
    unsafe extern "system" fn(VkDevice, VkBuffer, *const VkAllocationCallbacks);
pub type PFN_vkGetBufferMemoryRequirements =
    unsafe extern "system" fn(VkDevice, VkBuffer, *mut VkMemoryRequirements);
pub type PFN_vkBindBufferMemory =
    unsafe extern "system" fn(VkDevice, VkBuffer, VkDeviceMemory, VkDeviceSize) -> VkResult;
pub type PFN_vkCreateImage = unsafe extern "system" fn(
    VkDevice,
    *const VkImageCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkImage,
) -> VkResult;
pub type PFN_vkDestroyImage =
    unsafe extern "system" fn(VkDevice, VkImage, *const VkAllocationCallbacks);
pub type PFN_vkGetImageMemoryRequirements =
    unsafe extern "system" fn(VkDevice, VkImage, *mut VkMemoryRequirements);
pub type PFN_vkBindImageMemory =
    unsafe extern "system" fn(VkDevice, VkImage, VkDeviceMemory, VkDeviceSize) -> VkResult;
pub type PFN_vkAllocateMemory = unsafe extern "system" fn(
    VkDevice,
    *const VkMemoryAllocateInfo,
    *const VkAllocationCallbacks,
    *mut VkDeviceMemory,
) -> VkResult;
pub type PFN_vkFreeMemory =
    unsafe extern "system" fn(VkDevice, VkDeviceMemory, *const VkAllocationCallbacks);
pub type PFN_vkMapMemory = unsafe extern "system" fn(
    VkDevice,
    VkDeviceMemory,
    VkDeviceSize,
    VkDeviceSize,
    u32,
    *mut *mut c_void,
) -> VkResult;
pub type PFN_vkUnmapMemory = unsafe extern "system" fn(VkDevice, VkDeviceMemory);
pub type PFN_vkFlushMappedMemoryRanges =
    unsafe extern "system" fn(VkDevice, u32, *const VkMappedMemoryRange) -> VkResult;
pub type PFN_vkCreateImageView = unsafe extern "system" fn(
    VkDevice,
    *const VkImageViewCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkImageView,
) -> VkResult;
pub type PFN_vkDestroyImageView =
    unsafe extern "system" fn(VkDevice, VkImageView, *const VkAllocationCallbacks);
pub type PFN_vkCreateShaderModule = unsafe extern "system" fn(
    VkDevice,
    *const VkShaderModuleCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkShaderModule,
) -> VkResult;
pub type PFN_vkDestroyShaderModule =
    unsafe extern "system" fn(VkDevice, VkShaderModule, *const VkAllocationCallbacks);
pub type PFN_vkCreateDescriptorPool = unsafe extern "system" fn(
    VkDevice,
    *const VkDescriptorPoolCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkDescriptorPool,
) -> VkResult;
pub type PFN_vkDestroyDescriptorPool =
    unsafe extern "system" fn(VkDevice, VkDescriptorPool, *const VkAllocationCallbacks);
pub type PFN_vkCreateGraphicsPipelines = unsafe extern "system" fn(
    VkDevice,
    VkPipelineCache,
    u32,
    *const VkGraphicsPipelineCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkPipeline,
) -> VkResult;
pub type PFN_vkCreateComputePipelines = unsafe extern "system" fn(
    VkDevice,
    VkPipelineCache,
    u32,
    *const VkComputePipelineCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkPipeline,
) -> VkResult;
pub type PFN_vkDestroyPipeline =
    unsafe extern "system" fn(VkDevice, VkPipeline, *const VkAllocationCallbacks);
pub type PFN_vkCreatePipelineLayout = unsafe extern "system" fn(
    VkDevice,
    *const VkPipelineLayoutCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkPipelineLayout,
) -> VkResult;
pub type PFN_vkDestroyPipelineLayout =
    unsafe extern "system" fn(VkDevice, VkPipelineLayout, *const VkAllocationCallbacks);
pub type PFN_vkCreateDescriptorSetLayout = unsafe extern "system" fn(
    VkDevice,
    *const VkDescriptorSetLayoutCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkDescriptorSetLayout,
) -> VkResult;
pub type PFN_vkDestroyDescriptorSetLayout =
    unsafe extern "system" fn(VkDevice, VkDescriptorSetLayout, *const VkAllocationCallbacks);
pub type PFN_vkAllocateDescriptorSets = unsafe extern "system" fn(
    VkDevice,
    *const VkDescriptorSetAllocateInfo,
    *mut VkDescriptorSet,
) -> VkResult;
pub type PFN_vkFreeDescriptorSets =
    unsafe extern "system" fn(VkDevice, VkDescriptorPool, u32, *const VkDescriptorSet) -> VkResult;
pub type PFN_vkUpdateDescriptorSets =
    unsafe extern "system" fn(VkDevice, u32, *const VkWriteDescriptorSet, u32, *const c_void);
pub type PFN_vkCreateSampler = unsafe extern "system" fn(
    VkDevice,
    *const VkSamplerCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkSampler,
) -> VkResult;
pub type PFN_vkDestroySampler =
    unsafe extern "system" fn(VkDevice, VkSampler, *const VkAllocationCallbacks);
pub type PFN_vkCreateRenderPass = unsafe extern "system" fn(
    VkDevice,
    *const VkRenderPassCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkRenderPass,
) -> VkResult;
pub type PFN_vkDestroyRenderPass =
    unsafe extern "system" fn(VkDevice, VkRenderPass, *const VkAllocationCallbacks);
pub type PFN_vkCreateFramebuffer = unsafe extern "system" fn(
    VkDevice,
    *const VkFramebufferCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkFramebuffer,
) -> VkResult;
pub type PFN_vkDestroyFramebuffer =
    unsafe extern "system" fn(VkDevice, VkFramebuffer, *const VkAllocationCallbacks);
pub type PFN_vkCreateCommandPool = unsafe extern "system" fn(
    VkDevice,
    *const VkCommandPoolCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkCommandPool,
) -> VkResult;
pub type PFN_vkDestroyCommandPool =
    unsafe extern "system" fn(VkDevice, VkCommandPool, *const VkAllocationCallbacks);
pub type PFN_vkAllocateCommandBuffers = unsafe extern "system" fn(
    VkDevice,
    *const VkCommandBufferAllocateInfo,
    *mut VkCommandBuffer,
) -> VkResult;
pub type PFN_vkFreeCommandBuffers =
    unsafe extern "system" fn(VkDevice, VkCommandPool, u32, *const VkCommandBuffer);
pub type PFN_vkResetCommandPool =
    unsafe extern "system" fn(VkDevice, VkCommandPool, u32) -> VkResult;
pub type PFN_vkBeginCommandBuffer =
    unsafe extern "system" fn(VkCommandBuffer, *const VkCommandBufferBeginInfo) -> VkResult;
pub type PFN_vkEndCommandBuffer = unsafe extern "system" fn(VkCommandBuffer) -> VkResult;
pub type PFN_vkCmdBeginRenderPass =
    unsafe extern "system" fn(VkCommandBuffer, *const VkRenderPassBeginInfo, u32);
pub type PFN_vkCmdEndRenderPass = unsafe extern "system" fn(VkCommandBuffer);
pub type PFN_vkCmdBindPipeline = unsafe extern "system" fn(VkCommandBuffer, u32, VkPipeline);
pub type PFN_vkCmdSetViewport =
    unsafe extern "system" fn(VkCommandBuffer, u32, u32, *const VkViewport);
pub type PFN_vkCmdSetScissor =
    unsafe extern "system" fn(VkCommandBuffer, u32, u32, *const VkRect2D);
pub type PFN_vkCmdBindVertexBuffers =
    unsafe extern "system" fn(VkCommandBuffer, u32, u32, *const VkBuffer, *const VkDeviceSize);
pub type PFN_vkCmdBindIndexBuffer =
    unsafe extern "system" fn(VkCommandBuffer, VkBuffer, VkDeviceSize, u32);
pub type PFN_vkCmdDraw = unsafe extern "system" fn(VkCommandBuffer, u32, u32, u32, u32);
pub type PFN_vkCmdDrawIndexed = unsafe extern "system" fn(VkCommandBuffer, u32, u32, u32, i32, u32);
pub type PFN_vkCmdDrawIndirect =
    unsafe extern "system" fn(VkCommandBuffer, VkBuffer, VkDeviceSize, u32, u32);
pub type PFN_vkCmdDrawMeshTasksEXT = unsafe extern "system" fn(VkCommandBuffer, u32, u32, u32);
pub type PFN_vkCmdBindDescriptorSets = unsafe extern "system" fn(
    VkCommandBuffer,
    u32,
    VkPipelineLayout,
    u32,
    u32,
    *const VkDescriptorSet,
    u32,
    *const u32,
);
pub type PFN_vkCmdPushConstants =
    unsafe extern "system" fn(VkCommandBuffer, VkPipelineLayout, u32, u32, u32, *const c_void);
pub type PFN_vkCmdDispatch = unsafe extern "system" fn(VkCommandBuffer, u32, u32, u32);
pub type PFN_vkCmdDispatchIndirect =
    unsafe extern "system" fn(VkCommandBuffer, VkBuffer, VkDeviceSize);
pub type PFN_vkCmdCopyBuffer =
    unsafe extern "system" fn(VkCommandBuffer, VkBuffer, VkBuffer, u32, *const VkBufferCopy);
pub type PFN_vkCmdCopyImage =
    unsafe extern "system" fn(VkCommandBuffer, VkImage, u32, VkImage, u32, u32, *const VkImageCopy);
pub type PFN_vkCmdCopyBufferToImage = unsafe extern "system" fn(
    VkCommandBuffer,
    VkBuffer,
    VkImage,
    u32,
    u32,
    *const VkBufferImageCopy,
);
pub type PFN_vkCmdCopyImageToBuffer = unsafe extern "system" fn(
    VkCommandBuffer,
    VkImage,
    u32,
    VkBuffer,
    u32,
    *const VkBufferImageCopy,
);
pub type PFN_vkCmdPipelineBarrier = unsafe extern "system" fn(
    VkCommandBuffer,
    u32,
    u32,
    u32,
    u32,
    *const VkMemoryBarrier,
    u32,
    *const VkBufferMemoryBarrier,
    u32,
    *const VkImageMemoryBarrier,
);
pub type PFN_vkCreateFence = unsafe extern "system" fn(
    VkDevice,
    *const VkFenceCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkFence,
) -> VkResult;
pub type PFN_vkDestroyFence =
    unsafe extern "system" fn(VkDevice, VkFence, *const VkAllocationCallbacks);
pub type PFN_vkWaitForFences =
    unsafe extern "system" fn(VkDevice, u32, *const VkFence, u32, u64) -> VkResult;
pub type PFN_vkResetFences = unsafe extern "system" fn(VkDevice, u32, *const VkFence) -> VkResult;
pub type PFN_vkCreateSemaphore = unsafe extern "system" fn(
    VkDevice,
    *const VkSemaphoreCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkSemaphore,
) -> VkResult;
pub type PFN_vkDestroySemaphore =
    unsafe extern "system" fn(VkDevice, VkSemaphore, *const VkAllocationCallbacks);
pub type PFN_vkQueueSubmit =
    unsafe extern "system" fn(VkQueue, u32, *const VkSubmitInfo, VkFence) -> VkResult;
pub type PFN_vkQueuePresentKHR =
    unsafe extern "system" fn(VkQueue, *const VkPresentInfoKHR) -> VkResult;
pub type PFN_vkQueueWaitIdle = unsafe extern "system" fn(VkQueue) -> VkResult;
pub type PFN_vkCreateSwapchainKHR = unsafe extern "system" fn(
    VkDevice,
    *const VkSwapchainCreateInfoKHR,
    *const VkAllocationCallbacks,
    *mut VkSwapchainKHR,
) -> VkResult;
pub type PFN_vkDestroySwapchainKHR =
    unsafe extern "system" fn(VkDevice, VkSwapchainKHR, *const VkAllocationCallbacks);
pub type PFN_vkGetSwapchainImagesKHR =
    unsafe extern "system" fn(VkDevice, VkSwapchainKHR, *mut u32, *mut VkImage) -> VkResult;
pub type PFN_vkAcquireNextImageKHR = unsafe extern "system" fn(
    VkDevice,
    VkSwapchainKHR,
    u64,
    VkSemaphore,
    VkFence,
    *mut u32,
) -> VkResult;
pub type PFN_vkCreateQueryPool = unsafe extern "system" fn(
    VkDevice,
    *const VkQueryPoolCreateInfo,
    *const VkAllocationCallbacks,
    *mut VkQueryPool,
) -> VkResult;
pub type PFN_vkDestroyQueryPool =
    unsafe extern "system" fn(VkDevice, VkQueryPool, *const VkAllocationCallbacks);
pub type PFN_vkGetQueryPoolResults = unsafe extern "system" fn(
    VkDevice,
    VkQueryPool,
    u32,
    u32,
    usize,
    *mut c_void,
    VkDeviceSize,
    u32,
) -> VkResult;
pub type PFN_vkCmdResetQueryPool =
    unsafe extern "system" fn(VkCommandBuffer, VkQueryPool, u32, u32);
pub type PFN_vkCmdWriteTimestamp =
    unsafe extern "system" fn(VkCommandBuffer, u32, VkQueryPool, u32);
pub type PFN_vkGetBufferDeviceAddress =
    unsafe extern "system" fn(VkDevice, *const VkBufferDeviceAddressInfo) -> VkDeviceAddress;
pub type PFN_vkCreateAccelerationStructureKHR = unsafe extern "system" fn(
    VkDevice,
    *const VkAccelerationStructureCreateInfoKHR,
    *const VkAllocationCallbacks,
    *mut VkAccelerationStructureKHR,
) -> VkResult;
pub type PFN_vkDestroyAccelerationStructureKHR =
    unsafe extern "system" fn(VkDevice, VkAccelerationStructureKHR, *const VkAllocationCallbacks);
pub type PFN_vkGetAccelerationStructureBuildSizesKHR = unsafe extern "system" fn(
    VkDevice,
    u32,
    *const VkAccelerationStructureBuildGeometryInfoKHR,
    *const u32,
    *mut VkAccelerationStructureBuildSizesInfoKHR,
);
pub type PFN_vkGetAccelerationStructureDeviceAddressKHR =
    unsafe extern "system" fn(
        VkDevice,
        *const VkAccelerationStructureDeviceAddressInfoKHR,
    ) -> VkDeviceAddress;
pub type PFN_vkCmdBuildAccelerationStructuresKHR = unsafe extern "system" fn(
    VkCommandBuffer,
    u32,
    *const VkAccelerationStructureBuildGeometryInfoKHR,
    *const *const VkAccelerationStructureBuildRangeInfoKHR,
);

pub type PFN_vkVoidFunction = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VkMappedMemoryRange {
    pub sType: u32,
    pub pNext: *const c_void,
    pub memory: VkDeviceMemory,
    pub offset: VkDeviceSize,
    pub size: VkDeviceSize,
}

// MARK: - Native loader

/// Opens the platform Vulkan driver loader at runtime.
///
/// Swift called the `CVulkanHeaders` C shim, which did the same `dlopen` /
/// `LoadLibraryExW` once and returned `vkGetInstanceProcAddr`. Doing it in Rust
/// keeps the backend dependency-free: no link-time Vulkan library is required, so
/// a machine without a driver still builds and reports "unsupported backend".
pub struct VulkanLoader {
    get_instance_proc_addr: PFN_vkGetInstanceProcAddr,
    /// Kept open for the process lifetime: the resolved entry points live in it.
    _library: LibraryHandle,
}

// SAFETY: the loader holds a `dlopen`/`LoadLibrary` handle and one resolved
// function pointer. The library stays mapped for the loader's lifetime, and both
// `dlsym` and `GetProcAddress` results are immutable after loading, so sharing the
// loader across threads only ever reads plain data. `VulkanContext` needs this to
// sit behind an `Arc` for the completion thread.
unsafe impl Send for VulkanLoader {}
// SAFETY: same reasoning as the `Send` impl: every operation is a read of an
// immutable handle plus a call into the driver's own thread-safe resolver.
unsafe impl Sync for VulkanLoader {}

impl core::fmt::Debug for VulkanLoader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("VulkanLoader")
            .finish_non_exhaustive()
    }
}

impl VulkanLoader {
    /// Opens the driver loader, or `None` when the platform has no Vulkan.
    ///
    /// The macOS host never reaches this: the module is only compiled for the
    /// platforms whose driver ships a loader, and `cfg` keeps macOS on Metal.
    pub fn open() -> Option<Self> {
        let library = LibraryHandle::open()?;
        // SAFETY: the symbol name matches the Vulkan ABI, and the returned
        // pointer is only used as the declared function type.
        let symbol = unsafe { library.symbol(b"vkGetInstanceProcAddr\0") }?;
        // SAFETY: `vkGetInstanceProcAddr` has the documented signature; the
        // loader returns exactly that function.
        let get_instance_proc_addr: PFN_vkGetInstanceProcAddr =
            unsafe { core::mem::transmute_copy(&symbol) };
        Some(Self {
            get_instance_proc_addr,
            _library: library,
        })
    }

    /// Resolves an instance-level entry point through the driver loader.
    pub fn resolve(&self, instance: VkInstance, name: &core::ffi::CStr) -> PFN_vkVoidFunction {
        // SAFETY: the loader is open for the process lifetime and the name is a
        // NUL-terminated string.
        unsafe { (self.get_instance_proc_addr)(instance, name.as_ptr()) }
    }

    /// Casts a resolved pointer to a concrete function type.
    ///
    /// # Safety
    /// `name` must name the entry point that `F` describes; Vulkan's ABI is
    /// fixed, so a mismatch is a programming error rather than a runtime check.
    pub unsafe fn load<F: Copy>(&self, instance: VkInstance, name: &core::ffi::CStr) -> Option<F> {
        let pointer = self.resolve(instance, name);
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the caller asserted `F` matches `name`, and the pointer is
        // non-null, so the transmute yields a callable function pointer.
        Some(unsafe { core::mem::transmute_copy(&pointer) })
    }
}

/// A loaded dynamic library. Closed when the last reference drops, which for
/// this backend is process exit.
pub struct LibraryHandle {
    #[cfg(unix)]
    handle: *mut c_void,
    #[cfg(windows)]
    handle: *mut c_void,
}

impl LibraryHandle {
    #[cfg(unix)]
    fn open() -> Option<Self> {
        // Android ships `libvulkan.so`; desktop Linux publishes the versioned
        // soname. Try both so either packaging works.
        const CANDIDATES: [&[u8]; 2] = [b"libvulkan.so.1\0", b"libvulkan.so\0"];
        for candidate in CANDIDATES {
            // SAFETY: the name is a NUL-terminated C string and the flags are the
            // documented `dlopen` values.
            let handle = unsafe { dlopen(candidate.as_ptr().cast(), RTLD_NOW | RTLD_LOCAL) };
            if !handle.is_null() {
                return Some(Self { handle });
            }
        }
        None
    }

    #[cfg(windows)]
    fn open() -> Option<Self> {
        // The Vulkan loader on Windows is `vulkan-1.dll` in System32.
        const CANDIDATES: [&[u8]; 2] = [b"vulkan-1.dll\0", b"vulkan-1.dll\0"];
        let _ = CANDIDATES;
        // SAFETY: the name is a NUL-terminated C string.
        let handle = unsafe { LoadLibraryA(c"vulkan-1.dll".as_ptr().cast()) };
        if handle.is_null() {
            None
        } else {
            Some(Self { handle })
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn open() -> Option<Self> {
        None
    }

    /// Resolves a symbol by its NUL-terminated name.
    ///
    /// # Safety
    /// `name` must be NUL-terminated and name a symbol in this library.
    #[cfg(unix)]
    unsafe fn symbol(&self, name: &[u8]) -> Option<*const c_void> {
        // SAFETY: the loader keeps the library mapped for its lifetime and the
        // name is NUL-terminated.
        let pointer = unsafe { dlsym(self.handle, name.as_ptr().cast()) };
        if pointer.is_null() {
            None
        } else {
            Some(pointer.cast_const())
        }
    }

    /// # Safety
    /// `name` must be NUL-terminated and name a symbol in this library.
    #[cfg(windows)]
    unsafe fn symbol(&self, name: &[u8]) -> Option<*const c_void> {
        // SAFETY: the library handle is live and the name is NUL-terminated.
        let pointer = unsafe { GetProcAddress(self.handle, name.as_ptr().cast()) };
        if pointer.is_null() {
            None
        } else {
            Some(pointer.cast_const())
        }
    }

    /// # Safety
    /// `name` must be NUL-terminated.
    #[cfg(not(any(unix, windows)))]
    unsafe fn symbol(&self, _name: &[u8]) -> Option<*const c_void> {
        None
    }
}

impl Drop for LibraryHandle {
    #[cfg(unix)]
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: the handle came from `dlopen` and is only closed once.
            unsafe { dlclose(self.handle) };
        }
    }

    #[cfg(windows)]
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: the handle came from `LoadLibraryA` and is only freed once.
            unsafe { FreeLibrary(self.handle) };
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn drop(&mut self) {}
}

#[cfg(unix)]
const RTLD_NOW: i32 = 0x2;
#[cfg(unix)]
const RTLD_LOCAL: i32 = 0x0;

#[cfg(unix)]
unsafe extern "C" {
    fn dlopen(filename: *const i8, flag: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
}

#[cfg(windows)]
unsafe extern "system" {
    fn LoadLibraryA(name: *const i8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const i8) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

// MARK: - Resolved command tables
//
// Grouped exactly like Swift's `VulkanInstanceCommands` /
// `VulkanDeviceCoreCommands` / `VulkanDeviceResourceCommands` /
// `VulkanDeviceDrawCommands` / `VulkanDeviceSyncCommands`, so each struct stays
// readable and a missing entry point names its own group.

/// Every instance-level entry point the backend uses.
#[derive(Clone, Copy)]
pub struct InstanceCommands {
    pub destroy_instance: PFN_vkDestroyInstance,
    pub enumerate_physical_devices: PFN_vkEnumeratePhysicalDevices,
    pub get_physical_device_properties: PFN_vkGetPhysicalDeviceProperties,
    pub get_physical_device_properties2: PFN_vkGetPhysicalDeviceProperties2,
    pub get_physical_device_features2: PFN_vkGetPhysicalDeviceFeatures2,
    pub enumerate_device_extension_properties: PFN_vkEnumerateDeviceExtensionProperties,
    pub get_physical_device_queue_family_properties: PFN_vkGetPhysicalDeviceQueueFamilyProperties,
    pub get_physical_device_memory_properties: PFN_vkGetPhysicalDeviceMemoryProperties,
    pub get_image_format_properties: PFN_vkGetPhysicalDeviceImageFormatProperties,
    pub create_device: PFN_vkCreateDevice,
    pub destroy_device: PFN_vkDestroyDevice,
    pub get_device_queue: PFN_vkGetDeviceQueue,
    pub device_wait_idle: PFN_vkDeviceWaitIdle,
    pub destroy_surface: PFN_vkDestroySurfaceKHR,
    pub get_surface_support: PFN_vkGetPhysicalDeviceSurfaceSupportKHR,
    pub get_surface_capabilities: PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR,
    pub get_surface_formats: PFN_vkGetPhysicalDeviceSurfaceFormatsKHR,
    pub get_surface_present_modes: PFN_vkGetPhysicalDeviceSurfacePresentModesKHR,
}

impl InstanceCommands {
    /// Resolves the required instance entry points. A missing one returns `None`
    /// so the caller reports which name failed instead of aborting.
    pub fn load(loader: &VulkanLoader, instance: VkInstance) -> Option<Self> {
        // SAFETY: each name is the exact entry point its function type declares.
        unsafe {
            Some(Self {
                destroy_instance: loader.load(instance, c"vkDestroyInstance")?,
                enumerate_physical_devices: loader.load(instance, c"vkEnumeratePhysicalDevices")?,
                get_physical_device_properties: loader
                    .load(instance, c"vkGetPhysicalDeviceProperties")?,
                get_physical_device_properties2: loader
                    .load(instance, c"vkGetPhysicalDeviceProperties2")?,
                get_physical_device_features2: loader
                    .load(instance, c"vkGetPhysicalDeviceFeatures2")?,
                enumerate_device_extension_properties: loader
                    .load(instance, c"vkEnumerateDeviceExtensionProperties")?,
                get_physical_device_queue_family_properties: loader
                    .load(instance, c"vkGetPhysicalDeviceQueueFamilyProperties")?,
                get_physical_device_memory_properties: loader
                    .load(instance, c"vkGetPhysicalDeviceMemoryProperties")?,
                get_image_format_properties: loader
                    .load(instance, c"vkGetPhysicalDeviceImageFormatProperties")?,
                create_device: loader.load(instance, c"vkCreateDevice")?,
                destroy_device: loader.load(instance, c"vkDestroyDevice")?,
                get_device_queue: loader.load(instance, c"vkGetDeviceQueue")?,
                device_wait_idle: loader.load(instance, c"vkDeviceWaitIdle")?,
                destroy_surface: loader.load(instance, c"vkDestroySurfaceKHR")?,
                get_surface_support: loader
                    .load(instance, c"vkGetPhysicalDeviceSurfaceSupportKHR")?,
                get_surface_capabilities: loader
                    .load(instance, c"vkGetPhysicalDeviceSurfaceCapabilitiesKHR")?,
                get_surface_formats: loader
                    .load(instance, c"vkGetPhysicalDeviceSurfaceFormatsKHR")?,
                get_surface_present_modes: loader
                    .load(instance, c"vkGetPhysicalDeviceSurfacePresentModesKHR")?,
            })
        }
    }
}

/// Device entry points for memory, buffers, images, views, shaders and pools.
#[derive(Clone, Copy)]
pub struct CoreCommands {
    pub create_buffer: PFN_vkCreateBuffer,
    pub destroy_buffer: PFN_vkDestroyBuffer,
    pub get_buffer_memory_requirements: PFN_vkGetBufferMemoryRequirements,
    pub bind_buffer_memory: PFN_vkBindBufferMemory,
    pub create_image: PFN_vkCreateImage,
    pub destroy_image: PFN_vkDestroyImage,
    pub get_image_memory_requirements: PFN_vkGetImageMemoryRequirements,
    pub bind_image_memory: PFN_vkBindImageMemory,
    pub allocate_memory: PFN_vkAllocateMemory,
    pub free_memory: PFN_vkFreeMemory,
    pub map_memory: PFN_vkMapMemory,
    pub unmap_memory: PFN_vkUnmapMemory,
    pub flush_mapped_memory_ranges: PFN_vkFlushMappedMemoryRanges,
    pub create_image_view: PFN_vkCreateImageView,
    pub destroy_image_view: PFN_vkDestroyImageView,
    pub create_shader_module: PFN_vkCreateShaderModule,
    pub destroy_shader_module: PFN_vkDestroyShaderModule,
    pub create_descriptor_pool: PFN_vkCreateDescriptorPool,
    pub destroy_descriptor_pool: PFN_vkDestroyDescriptorPool,
}

impl CoreCommands {
    /// # Safety
    /// `device` must be a live logical device and `resolve` must resolve entry
    /// points for it.
    pub unsafe fn load(resolve: &impl Fn(&core::ffi::CStr) -> PFN_vkVoidFunction) -> Option<Self> {
        macro_rules! pick {
            ($name:literal) => {{
                let name = core::ffi::CStr::from_bytes_with_nul($name)
                    .expect("entry-point names are NUL-terminated");
                let pointer = resolve(name);
                if pointer.is_null() {
                    return None;
                }
                // SAFETY: the caller asserted the entry points belong to `device`.
                unsafe { core::mem::transmute_copy(&pointer) }
            }};
        }
        Some(Self {
            create_buffer: pick!(b"vkCreateBuffer\0"),
            destroy_buffer: pick!(b"vkDestroyBuffer\0"),
            get_buffer_memory_requirements: pick!(b"vkGetBufferMemoryRequirements\0"),
            bind_buffer_memory: pick!(b"vkBindBufferMemory\0"),
            create_image: pick!(b"vkCreateImage\0"),
            destroy_image: pick!(b"vkDestroyImage\0"),
            get_image_memory_requirements: pick!(b"vkGetImageMemoryRequirements\0"),
            bind_image_memory: pick!(b"vkBindImageMemory\0"),
            allocate_memory: pick!(b"vkAllocateMemory\0"),
            free_memory: pick!(b"vkFreeMemory\0"),
            map_memory: pick!(b"vkMapMemory\0"),
            unmap_memory: pick!(b"vkUnmapMemory\0"),
            flush_mapped_memory_ranges: pick!(b"vkFlushMappedMemoryRanges\0"),
            create_image_view: pick!(b"vkCreateImageView\0"),
            destroy_image_view: pick!(b"vkDestroyImageView\0"),
            create_shader_module: pick!(b"vkCreateShaderModule\0"),
            destroy_shader_module: pick!(b"vkDestroyShaderModule\0"),
            create_descriptor_pool: pick!(b"vkCreateDescriptorPool\0"),
            destroy_descriptor_pool: pick!(b"vkDestroyDescriptorPool\0"),
        })
    }
}

/// Query-pool entry points, used only for GPU timing.
#[derive(Clone, Copy)]
pub struct QueryCommands {
    pub create_query_pool: PFN_vkCreateQueryPool,
    pub destroy_query_pool: PFN_vkDestroyQueryPool,
    pub get_query_pool_results: PFN_vkGetQueryPoolResults,
    pub cmd_reset_query_pool: PFN_vkCmdResetQueryPool,
    pub cmd_write_timestamp: PFN_vkCmdWriteTimestamp,
}

impl QueryCommands {
    /// # Safety
    /// `resolve` must resolve entry points for a live logical device.
    pub unsafe fn load(resolve: &impl Fn(&core::ffi::CStr) -> PFN_vkVoidFunction) -> Option<Self> {
        macro_rules! pick {
            ($name:literal) => {{
                let name = core::ffi::CStr::from_bytes_with_nul($name)
                    .expect("entry-point names are NUL-terminated");
                let pointer = resolve(name);
                if pointer.is_null() {
                    return None;
                }
                // SAFETY: the caller asserted these entry points belong to a
                // live logical device.
                unsafe { core::mem::transmute_copy(&pointer) }
            }};
        }
        Some(Self {
            create_query_pool: pick!(b"vkCreateQueryPool\0"),
            destroy_query_pool: pick!(b"vkDestroyQueryPool\0"),
            get_query_pool_results: pick!(b"vkGetQueryPoolResults\0"),
            cmd_reset_query_pool: pick!(b"vkCmdResetQueryPool\0"),
            cmd_write_timestamp: pick!(b"vkCmdWriteTimestamp\0"),
        })
    }
}

/// Device entry points for pipelines, descriptors, samplers, passes and pools.
#[derive(Clone, Copy)]
pub struct ResourceCommands {
    pub queries: QueryCommands,
    pub create_graphics_pipelines: PFN_vkCreateGraphicsPipelines,
    pub create_compute_pipelines: PFN_vkCreateComputePipelines,
    pub destroy_pipeline: PFN_vkDestroyPipeline,
    pub create_pipeline_layout: PFN_vkCreatePipelineLayout,
    pub destroy_pipeline_layout: PFN_vkDestroyPipelineLayout,
    pub create_descriptor_set_layout: PFN_vkCreateDescriptorSetLayout,
    pub destroy_descriptor_set_layout: PFN_vkDestroyDescriptorSetLayout,
    pub allocate_descriptor_sets: PFN_vkAllocateDescriptorSets,
    pub free_descriptor_sets: PFN_vkFreeDescriptorSets,
    pub update_descriptor_sets: PFN_vkUpdateDescriptorSets,
    pub create_sampler: PFN_vkCreateSampler,
    pub destroy_sampler: PFN_vkDestroySampler,
    pub create_render_pass: PFN_vkCreateRenderPass,
    pub destroy_render_pass: PFN_vkDestroyRenderPass,
    pub create_framebuffer: PFN_vkCreateFramebuffer,
    pub destroy_framebuffer: PFN_vkDestroyFramebuffer,
    pub create_command_pool: PFN_vkCreateCommandPool,
    pub destroy_command_pool: PFN_vkDestroyCommandPool,
    pub allocate_command_buffers: PFN_vkAllocateCommandBuffers,
}

impl ResourceCommands {
    /// # Safety
    /// `resolve` must resolve entry points for a live logical device.
    pub unsafe fn load(resolve: &impl Fn(&core::ffi::CStr) -> PFN_vkVoidFunction) -> Option<Self> {
        macro_rules! pick {
            ($name:literal) => {{
                let name = core::ffi::CStr::from_bytes_with_nul($name)
                    .expect("entry-point names are NUL-terminated");
                let pointer = resolve(name);
                if pointer.is_null() {
                    return None;
                }
                // SAFETY: the caller asserted these entry points belong to a
                // live logical device.
                unsafe { core::mem::transmute_copy(&pointer) }
            }};
        }
        // SAFETY: forwarded from this function's contract.
        let queries = unsafe { QueryCommands::load(resolve) }?;
        Some(Self {
            queries,
            create_graphics_pipelines: pick!(b"vkCreateGraphicsPipelines\0"),
            create_compute_pipelines: pick!(b"vkCreateComputePipelines\0"),
            destroy_pipeline: pick!(b"vkDestroyPipeline\0"),
            create_pipeline_layout: pick!(b"vkCreatePipelineLayout\0"),
            destroy_pipeline_layout: pick!(b"vkDestroyPipelineLayout\0"),
            create_descriptor_set_layout: pick!(b"vkCreateDescriptorSetLayout\0"),
            destroy_descriptor_set_layout: pick!(b"vkDestroyDescriptorSetLayout\0"),
            allocate_descriptor_sets: pick!(b"vkAllocateDescriptorSets\0"),
            free_descriptor_sets: pick!(b"vkFreeDescriptorSets\0"),
            update_descriptor_sets: pick!(b"vkUpdateDescriptorSets\0"),
            create_sampler: pick!(b"vkCreateSampler\0"),
            destroy_sampler: pick!(b"vkDestroySampler\0"),
            create_render_pass: pick!(b"vkCreateRenderPass\0"),
            destroy_render_pass: pick!(b"vkDestroyRenderPass\0"),
            create_framebuffer: pick!(b"vkCreateFramebuffer\0"),
            destroy_framebuffer: pick!(b"vkDestroyFramebuffer\0"),
            create_command_pool: pick!(b"vkCreateCommandPool\0"),
            destroy_command_pool: pick!(b"vkDestroyCommandPool\0"),
            allocate_command_buffers: pick!(b"vkAllocateCommandBuffers\0"),
        })
    }
}

/// Device entry points for command-buffer recording.
#[derive(Clone, Copy)]
pub struct DrawCommands {
    pub free_command_buffers: PFN_vkFreeCommandBuffers,
    pub reset_command_pool: PFN_vkResetCommandPool,
    pub begin_command_buffer: PFN_vkBeginCommandBuffer,
    pub end_command_buffer: PFN_vkEndCommandBuffer,
    pub cmd_begin_render_pass: PFN_vkCmdBeginRenderPass,
    pub cmd_end_render_pass: PFN_vkCmdEndRenderPass,
    pub cmd_bind_pipeline: PFN_vkCmdBindPipeline,
    pub cmd_set_viewport: PFN_vkCmdSetViewport,
    pub cmd_set_scissor: PFN_vkCmdSetScissor,
    pub cmd_bind_vertex_buffers: PFN_vkCmdBindVertexBuffers,
    pub cmd_bind_index_buffer: PFN_vkCmdBindIndexBuffer,
    pub cmd_draw: PFN_vkCmdDraw,
    pub cmd_draw_indexed: PFN_vkCmdDrawIndexed,
    pub cmd_draw_indirect: PFN_vkCmdDrawIndirect,
    pub cmd_bind_descriptor_sets: PFN_vkCmdBindDescriptorSets,
    pub cmd_push_constants: PFN_vkCmdPushConstants,
    pub cmd_dispatch: PFN_vkCmdDispatch,
    pub cmd_dispatch_indirect: PFN_vkCmdDispatchIndirect,
    pub cmd_copy_buffer: PFN_vkCmdCopyBuffer,
    pub cmd_copy_image: PFN_vkCmdCopyImage,
    pub cmd_copy_buffer_to_image: PFN_vkCmdCopyBufferToImage,
    pub cmd_copy_image_to_buffer: PFN_vkCmdCopyImageToBuffer,
    pub cmd_pipeline_barrier: PFN_vkCmdPipelineBarrier,
}

impl DrawCommands {
    /// # Safety
    /// `resolve` must resolve entry points for a live logical device.
    pub unsafe fn load(resolve: &impl Fn(&core::ffi::CStr) -> PFN_vkVoidFunction) -> Option<Self> {
        macro_rules! pick {
            ($name:literal) => {{
                let name = core::ffi::CStr::from_bytes_with_nul($name)
                    .expect("entry-point names are NUL-terminated");
                let pointer = resolve(name);
                if pointer.is_null() {
                    return None;
                }
                // SAFETY: the caller asserted these entry points belong to a
                // live logical device.
                unsafe { core::mem::transmute_copy(&pointer) }
            }};
        }
        Some(Self {
            free_command_buffers: pick!(b"vkFreeCommandBuffers\0"),
            reset_command_pool: pick!(b"vkResetCommandPool\0"),
            begin_command_buffer: pick!(b"vkBeginCommandBuffer\0"),
            end_command_buffer: pick!(b"vkEndCommandBuffer\0"),
            cmd_begin_render_pass: pick!(b"vkCmdBeginRenderPass\0"),
            cmd_end_render_pass: pick!(b"vkCmdEndRenderPass\0"),
            cmd_bind_pipeline: pick!(b"vkCmdBindPipeline\0"),
            cmd_set_viewport: pick!(b"vkCmdSetViewport\0"),
            cmd_set_scissor: pick!(b"vkCmdSetScissor\0"),
            cmd_bind_vertex_buffers: pick!(b"vkCmdBindVertexBuffers\0"),
            cmd_bind_index_buffer: pick!(b"vkCmdBindIndexBuffer\0"),
            cmd_draw: pick!(b"vkCmdDraw\0"),
            cmd_draw_indexed: pick!(b"vkCmdDrawIndexed\0"),
            cmd_draw_indirect: pick!(b"vkCmdDrawIndirect\0"),
            cmd_bind_descriptor_sets: pick!(b"vkCmdBindDescriptorSets\0"),
            cmd_push_constants: pick!(b"vkCmdPushConstants\0"),
            cmd_dispatch: pick!(b"vkCmdDispatch\0"),
            cmd_dispatch_indirect: pick!(b"vkCmdDispatchIndirect\0"),
            cmd_copy_buffer: pick!(b"vkCmdCopyBuffer\0"),
            cmd_copy_image: pick!(b"vkCmdCopyImage\0"),
            cmd_copy_buffer_to_image: pick!(b"vkCmdCopyBufferToImage\0"),
            cmd_copy_image_to_buffer: pick!(b"vkCmdCopyImageToBuffer\0"),
            cmd_pipeline_barrier: pick!(b"vkCmdPipelineBarrier\0"),
        })
    }
}

/// Device entry points for synchronization, submission and the swapchain.
#[derive(Clone, Copy)]
pub struct SyncCommands {
    pub create_fence: PFN_vkCreateFence,
    pub destroy_fence: PFN_vkDestroyFence,
    pub wait_for_fences: PFN_vkWaitForFences,
    pub reset_fences: PFN_vkResetFences,
    pub create_semaphore: PFN_vkCreateSemaphore,
    pub destroy_semaphore: PFN_vkDestroySemaphore,
    pub queue_submit: PFN_vkQueueSubmit,
    pub queue_present: PFN_vkQueuePresentKHR,
    pub queue_wait_idle: PFN_vkQueueWaitIdle,
    pub create_swapchain: PFN_vkCreateSwapchainKHR,
    pub destroy_swapchain: PFN_vkDestroySwapchainKHR,
    pub get_swapchain_images: PFN_vkGetSwapchainImagesKHR,
    pub acquire_next_image: PFN_vkAcquireNextImageKHR,
}

impl SyncCommands {
    /// # Safety
    /// `resolve` must resolve entry points for a live logical device whose
    /// swapchain extension is enabled.
    pub unsafe fn load(resolve: &impl Fn(&core::ffi::CStr) -> PFN_vkVoidFunction) -> Option<Self> {
        macro_rules! pick {
            ($name:literal) => {{
                let name = core::ffi::CStr::from_bytes_with_nul($name)
                    .expect("entry-point names are NUL-terminated");
                let pointer = resolve(name);
                if pointer.is_null() {
                    return None;
                }
                // SAFETY: the caller asserted these entry points belong to a live
                // logical device with the swapchain extension enabled.
                unsafe { core::mem::transmute_copy(&pointer) }
            }};
        }
        Some(Self {
            create_fence: pick!(b"vkCreateFence\0"),
            destroy_fence: pick!(b"vkDestroyFence\0"),
            wait_for_fences: pick!(b"vkWaitForFences\0"),
            reset_fences: pick!(b"vkResetFences\0"),
            create_semaphore: pick!(b"vkCreateSemaphore\0"),
            destroy_semaphore: pick!(b"vkDestroySemaphore\0"),
            queue_submit: pick!(b"vkQueueSubmit\0"),
            queue_present: pick!(b"vkQueuePresentKHR\0"),
            queue_wait_idle: pick!(b"vkQueueWaitIdle\0"),
            create_swapchain: pick!(b"vkCreateSwapchainKHR\0"),
            destroy_swapchain: pick!(b"vkDestroySwapchainKHR\0"),
            get_swapchain_images: pick!(b"vkGetSwapchainImagesKHR\0"),
            acquire_next_image: pick!(b"vkAcquireNextImageKHR\0"),
        })
    }
}

/// Optional ray-tracing and mesh-shader entry points. Each is `Some` only when
/// the corresponding feature and extension are both present, so a caller can
/// test the pointer instead of the capability flag.
#[derive(Clone, Copy, Default)]
pub struct AdvancedCommands {
    pub buffer_device_address: Option<PFN_vkGetBufferDeviceAddress>,
    pub create_acceleration_structure: Option<PFN_vkCreateAccelerationStructureKHR>,
    pub destroy_acceleration_structure: Option<PFN_vkDestroyAccelerationStructureKHR>,
    pub acceleration_structure_build_sizes: Option<PFN_vkGetAccelerationStructureBuildSizesKHR>,
    pub acceleration_structure_address: Option<PFN_vkGetAccelerationStructureDeviceAddressKHR>,
    pub build_acceleration_structures: Option<PFN_vkCmdBuildAccelerationStructuresKHR>,
    pub draw_mesh_tasks: Option<PFN_vkCmdDrawMeshTasksEXT>,
    /// Driver-required scratch alignment, at least 256 bytes.
    pub scratch_alignment: VkDeviceSize,
}

impl AdvancedCommands {
    /// Resolves the optional entry points that `features` permits.
    ///
    /// # Safety
    /// `resolve` must resolve entry points for a live logical device.
    pub unsafe fn load(
        resolve: &impl Fn(&core::ffi::CStr) -> PFN_vkVoidFunction,
        features: &FeatureSupport,
        loader: &VulkanLoader,
        instance: VkInstance,
        physical_device: VkPhysicalDevice,
        instance_commands: &InstanceCommands,
    ) -> Self {
        let optional = |name: &core::ffi::CStr, enabled: bool| -> Option<PFN_vkVoidFunction> {
            if !enabled {
                return None;
            }
            let pointer = resolve(name);
            if pointer.is_null() {
                None
            } else {
                Some(pointer)
            }
        };
        macro_rules! pick_optional {
            ($name:literal, $enabled:expr) => {{
                let name = core::ffi::CStr::from_bytes_with_nul($name)
                    .expect("entry-point names are NUL-terminated");
                let pointer = optional(name, $enabled);
                // SAFETY: the caller asserted these entry points belong to a live
                // logical device, and each name matches its declared type.
                pointer.map(|pointer| unsafe { core::mem::transmute_copy(&pointer) })
            }};
        }
        let mut properties = VkPhysicalDeviceAccelerationStructurePropertiesKHR::default();
        let mut info = VkPhysicalDeviceProperties2 {
            pNext: (&mut properties as *mut VkPhysicalDeviceAccelerationStructurePropertiesKHR)
                .cast::<c_void>(),
            ..Default::default()
        };
        // SAFETY: `info` and its `pNext` chain outlive the call, which only
        // writes into the properties block.
        unsafe {
            if features.acceleration_structures {
                (instance_commands.get_physical_device_properties2)(physical_device, &mut info);
            }
        }
        let _ = (loader, instance);
        Self {
            buffer_device_address: pick_optional!(
                b"vkGetBufferDeviceAddress\0",
                features.acceleration_structures
            ),
            create_acceleration_structure: pick_optional!(
                b"vkCreateAccelerationStructureKHR\0",
                features.acceleration_structures
            ),
            destroy_acceleration_structure: pick_optional!(
                b"vkDestroyAccelerationStructureKHR\0",
                features.acceleration_structures
            ),
            acceleration_structure_build_sizes: pick_optional!(
                b"vkGetAccelerationStructureBuildSizesKHR\0",
                features.acceleration_structures
            ),
            acceleration_structure_address: pick_optional!(
                b"vkGetAccelerationStructureDeviceAddressKHR\0",
                features.acceleration_structures
            ),
            build_acceleration_structures: pick_optional!(
                b"vkCmdBuildAccelerationStructuresKHR\0",
                features.acceleration_structures
            ),
            draw_mesh_tasks: pick_optional!(b"vkCmdDrawMeshTasksEXT\0", features.mesh),
            scratch_alignment: (properties.minAccelerationStructureScratchOffsetAlignment as u64)
                .max(256),
        }
    }
}

/// Device features the backend enabled, plus what the driver reported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureSupport {
    pub spirv_version: u32,
    pub maintenance4: bool,
    pub non_solid_fill: bool,
    pub acceleration_structures: bool,
    pub ray_query: bool,
    pub mesh: bool,
    pub task: bool,
}

/// SPIR-V versions, as the loader reports them.
pub const SPIRV_1_3: u32 = 0x0001_0300;
pub const SPIRV_1_4: u32 = 0x0001_0400;
pub const SPIRV_1_5: u32 = 0x0001_0500;
pub const SPIRV_1_6: u32 = 0x0001_0600;

/// Packs a Vulkan API version the way the loader reports it.
pub const fn api_version(major: u32, minor: u32) -> u32 {
    (major << 22) | (minor << 12)
}
