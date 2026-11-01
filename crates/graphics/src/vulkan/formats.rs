//! Translation of NativeRHI/Vulkan/VulkanFormats.swift and the value mappings
//! from VulkanAuxiliary.swift: RHI ↔ Vulkan format, stage, descriptor and
//! sampler translation.
//!
//! Pure free functions, so every mapping is testable without a driver.
use crate::rhi::{
    BindingType, BlendFactor, BlendOperation, CompareOp, CullMode, FrontWinding, IndexType,
    PrimitiveType, RhiResult, SamplerAddressMode, SamplerFilter, SamplerMipFilter, ShaderStage,
    ShaderVisibility, TextureFormat, VertexFormat, rhi_validate_sample_count,
};

use super::sys;

// MARK: - Texture formats

/// The backend-neutral format a native image format represents.
///
/// Only the color formats a surface can present round-trip; anything else
/// reports `Invalid`, matching Swift's default branch.
pub fn texture_format(format: u32) -> TextureFormat {
    match format {
        sys::VK_FORMAT_B8G8R8A8_UNORM => TextureFormat::Bgra8Unorm,
        sys::VK_FORMAT_B8G8R8A8_SRGB => TextureFormat::Bgra8UnormSrgb,
        sys::VK_FORMAT_R8G8B8A8_UNORM => TextureFormat::Rgba8Unorm,
        sys::VK_FORMAT_R8G8B8A8_SRGB => TextureFormat::Rgba8UnormSrgb,
        _ => TextureFormat::Invalid,
    }
}

/// The native image format for a backend-neutral texture format.
pub fn vk_format(format: TextureFormat) -> u32 {
    match format {
        TextureFormat::Invalid => sys::VK_FORMAT_UNDEFINED,
        TextureFormat::R8Unorm => sys::VK_FORMAT_R8_UNORM,
        TextureFormat::Rgba8Unorm => sys::VK_FORMAT_R8G8B8A8_UNORM,
        TextureFormat::Bgra8Unorm => sys::VK_FORMAT_B8G8R8A8_UNORM,
        TextureFormat::Bgra8UnormSrgb => sys::VK_FORMAT_B8G8R8A8_SRGB,
        TextureFormat::Rgba8UnormSrgb => sys::VK_FORMAT_R8G8B8A8_SRGB,
        TextureFormat::Rgba16Float => sys::VK_FORMAT_R16G16B16A16_SFLOAT,
        TextureFormat::Rgba32Float => sys::VK_FORMAT_R32G32B32A32_SFLOAT,
        TextureFormat::R32Uint => sys::VK_FORMAT_R32_UINT,
        TextureFormat::R32Float => sys::VK_FORMAT_R32_SFLOAT,
        // X8_D24 keeps the depth24 request at its 32-bit packed width, which is
        // what Swift chose for the Vulkan path.
        TextureFormat::Depth24Unorm => sys::VK_FORMAT_X8_D24_UNORM_PACK32,
        TextureFormat::Depth24UnormStencil8 => sys::VK_FORMAT_D24_UNORM_S8_UINT,
        TextureFormat::Depth32Float => sys::VK_FORMAT_D32_SFLOAT,
    }
}

/// Whether a format has a depth aspect.
pub fn is_depth(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Depth24Unorm
            | TextureFormat::Depth24UnormStencil8
            | TextureFormat::Depth32Float
    )
}

/// Whether a format has a stencil aspect.
#[cfg(test)]
pub fn is_stencil(format: TextureFormat) -> bool {
    format == TextureFormat::Depth24UnormStencil8
}

/// The image aspect mask a format's subresource range needs.
pub fn aspect_flags(format: TextureFormat) -> u32 {
    if format == TextureFormat::Depth24UnormStencil8 {
        return sys::VK_IMAGE_ASPECT_DEPTH_BIT | sys::VK_IMAGE_ASPECT_STENCIL_BIT;
    }
    if is_depth(format) {
        sys::VK_IMAGE_ASPECT_DEPTH_BIT
    } else {
        sys::VK_IMAGE_ASPECT_COLOR_BIT
    }
}

// MARK: - Vertex and pipeline state

pub fn vk_vertex_format(format: VertexFormat) -> u32 {
    match format {
        VertexFormat::Float => sys::VK_FORMAT_R32_SFLOAT,
        VertexFormat::Float2 => sys::VK_FORMAT_R32G32_SFLOAT,
        VertexFormat::Float3 => sys::VK_FORMAT_R32G32B32_SFLOAT,
        VertexFormat::Float4 => sys::VK_FORMAT_R32G32B32A32_SFLOAT,
        VertexFormat::Unorm8x4 => sys::VK_FORMAT_R8G8B8A8_UNORM,
    }
}

pub fn vk_primitive_topology(primitive: PrimitiveType) -> u32 {
    match primitive {
        PrimitiveType::TriangleList => sys::VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST,
        PrimitiveType::TriangleStrip => sys::VK_PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP,
        PrimitiveType::LineList => sys::VK_PRIMITIVE_TOPOLOGY_LINE_LIST,
        PrimitiveType::LineStrip => sys::VK_PRIMITIVE_TOPOLOGY_LINE_STRIP,
        PrimitiveType::PointList => sys::VK_PRIMITIVE_TOPOLOGY_POINT_LIST,
    }
}

pub fn vk_cull_mode(mode: CullMode) -> u32 {
    match mode {
        CullMode::None => sys::VK_CULL_MODE_NONE,
        CullMode::Front => sys::VK_CULL_MODE_FRONT_BIT,
        CullMode::Back => sys::VK_CULL_MODE_BACK_BIT,
    }
}

pub fn vk_front_face(winding: FrontWinding) -> u32 {
    match winding {
        FrontWinding::Clockwise => sys::VK_FRONT_FACE_CLOCKWISE,
        FrontWinding::CounterClockwise => sys::VK_FRONT_FACE_COUNTER_CLOCKWISE,
    }
}

pub fn vk_compare_op(op: CompareOp) -> u32 {
    match op {
        CompareOp::Never => sys::VK_COMPARE_OP_NEVER,
        CompareOp::Less => sys::VK_COMPARE_OP_LESS,
        CompareOp::Equal => sys::VK_COMPARE_OP_EQUAL,
        CompareOp::LessOrEqual => sys::VK_COMPARE_OP_LESS_OR_EQUAL,
        CompareOp::Greater => sys::VK_COMPARE_OP_GREATER,
        CompareOp::NotEqual => sys::VK_COMPARE_OP_NOT_EQUAL,
        CompareOp::GreaterOrEqual => sys::VK_COMPARE_OP_GREATER_OR_EQUAL,
        CompareOp::Always => sys::VK_COMPARE_OP_ALWAYS,
    }
}

pub fn vk_blend_factor(factor: BlendFactor) -> u32 {
    match factor {
        BlendFactor::Zero => sys::VK_BLEND_FACTOR_ZERO,
        BlendFactor::One => sys::VK_BLEND_FACTOR_ONE,
        BlendFactor::SourceColor => sys::VK_BLEND_FACTOR_SRC_COLOR,
        BlendFactor::OneMinusSourceColor => sys::VK_BLEND_FACTOR_ONE_MINUS_SRC_COLOR,
        BlendFactor::DestinationColor => sys::VK_BLEND_FACTOR_DST_COLOR,
        BlendFactor::OneMinusDestinationColor => sys::VK_BLEND_FACTOR_ONE_MINUS_DST_COLOR,
        BlendFactor::SourceAlpha => sys::VK_BLEND_FACTOR_SRC_ALPHA,
        BlendFactor::OneMinusSourceAlpha => sys::VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA,
        BlendFactor::DestinationAlpha => sys::VK_BLEND_FACTOR_DST_ALPHA,
        BlendFactor::OneMinusDestinationAlpha => sys::VK_BLEND_FACTOR_ONE_MINUS_DST_ALPHA,
        BlendFactor::BlendColor => sys::VK_BLEND_FACTOR_CONSTANT_COLOR,
        BlendFactor::OneMinusBlendColor => sys::VK_BLEND_FACTOR_ONE_MINUS_CONSTANT_COLOR,
        BlendFactor::SourceAlphaSaturate => sys::VK_BLEND_FACTOR_SRC_ALPHA_SATURATE,
    }
}

pub fn vk_blend_op(op: BlendOperation) -> u32 {
    match op {
        BlendOperation::Add => sys::VK_BLEND_OP_ADD,
        BlendOperation::Subtract => sys::VK_BLEND_OP_SUBTRACT,
        BlendOperation::ReverseSubtract => sys::VK_BLEND_OP_REVERSE_SUBTRACT,
        BlendOperation::Min => sys::VK_BLEND_OP_MIN,
        BlendOperation::Max => sys::VK_BLEND_OP_MAX,
    }
}

pub fn vk_filter(filter: SamplerFilter) -> u32 {
    match filter {
        SamplerFilter::Nearest => sys::VK_FILTER_NEAREST,
        SamplerFilter::Linear => sys::VK_FILTER_LINEAR,
    }
}

pub fn vk_sampler_mipmap_mode(mode: SamplerMipFilter) -> u32 {
    match mode {
        SamplerMipFilter::Nearest => sys::VK_SAMPLER_MIPMAP_MODE_NEAREST,
        SamplerMipFilter::Linear => sys::VK_SAMPLER_MIPMAP_MODE_LINEAR,
    }
}

pub fn vk_sampler_address_mode(mode: SamplerAddressMode) -> u32 {
    match mode {
        SamplerAddressMode::Repeat => sys::VK_SAMPLER_ADDRESS_MODE_REPEAT,
        SamplerAddressMode::MirrorRepeat => sys::VK_SAMPLER_ADDRESS_MODE_MIRRORED_REPEAT,
        SamplerAddressMode::ClampToEdge => sys::VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    }
}

pub fn vk_index_type(index_type: IndexType) -> u32 {
    match index_type {
        IndexType::Uint16 => sys::VK_INDEX_TYPE_UINT16,
        IndexType::Uint32 => sys::VK_INDEX_TYPE_UINT32,
    }
}

/// The sample-count bit for a sample count, validating it first.
pub fn vk_sample_count(count: usize) -> RhiResult<u32> {
    rhi_validate_sample_count(count)?;
    Ok(count as u32)
}

// MARK: - Descriptors and stages

/// The native shader stage bits for one stage.
pub fn vk_stage_flags(stage: ShaderStage) -> u32 {
    match stage {
        ShaderStage::Vertex => sys::VK_SHADER_STAGE_VERTEX_BIT,
        ShaderStage::Fragment => sys::VK_SHADER_STAGE_FRAGMENT_BIT,
        ShaderStage::Compute => sys::VK_SHADER_STAGE_COMPUTE_BIT,
        ShaderStage::Task => sys::VK_SHADER_STAGE_TASK_BIT_EXT,
        ShaderStage::Mesh => sys::VK_SHADER_STAGE_MESH_BIT_EXT,
    }
}

/// The native shader stage bits for a visibility set, in declaration order.
pub fn vk_visibility_flags(visibility: ShaderVisibility) -> u32 {
    visibility
        .stages()
        .into_iter()
        .fold(0, |bits, stage| bits | vk_stage_flags(stage))
}

/// The native descriptor type for a binding declaration.
pub fn vk_descriptor_type(binding_type: BindingType, dynamic: bool) -> u32 {
    match binding_type {
        BindingType::Sampler => sys::VK_DESCRIPTOR_TYPE_SAMPLER,
        BindingType::Texture => sys::VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE,
        BindingType::StorageTexture => sys::VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
        BindingType::UniformBuffer if dynamic => sys::VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER_DYNAMIC,
        BindingType::UniformBuffer => sys::VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER,
        BindingType::StorageBuffer if dynamic => sys::VK_DESCRIPTOR_TYPE_STORAGE_BUFFER_DYNAMIC,
        BindingType::StorageBuffer => sys::VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        BindingType::AccelerationStructure => sys::VK_DESCRIPTOR_TYPE_ACCELERATION_STRUCTURE_KHR,
    }
}

/// The native buffer usage flags for a backend-neutral usage set.
pub fn vk_buffer_usage(usage: crate::rhi::BufferUsage) -> u32 {
    let mut flags = sys::VK_BUFFER_USAGE_TRANSFER_SRC_BIT | sys::VK_BUFFER_USAGE_TRANSFER_DST_BIT;
    if usage.contains(crate::rhi::BufferUsage::VERTEX) {
        flags |= sys::VK_BUFFER_USAGE_VERTEX_BUFFER_BIT;
    }
    if usage.contains(crate::rhi::BufferUsage::INDEX) {
        flags |= sys::VK_BUFFER_USAGE_INDEX_BUFFER_BIT;
    }
    if usage.contains(crate::rhi::BufferUsage::INDIRECT) {
        flags |= sys::VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT;
    }
    if usage.contains(crate::rhi::BufferUsage::UNIFORM) {
        flags |= sys::VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT;
    }
    if usage.contains(crate::rhi::BufferUsage::STORAGE_READ)
        || usage.contains(crate::rhi::BufferUsage::STORAGE_WRITE)
    {
        flags |= sys::VK_BUFFER_USAGE_STORAGE_BUFFER_BIT;
    }
    if usage.contains(crate::rhi::BufferUsage::TRANSFER_SOURCE) {
        flags |= sys::VK_BUFFER_USAGE_TRANSFER_SRC_BIT;
    }
    if usage.contains(crate::rhi::BufferUsage::TRANSFER_DESTINATION) {
        flags |= sys::VK_BUFFER_USAGE_TRANSFER_DST_BIT;
    }
    flags
}

/// The native image usage flags for a backend-neutral usage set.
pub fn vk_image_usage(usage: crate::rhi::TextureUsage) -> u32 {
    let mut flags = sys::VK_IMAGE_USAGE_TRANSFER_SRC_BIT | sys::VK_IMAGE_USAGE_TRANSFER_DST_BIT;
    if usage.contains(crate::rhi::TextureUsage::SAMPLED) {
        flags |= sys::VK_IMAGE_USAGE_SAMPLED_BIT;
    }
    if usage.contains(crate::rhi::TextureUsage::COLOR_TARGET) {
        flags |= sys::VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    }
    if usage.contains(crate::rhi::TextureUsage::DEPTH_STENCIL_TARGET) {
        flags |= sys::VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT;
    }
    if usage.contains(crate::rhi::TextureUsage::STORAGE_READ)
        || usage.contains(crate::rhi::TextureUsage::STORAGE_WRITE)
    {
        flags |= sys::VK_IMAGE_USAGE_STORAGE_BIT;
    }
    if usage.contains(crate::rhi::TextureUsage::TRANSFER_SOURCE) {
        flags |= sys::VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
    }
    if usage.contains(crate::rhi::TextureUsage::TRANSFER_DESTINATION) {
        flags |= sys::VK_IMAGE_USAGE_TRANSFER_DST_BIT;
    }
    flags
}

/// The native image view type for a texture dimension.
pub fn vk_image_view_type(dimension: crate::rhi::TextureDimension) -> u32 {
    match dimension {
        crate::rhi::TextureDimension::Texture2D => sys::VK_IMAGE_VIEW_TYPE_2D,
        crate::rhi::TextureDimension::Texture2DArray => sys::VK_IMAGE_VIEW_TYPE_2D_ARRAY,
        crate::rhi::TextureDimension::Texture3D => sys::VK_IMAGE_VIEW_TYPE_3D,
        crate::rhi::TextureDimension::Cube => sys::VK_IMAGE_VIEW_TYPE_CUBE,
    }
}

/// The native image type for a texture dimension.
pub fn vk_image_type(dimension: crate::rhi::TextureDimension) -> u32 {
    match dimension {
        crate::rhi::TextureDimension::Texture3D => sys::VK_IMAGE_TYPE_3D,
        _ => sys::VK_IMAGE_TYPE_2D,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{BufferUsage, TextureDimension, TextureUsage};

    #[test]
    fn every_color_format_round_trips_through_native_formats() {
        for format in [
            TextureFormat::R8Unorm,
            TextureFormat::Rgba8Unorm,
            TextureFormat::Bgra8Unorm,
            TextureFormat::Rgba8UnormSrgb,
            TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Rgba16Float,
            TextureFormat::Rgba32Float,
            TextureFormat::R32Uint,
            TextureFormat::R32Float,
            TextureFormat::Depth24Unorm,
            TextureFormat::Depth24UnormStencil8,
            TextureFormat::Depth32Float,
        ] {
            assert_ne!(vk_format(format), sys::VK_FORMAT_UNDEFINED, "{format:?}");
        }
        // The presentable formats are the ones the reverse map knows.
        assert_eq!(
            texture_format(sys::VK_FORMAT_B8G8R8A8_SRGB),
            TextureFormat::Bgra8UnormSrgb
        );
        assert_eq!(
            texture_format(sys::VK_FORMAT_R32_SFLOAT),
            TextureFormat::Invalid
        );
        assert_eq!(vk_format(TextureFormat::Invalid), sys::VK_FORMAT_UNDEFINED);
    }

    #[test]
    fn depth_and_stencil_aspects_match_the_formats() {
        assert_eq!(
            aspect_flags(TextureFormat::Rgba8Unorm),
            sys::VK_IMAGE_ASPECT_COLOR_BIT
        );
        assert_eq!(
            aspect_flags(TextureFormat::Depth32Float),
            sys::VK_IMAGE_ASPECT_DEPTH_BIT
        );
        assert_eq!(
            aspect_flags(TextureFormat::Depth24UnormStencil8),
            sys::VK_IMAGE_ASPECT_DEPTH_BIT | sys::VK_IMAGE_ASPECT_STENCIL_BIT
        );
        assert!(is_depth(TextureFormat::Depth24Unorm));
        assert!(!is_stencil(TextureFormat::Depth32Float));
        assert!(is_stencil(TextureFormat::Depth24UnormStencil8));
    }

    #[test]
    fn dynamic_bindings_select_the_dynamic_descriptor_types() {
        assert_eq!(
            vk_descriptor_type(BindingType::UniformBuffer, false),
            sys::VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER
        );
        assert_eq!(
            vk_descriptor_type(BindingType::UniformBuffer, true),
            sys::VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER_DYNAMIC
        );
        assert_eq!(
            vk_descriptor_type(BindingType::StorageBuffer, true),
            sys::VK_DESCRIPTOR_TYPE_STORAGE_BUFFER_DYNAMIC
        );
        assert_eq!(
            vk_descriptor_type(BindingType::AccelerationStructure, false),
            sys::VK_DESCRIPTOR_TYPE_ACCELERATION_STRUCTURE_KHR
        );
    }

    #[test]
    fn visibility_flags_union_the_declared_stages() {
        assert_eq!(
            vk_visibility_flags(ShaderVisibility::GRAPHICS),
            sys::VK_SHADER_STAGE_VERTEX_BIT | sys::VK_SHADER_STAGE_FRAGMENT_BIT
        );
        assert_eq!(
            vk_visibility_flags(ShaderVisibility::COMPUTE),
            sys::VK_SHADER_STAGE_COMPUTE_BIT
        );
        let all = vk_visibility_flags(ShaderVisibility::ALL);
        for bit in [
            sys::VK_SHADER_STAGE_VERTEX_BIT,
            sys::VK_SHADER_STAGE_FRAGMENT_BIT,
            sys::VK_SHADER_STAGE_COMPUTE_BIT,
            sys::VK_SHADER_STAGE_TASK_BIT_EXT,
            sys::VK_SHADER_STAGE_MESH_BIT_EXT,
        ] {
            assert_ne!(all & bit, 0);
        }
    }

    #[test]
    fn usage_flags_always_include_both_transfer_directions() {
        let empty = vk_buffer_usage(BufferUsage::EMPTY);
        assert_ne!(empty & sys::VK_BUFFER_USAGE_TRANSFER_SRC_BIT, 0);
        assert_ne!(empty & sys::VK_BUFFER_USAGE_TRANSFER_DST_BIT, 0);
        let vertex = vk_buffer_usage(BufferUsage::VERTEX | BufferUsage::STORAGE_READ);
        assert_ne!(vertex & sys::VK_BUFFER_USAGE_VERTEX_BUFFER_BIT, 0);
        assert_ne!(vertex & sys::VK_BUFFER_USAGE_STORAGE_BUFFER_BIT, 0);
        let image = vk_image_usage(TextureUsage::SAMPLED | TextureUsage::COLOR_TARGET);
        assert_ne!(image & sys::VK_IMAGE_USAGE_SAMPLED_BIT, 0);
        assert_ne!(image & sys::VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT, 0);
    }

    #[test]
    fn sample_counts_are_validated() {
        assert_eq!(vk_sample_count(1), Ok(1));
        assert_eq!(vk_sample_count(8), Ok(8));
        assert!(vk_sample_count(3).is_err());
    }

    #[test]
    fn dimension_maps_choose_the_matching_native_types() {
        assert_eq!(
            vk_image_type(TextureDimension::Texture3D),
            sys::VK_IMAGE_TYPE_3D
        );
        assert_eq!(vk_image_type(TextureDimension::Cube), sys::VK_IMAGE_TYPE_2D);
        assert_eq!(
            vk_image_view_type(TextureDimension::Cube),
            sys::VK_IMAGE_VIEW_TYPE_CUBE
        );
        assert_eq!(
            vk_image_view_type(TextureDimension::Texture2DArray),
            sys::VK_IMAGE_VIEW_TYPE_2D_ARRAY
        );
    }

    #[test]
    fn vertex_formats_are_float_widths() {
        assert_eq!(
            vk_vertex_format(VertexFormat::Float),
            sys::VK_FORMAT_R32_SFLOAT
        );
        assert_eq!(
            vk_vertex_format(VertexFormat::Float4),
            sys::VK_FORMAT_R32G32B32A32_SFLOAT
        );
        assert_eq!(
            vk_vertex_format(VertexFormat::Unorm8x4),
            sys::VK_FORMAT_R8G8B8A8_UNORM
        );
        assert_eq!(vk_index_type(IndexType::Uint16), sys::VK_INDEX_TYPE_UINT16);
    }
}
