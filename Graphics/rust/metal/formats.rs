//! Translation of NativeRHI/Metal/MetalFormats.swift: RHI → Metal value
//! mappings.
//!
//! Pure free functions: every backend-neutral descriptor enum is translated to
//! the corresponding Metal enum here. Keeping the mappings out of the device
//! type keeps the device small and makes the translation trivially testable.
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLCompareFunction, MTLCullMode, MTLIndexType,
    MTLLoadAction, MTLPixelFormat, MTLPrimitiveType, MTLSamplerAddressMode, MTLSamplerMinMagFilter,
    MTLSamplerMipFilter, MTLTextureType, MTLTextureUsage, MTLTriangleFillMode, MTLVertexFormat,
    MTLVertexStepFunction, MTLWinding,
};

use crate::rhi::{
    BlendFactor, BlendOperation, ColorLoadAction, CompareOp, CullMode, DepthLoadAction, FillMode,
    FrontWinding, IndexType, PrimitiveType, SamplerAddressMode, SamplerFilter, SamplerMipFilter,
    TextureDimension, TextureFormat, TextureUsage, VertexFormat, VertexInputRate,
};

// MARK: Pixel formats

pub fn pixel_format(format: TextureFormat) -> MTLPixelFormat {
    match format {
        TextureFormat::Invalid => MTLPixelFormat::Invalid,
        TextureFormat::R8Unorm => MTLPixelFormat::R8Unorm,
        TextureFormat::Rgba8Unorm => MTLPixelFormat::RGBA8Unorm,
        TextureFormat::Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
        TextureFormat::Bgra8UnormSrgb => MTLPixelFormat::BGRA8Unorm_sRGB,
        TextureFormat::Rgba8UnormSrgb => MTLPixelFormat::RGBA8Unorm_sRGB,
        TextureFormat::Rgba16Float => MTLPixelFormat::RGBA16Float,
        TextureFormat::Rgba32Float => MTLPixelFormat::RGBA32Float,
        TextureFormat::R32Uint => MTLPixelFormat::R32Uint,
        TextureFormat::R32Float => MTLPixelFormat::R32Float,
        // Apple Silicon has no depth24-only format; d32Float is the depth target.
        TextureFormat::Depth24Unorm => MTLPixelFormat::Depth32Float,
        // depth24Unorm_stencil8 is unsupported on Apple Silicon; use the combined
        // d32f + s8 format that every Apple GPU implements.
        TextureFormat::Depth24UnormStencil8 => MTLPixelFormat::Depth32Float_Stencil8,
        TextureFormat::Depth32Float => MTLPixelFormat::Depth32Float,
    }
}

/// Bytes per pixel for a format, used to validate upload/readback strides.
pub fn bytes_per_pixel(format: TextureFormat) -> usize {
    match format {
        TextureFormat::R8Unorm => 1,
        TextureFormat::Rgba8Unorm
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb
        | TextureFormat::Rgba8UnormSrgb => 4,
        TextureFormat::Rgba16Float => 8,
        TextureFormat::Rgba32Float => 16,
        TextureFormat::R32Uint | TextureFormat::R32Float => 4,
        TextureFormat::Depth24Unorm | TextureFormat::Depth32Float => 4,
        TextureFormat::Depth24UnormStencil8 => 5,
        TextureFormat::Invalid => 4,
    }
}

pub fn color_format(format: MTLPixelFormat) -> TextureFormat {
    match format {
        MTLPixelFormat::R8Unorm => TextureFormat::R8Unorm,
        MTLPixelFormat::RGBA8Unorm => TextureFormat::Rgba8Unorm,
        MTLPixelFormat::BGRA8Unorm => TextureFormat::Bgra8Unorm,
        MTLPixelFormat::RGBA8Unorm_sRGB => TextureFormat::Rgba8UnormSrgb,
        MTLPixelFormat::BGRA8Unorm_sRGB => TextureFormat::Bgra8UnormSrgb,
        MTLPixelFormat::RGBA16Float => TextureFormat::Rgba16Float,
        MTLPixelFormat::RGBA32Float => TextureFormat::Rgba32Float,
        MTLPixelFormat::R32Uint => TextureFormat::R32Uint,
        MTLPixelFormat::R32Float => TextureFormat::R32Float,
        _ => TextureFormat::Invalid,
    }
}

// MARK: Texture dimension / usage

pub fn texture_type(dimension: TextureDimension) -> MTLTextureType {
    match dimension {
        TextureDimension::Texture2D => MTLTextureType::Type2D,
        TextureDimension::Texture3D => MTLTextureType::Type3D,
        TextureDimension::Cube => MTLTextureType::TypeCube,
        TextureDimension::Texture2DArray => MTLTextureType::Type2DArray,
    }
}

pub fn texture_usage(usage: TextureUsage) -> MTLTextureUsage {
    let mut result = MTLTextureUsage::Unknown;
    if usage.contains(TextureUsage::SAMPLED) {
        result |= MTLTextureUsage::ShaderRead;
    }
    if usage.contains(TextureUsage::COLOR_TARGET) {
        result |= MTLTextureUsage::RenderTarget;
    }
    // Depth/stencil attachments are render-target attachments in Metal.
    if usage.contains(TextureUsage::DEPTH_STENCIL_TARGET) {
        result |= MTLTextureUsage::RenderTarget;
    }
    if usage.contains(TextureUsage::STORAGE_READ) {
        result |= MTLTextureUsage::ShaderRead;
    }
    if usage.contains(TextureUsage::STORAGE_WRITE) {
        result |= MTLTextureUsage::ShaderWrite;
    }
    // Blit source needs shader-read on modern Metal; destination is implicit.
    if usage.contains(TextureUsage::TRANSFER_SOURCE) {
        result |= MTLTextureUsage::ShaderRead;
    }
    if usage.contains(TextureUsage::PRESENT) {
        result |= MTLTextureUsage::RenderTarget;
    }
    result
}

// MARK: Vertex layout

pub fn vertex_format(format: VertexFormat) -> MTLVertexFormat {
    match format {
        VertexFormat::Float => MTLVertexFormat::Float,
        VertexFormat::Float2 => MTLVertexFormat::Float2,
        VertexFormat::Float3 => MTLVertexFormat::Float3,
        VertexFormat::Float4 => MTLVertexFormat::Float4,
        VertexFormat::Unorm8x4 => MTLVertexFormat::UChar4Normalized,
    }
}

pub fn vertex_step_function(rate: VertexInputRate) -> MTLVertexStepFunction {
    match rate {
        VertexInputRate::PerVertex => MTLVertexStepFunction::PerVertex,
        VertexInputRate::PerInstance => MTLVertexStepFunction::PerInstance,
    }
}

// MARK: Compare / blend

pub fn compare_function(op: CompareOp) -> MTLCompareFunction {
    match op {
        CompareOp::Never => MTLCompareFunction::Never,
        CompareOp::Less => MTLCompareFunction::Less,
        CompareOp::Equal => MTLCompareFunction::Equal,
        CompareOp::LessOrEqual => MTLCompareFunction::LessEqual,
        CompareOp::Greater => MTLCompareFunction::Greater,
        CompareOp::NotEqual => MTLCompareFunction::NotEqual,
        CompareOp::GreaterOrEqual => MTLCompareFunction::GreaterEqual,
        CompareOp::Always => MTLCompareFunction::Always,
    }
}

pub fn blend_factor(factor: BlendFactor) -> MTLBlendFactor {
    match factor {
        BlendFactor::Zero => MTLBlendFactor::Zero,
        BlendFactor::One => MTLBlendFactor::One,
        BlendFactor::SourceColor => MTLBlendFactor::SourceColor,
        BlendFactor::OneMinusSourceColor => MTLBlendFactor::OneMinusSourceColor,
        BlendFactor::DestinationColor => MTLBlendFactor::DestinationColor,
        BlendFactor::OneMinusDestinationColor => MTLBlendFactor::OneMinusDestinationColor,
        BlendFactor::SourceAlpha => MTLBlendFactor::SourceAlpha,
        BlendFactor::OneMinusSourceAlpha => MTLBlendFactor::OneMinusSourceAlpha,
        BlendFactor::DestinationAlpha => MTLBlendFactor::DestinationAlpha,
        BlendFactor::OneMinusDestinationAlpha => MTLBlendFactor::OneMinusDestinationAlpha,
        BlendFactor::BlendColor => MTLBlendFactor::BlendColor,
        BlendFactor::OneMinusBlendColor => MTLBlendFactor::OneMinusBlendColor,
        BlendFactor::SourceAlphaSaturate => MTLBlendFactor::SourceAlphaSaturated,
    }
}

pub fn blend_operation(op: BlendOperation) -> MTLBlendOperation {
    match op {
        BlendOperation::Add => MTLBlendOperation::Add,
        BlendOperation::Subtract => MTLBlendOperation::Subtract,
        BlendOperation::ReverseSubtract => MTLBlendOperation::ReverseSubtract,
        BlendOperation::Min => MTLBlendOperation::Min,
        BlendOperation::Max => MTLBlendOperation::Max,
    }
}

// MARK: Primitives / raster state

pub fn primitive_type(primitive: PrimitiveType) -> MTLPrimitiveType {
    match primitive {
        PrimitiveType::TriangleList => MTLPrimitiveType::Triangle,
        PrimitiveType::TriangleStrip => MTLPrimitiveType::TriangleStrip,
        PrimitiveType::LineList => MTLPrimitiveType::Line,
        PrimitiveType::LineStrip => MTLPrimitiveType::LineStrip,
        PrimitiveType::PointList => MTLPrimitiveType::Point,
    }
}

pub fn cull_mode(mode: CullMode) -> MTLCullMode {
    match mode {
        CullMode::None => MTLCullMode::None,
        CullMode::Front => MTLCullMode::Front,
        CullMode::Back => MTLCullMode::Back,
    }
}

pub fn winding(winding: FrontWinding) -> MTLWinding {
    match winding {
        FrontWinding::Clockwise => MTLWinding::Clockwise,
        FrontWinding::CounterClockwise => MTLWinding::CounterClockwise,
    }
}

pub fn triangle_fill_mode(mode: FillMode) -> MTLTriangleFillMode {
    match mode {
        FillMode::Fill => MTLTriangleFillMode::Fill,
        FillMode::Lines => MTLTriangleFillMode::Lines,
    }
}

// MARK: Index / samplers

pub fn index_type(index_type: IndexType) -> MTLIndexType {
    match index_type {
        IndexType::UInt16 => MTLIndexType::UInt16,
        IndexType::UInt32 => MTLIndexType::UInt32,
    }
}

pub fn sampler_min_mag_filter(filter: SamplerFilter) -> MTLSamplerMinMagFilter {
    match filter {
        SamplerFilter::Nearest => MTLSamplerMinMagFilter::Nearest,
        SamplerFilter::Linear => MTLSamplerMinMagFilter::Linear,
    }
}

pub fn sampler_mip_filter(filter: SamplerMipFilter) -> MTLSamplerMipFilter {
    match filter {
        SamplerMipFilter::Nearest => MTLSamplerMipFilter::Nearest,
        SamplerMipFilter::Linear => MTLSamplerMipFilter::Linear,
    }
}

pub fn sampler_address_mode(mode: SamplerAddressMode) -> MTLSamplerAddressMode {
    match mode {
        SamplerAddressMode::Repeat => MTLSamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat => MTLSamplerAddressMode::MirrorRepeat,
        SamplerAddressMode::ClampToEdge => MTLSamplerAddressMode::ClampToEdge,
    }
}

// MARK: Load actions

pub fn load_action(action: ColorLoadAction) -> MTLLoadAction {
    match action {
        ColorLoadAction::Clear(_) => MTLLoadAction::Clear,
        ColorLoadAction::Load => MTLLoadAction::Load,
        ColorLoadAction::DontCare => MTLLoadAction::DontCare,
    }
}

pub fn depth_load_action(action: DepthLoadAction) -> MTLLoadAction {
    match action {
        DepthLoadAction::Clear(_) => MTLLoadAction::Clear,
        DepthLoadAction::Load => MTLLoadAction::Load,
        DepthLoadAction::DontCare => MTLLoadAction::DontCare,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_color_format_round_trips() {
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
            assert_eq!(color_format(pixel_format(format)), format, "{format:?}");
        }
    }

    #[test]
    fn depth_formats_use_the_apple_silicon_substitutes() {
        assert_eq!(pixel_format(TextureFormat::Depth24Unorm), MTLPixelFormat::Depth32Float);
        assert_eq!(
            pixel_format(TextureFormat::Depth24UnormStencil8),
            MTLPixelFormat::Depth32Float_Stencil8
        );
        // Depth formats are not valid color targets, so the reverse map reports
        // invalid rather than silently reusing them.
        assert_eq!(color_format(MTLPixelFormat::Depth32Float), TextureFormat::Invalid);
    }

    #[test]
    fn texture_usage_folds_storage_and_transfer_into_shader_flags() {
        let usage = texture_usage(
            TextureUsage::SAMPLED
                | TextureUsage::STORAGE_WRITE
                | TextureUsage::TRANSFER_SOURCE
                | TextureUsage::PRESENT,
        );
        assert!(usage.contains(MTLTextureUsage::ShaderRead));
        assert!(usage.contains(MTLTextureUsage::ShaderWrite));
        assert!(usage.contains(MTLTextureUsage::RenderTarget));
        assert!(!texture_usage(TextureUsage::EMPTY).contains(MTLTextureUsage::RenderTarget));
    }

    #[test]
    fn byte_counts_match_the_texel_sizes() {
        assert_eq!(bytes_per_pixel(TextureFormat::R8Unorm), 1);
        assert_eq!(bytes_per_pixel(TextureFormat::Bgra8UnormSrgb), 4);
        assert_eq!(bytes_per_pixel(TextureFormat::Rgba16Float), 8);
        assert_eq!(bytes_per_pixel(TextureFormat::Rgba32Float), 16);
        assert_eq!(bytes_per_pixel(TextureFormat::Depth24UnormStencil8), 5);
    }

    #[test]
    fn load_actions_only_clear_for_explicit_clears() {
        assert_eq!(load_action(ColorLoadAction::DontCare), MTLLoadAction::DontCare);
        assert_eq!(load_action(ColorLoadAction::Load), MTLLoadAction::Load);
        assert_eq!(load_action(ColorLoadAction::Clear([0.0; 4])), MTLLoadAction::Clear);
        assert_eq!(depth_load_action(DepthLoadAction::Clear(1.0)), MTLLoadAction::Clear);
    }
}
