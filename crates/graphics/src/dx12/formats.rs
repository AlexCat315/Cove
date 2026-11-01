//! Translation of NativeRHI/DX12/DX12Formats.swift: RHI → D3D12/DXGI value
//! mappings.
//!
//! Swift used small `UInt32` bridge codes (`GRHI_*`) so the C++ layer could switch
//! on them; the Rust port maps straight to the real D3D12 enumerants, which removes
//! one translation layer without changing behaviour.
use crate::rhi::{
    BlendFactor, BlendOperation, CompareOp, CullMode, FillMode, FrontWinding, PrimitiveType,
    SamplerAddressMode, TextureDimension, TextureFormat, VertexFormat, rhi_validate_sample_count,
};

use super::sys;

// MARK: - Formats

/// The DXGI format for a backend-neutral texture format.
pub fn dxgi_format(format: TextureFormat) -> u32 {
    match format {
        TextureFormat::Invalid => sys::DXGI_FORMAT_UNKNOWN,
        TextureFormat::R8Unorm => sys::DXGI_FORMAT_R8_UNORM,
        TextureFormat::Rgba8Unorm => sys::DXGI_FORMAT_R8G8B8A8_UNORM,
        TextureFormat::Bgra8Unorm => sys::DXGI_FORMAT_B8G8R8A8_UNORM,
        TextureFormat::Bgra8UnormSrgb => sys::DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        TextureFormat::Rgba8UnormSrgb => sys::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        TextureFormat::Rgba16Float => sys::DXGI_FORMAT_R16G16B16A16_FLOAT,
        TextureFormat::Rgba32Float => sys::DXGI_FORMAT_R32G32B32A32_FLOAT,
        TextureFormat::R32Uint => sys::DXGI_FORMAT_R32_UINT,
        TextureFormat::R32Float => sys::DXGI_FORMAT_R32_FLOAT,
        // D3D12 has no depth24-only format, so 24-bit depth requests use the
        // combined 24+8 target, matching Swift's bridge mapping.
        TextureFormat::Depth24Unorm => sys::DXGI_FORMAT_D24_UNORM_S8_UINT,
        TextureFormat::Depth24UnormStencil8 => sys::DXGI_FORMAT_D24_UNORM_S8_UINT,
        TextureFormat::Depth32Float => sys::DXGI_FORMAT_D32_FLOAT,
    }
}

/// The backend-neutral format a DXGI format represents.
pub fn texture_format(format: u32) -> TextureFormat {
    match format {
        sys::DXGI_FORMAT_R8_UNORM => TextureFormat::R8Unorm,
        sys::DXGI_FORMAT_R8G8B8A8_UNORM => TextureFormat::Rgba8Unorm,
        sys::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => TextureFormat::Rgba8UnormSrgb,
        sys::DXGI_FORMAT_B8G8R8A8_UNORM => TextureFormat::Bgra8Unorm,
        sys::DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => TextureFormat::Bgra8UnormSrgb,
        sys::DXGI_FORMAT_R16G16B16A16_FLOAT => TextureFormat::Rgba16Float,
        sys::DXGI_FORMAT_R32G32B32A32_FLOAT => TextureFormat::Rgba32Float,
        sys::DXGI_FORMAT_R32_UINT => TextureFormat::R32Uint,
        sys::DXGI_FORMAT_R32_FLOAT => TextureFormat::R32Float,
        sys::DXGI_FORMAT_D32_FLOAT => TextureFormat::Depth32Float,
        sys::DXGI_FORMAT_D24_UNORM_S8_UINT => TextureFormat::Depth24UnormStencil8,
        _ => TextureFormat::Invalid,
    }
}

pub fn dxgi_vertex_format(format: VertexFormat) -> u32 {
    match format {
        VertexFormat::Float => sys::DXGI_FORMAT_R32_FLOAT,
        VertexFormat::Float2 => sys::DXGI_FORMAT_R32G32_FLOAT,
        VertexFormat::Float3 => sys::DXGI_FORMAT_R32G32B32_FLOAT,
        VertexFormat::Float4 => sys::DXGI_FORMAT_R32G32B32A32_FLOAT,
        VertexFormat::Unorm8x4 => sys::DXGI_FORMAT_R8G8B8A8_UNORM,
    }
}

// MARK: - Pipeline state

pub fn comparison_function(op: CompareOp) -> u32 {
    match op {
        CompareOp::Never => sys::D3D12_COMPARISON_FUNC_NEVER,
        CompareOp::Less => sys::D3D12_COMPARISON_FUNC_LESS,
        CompareOp::Equal => sys::D3D12_COMPARISON_FUNC_EQUAL,
        CompareOp::LessOrEqual => sys::D3D12_COMPARISON_FUNC_LESS_EQUAL,
        CompareOp::Greater => sys::D3D12_COMPARISON_FUNC_GREATER,
        CompareOp::NotEqual => sys::D3D12_COMPARISON_FUNC_NOT_EQUAL,
        CompareOp::GreaterOrEqual => sys::D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        CompareOp::Always => sys::D3D12_COMPARISON_FUNC_ALWAYS,
    }
}

pub fn blend_factor(factor: BlendFactor) -> u32 {
    match factor {
        BlendFactor::Zero => sys::D3D12_BLEND_ZERO,
        BlendFactor::One => sys::D3D12_BLEND_ONE,
        BlendFactor::SourceColor => sys::D3D12_BLEND_SRC_COLOR,
        BlendFactor::OneMinusSourceColor => sys::D3D12_BLEND_INV_SRC_COLOR,
        BlendFactor::DestinationColor => sys::D3D12_BLEND_DEST_COLOR,
        BlendFactor::OneMinusDestinationColor => sys::D3D12_BLEND_INV_DEST_COLOR,
        BlendFactor::SourceAlpha => sys::D3D12_BLEND_SRC_ALPHA,
        BlendFactor::OneMinusSourceAlpha => sys::D3D12_BLEND_INV_SRC_ALPHA,
        BlendFactor::DestinationAlpha => sys::D3D12_BLEND_DEST_ALPHA,
        BlendFactor::OneMinusDestinationAlpha => sys::D3D12_BLEND_INV_DEST_ALPHA,
        BlendFactor::BlendColor => sys::D3D12_BLEND_BLEND_FACTOR,
        BlendFactor::OneMinusBlendColor => sys::D3D12_BLEND_INV_BLEND_FACTOR,
        BlendFactor::SourceAlphaSaturate => sys::D3D12_BLEND_SRC_ALPHA_SAT,
    }
}

pub fn blend_operation(op: BlendOperation) -> u32 {
    match op {
        BlendOperation::Add => sys::D3D12_BLEND_OP_ADD,
        BlendOperation::Subtract => sys::D3D12_BLEND_OP_SUBTRACT,
        BlendOperation::ReverseSubtract => sys::D3D12_BLEND_OP_REV_SUBTRACT,
        BlendOperation::Min => sys::D3D12_BLEND_OP_MIN,
        BlendOperation::Max => sys::D3D12_BLEND_OP_MAX,
    }
}

pub fn fill_mode(mode: FillMode) -> u32 {
    match mode {
        FillMode::Fill => sys::D3D12_FILL_MODE_SOLID,
        FillMode::Lines => sys::D3D12_FILL_MODE_WIREFRAME,
    }
}

pub fn cull_mode(mode: CullMode) -> u32 {
    match mode {
        CullMode::None => sys::D3D12_CULL_MODE_NONE,
        CullMode::Front => sys::D3D12_CULL_MODE_FRONT,
        CullMode::Back => sys::D3D12_CULL_MODE_BACK,
    }
}

pub fn front_counter_clockwise(winding: FrontWinding) -> u32 {
    match winding {
        FrontWinding::CounterClockwise => 1,
        FrontWinding::Clockwise => 0,
    }
}

pub fn primitive_topology(primitive: PrimitiveType) -> u32 {
    match primitive {
        PrimitiveType::TriangleList => sys::D3D12_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        PrimitiveType::TriangleStrip => sys::D3D12_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        PrimitiveType::LineList => sys::D3D12_PRIMITIVE_TOPOLOGY_LINELIST,
        PrimitiveType::LineStrip => sys::D3D12_PRIMITIVE_TOPOLOGY_LINESTRIP,
        PrimitiveType::PointList => sys::D3D12_PRIMITIVE_TOPOLOGY_POINTLIST,
    }
}

/// D3D12 groups topologies into coarse "primitive topology types" for the pipeline
/// state; the exact topology is set on the command list.
pub fn primitive_topology_type(primitive: PrimitiveType) -> u32 {
    match primitive {
        PrimitiveType::TriangleList | PrimitiveType::TriangleStrip => 3,
        PrimitiveType::LineList | PrimitiveType::LineStrip => 2,
        PrimitiveType::PointList => 1,
    }
}

pub fn sampler_address_mode(mode: SamplerAddressMode) -> u32 {
    // D3D12_TEXTURE_ADDRESS_MODE: WRAP=1, MIRROR=2, CLAMP=3.
    match mode {
        SamplerAddressMode::Repeat => 1,
        SamplerAddressMode::MirrorRepeat => 2,
        SamplerAddressMode::ClampToEdge => 3,
    }
}

pub fn resource_dimension(dimension: TextureDimension) -> u32 {
    match dimension {
        TextureDimension::Texture2D | TextureDimension::Cube | TextureDimension::Texture2DArray => {
            sys::D3D12_RESOURCE_DIMENSION_TEXTURE2D
        }
        TextureDimension::Texture3D => sys::D3D12_RESOURCE_DIMENSION_TEXTURE3D,
    }
}

/// D3D12 array size encoding: `DepthOrArraySize` is the layer count, except for a
/// cube, whose six faces are its layers.
pub fn array_size(dimension: TextureDimension, layers: usize) -> u16 {
    let count = if dimension == TextureDimension::Cube {
        6
    } else {
        layers.max(1)
    };
    count as u16
}

pub fn sample_count(count: usize) -> crate::rhi::RhiResult<u32> {
    rhi_validate_sample_count(count)?;
    Ok(count as u32)
}

/// The `D3D12_RESOURCE_STATES` a backend-neutral resource state implies.
pub fn resource_state(state: crate::rhi::ResourceState) -> u32 {
    use crate::rhi::ResourceState;
    let mut flags = 0;
    if state.contains(ResourceState::CONSTANT_BUFFER) {
        flags |= sys::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER;
    }
    if state.contains(ResourceState::VERTEX_BUFFER) {
        flags |= sys::D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER;
    }
    if state.contains(ResourceState::INDEX_BUFFER) {
        flags |= sys::D3D12_RESOURCE_STATE_INDEX_BUFFER;
    }
    if state.contains(ResourceState::INDIRECT_ARGUMENT) {
        flags |= sys::D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT;
    }
    if state.contains(ResourceState::SHADER_RESOURCE) {
        flags |= sys::D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE
            | sys::D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
    }
    if state.contains(ResourceState::UNORDERED_ACCESS) {
        flags |= sys::D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    }
    if state.contains(ResourceState::RENDER_TARGET) {
        flags |= sys::D3D12_RESOURCE_STATE_RENDER_TARGET;
    }
    if state.contains(ResourceState::DEPTH_WRITE) {
        flags |= sys::D3D12_RESOURCE_STATE_DEPTH_WRITE;
    }
    if state.contains(ResourceState::DEPTH_READ) {
        flags |= sys::D3D12_RESOURCE_STATE_DEPTH_READ;
    }
    if state.contains(ResourceState::COPY_DESTINATION) {
        flags |= sys::D3D12_RESOURCE_STATE_COPY_DEST;
    }
    if state.contains(ResourceState::COPY_SOURCE) {
        flags |= sys::D3D12_RESOURCE_STATE_COPY_SOURCE;
    }
    if state.contains(ResourceState::PRESENT) {
        flags |= sys::D3D12_RESOURCE_STATE_PRESENT;
    }
    if state.contains(ResourceState::ACCELERATION_STRUCTURE_READ)
        || state.contains(ResourceState::ACCELERATION_STRUCTURE_WRITE)
    {
        flags |= sys::D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE;
    }
    if flags == 0 {
        sys::D3D12_RESOURCE_STATE_COMMON
    } else {
        flags
    }
}

/// The DXGI format a swapchain's back buffers use.
pub fn swapchain_format(format: TextureFormat) -> u32 {
    // Flip-model swapchains require an untyped-color format; the sRGB variant is
    // preferred when the caller asked for a linear-sRGB surface.
    match format {
        TextureFormat::Bgra8UnormSrgb => sys::DXGI_FORMAT_B8G8R8A8_UNORM,
        TextureFormat::Rgba8UnormSrgb => sys::DXGI_FORMAT_R8G8B8A8_UNORM,
        other => dxgi_format(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::ResourceState;

    #[test]
    fn every_format_maps_to_a_real_dxgi_format() {
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
            assert_ne!(dxgi_format(format), sys::DXGI_FORMAT_UNKNOWN, "{format:?}");
        }
        assert_eq!(
            dxgi_format(TextureFormat::Invalid),
            sys::DXGI_FORMAT_UNKNOWN
        );
    }

    #[test]
    fn color_formats_round_trip() {
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
        ] {
            assert_eq!(texture_format(dxgi_format(format)), format, "{format:?}");
        }
        // depth24Unorm has no distinct DXGI format, so it reports the combined one.
        assert_eq!(
            dxgi_format(TextureFormat::Depth24Unorm),
            sys::DXGI_FORMAT_D24_UNORM_S8_UINT
        );
        assert_eq!(
            texture_format(sys::DXGI_FORMAT_D24_UNORM_S8_UINT),
            TextureFormat::Depth24UnormStencil8
        );
    }

    #[test]
    fn blend_mappings_follow_the_d3d12_enumerants() {
        assert_eq!(blend_factor(BlendFactor::Zero), 1);
        assert_eq!(blend_factor(BlendFactor::One), 2);
        assert_eq!(blend_factor(BlendFactor::SourceAlpha), 5);
        assert_eq!(blend_factor(BlendFactor::OneMinusSourceAlpha), 6);
        assert_eq!(blend_operation(BlendOperation::Add), 1);
        assert_eq!(blend_operation(BlendOperation::Max), 5);
        assert_eq!(comparison_function(CompareOp::Always), 8);
        assert_eq!(comparison_function(CompareOp::Less), 2);
    }

    #[test]
    fn resource_states_fold_into_the_matching_bits() {
        assert_eq!(
            resource_state(ResourceState::EMPTY),
            sys::D3D12_RESOURCE_STATE_COMMON
        );
        assert_eq!(
            resource_state(ResourceState::RENDER_TARGET),
            sys::D3D12_RESOURCE_STATE_RENDER_TARGET
        );
        assert_eq!(
            resource_state(ResourceState::COPY_DESTINATION),
            sys::D3D12_RESOURCE_STATE_COPY_DEST
        );
        // Vertex and constant buffers share one D3D12 state bit.
        assert_eq!(
            resource_state(ResourceState::VERTEX_BUFFER),
            resource_state(ResourceState::CONSTANT_BUFFER)
        );
        let shader = resource_state(ResourceState::SHADER_RESOURCE);
        assert_ne!(shader & sys::D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, 0);
        assert_ne!(
            shader & sys::D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            0
        );
    }

    #[test]
    fn cube_textures_always_use_six_layers() {
        assert_eq!(array_size(TextureDimension::Cube, 1), 6);
        assert_eq!(array_size(TextureDimension::Texture2D, 4), 4);
        assert_eq!(array_size(TextureDimension::Texture2D, 0), 1);
        assert_eq!(array_size(TextureDimension::Texture3D, 8), 8);
    }

    #[test]
    fn swapchain_formats_stay_untyped_for_flip_model() {
        assert_eq!(
            swapchain_format(TextureFormat::Bgra8UnormSrgb),
            sys::DXGI_FORMAT_B8G8R8A8_UNORM
        );
        assert_eq!(
            swapchain_format(TextureFormat::Rgba8UnormSrgb),
            sys::DXGI_FORMAT_R8G8B8A8_UNORM
        );
        assert_eq!(
            swapchain_format(TextureFormat::Rgba8Unorm),
            sys::DXGI_FORMAT_R8G8B8A8_UNORM
        );
        assert!(sample_count(3).is_err());
        assert_eq!(sample_count(4), Ok(4));
    }

    #[test]
    fn topology_types_group_the_primitive_kinds() {
        assert_eq!(primitive_topology_type(PrimitiveType::TriangleList), 3);
        assert_eq!(primitive_topology_type(PrimitiveType::LineStrip), 2);
        assert_eq!(primitive_topology_type(PrimitiveType::PointList), 1);
        assert_eq!(primitive_topology(PrimitiveType::TriangleList), 4);
    }
}
