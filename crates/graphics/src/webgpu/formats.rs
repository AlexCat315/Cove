use crate::rhi::*;
use serde_json::{Value, json};

pub(super) fn texture_format(format: TextureFormat) -> RhiResult<&'static str> {
    Ok(match format {
        TextureFormat::Invalid => {
            return Err(RhiError::invalid_argument("invalid WebGPU texture format"));
        }
        TextureFormat::R8Unorm => "r8unorm",
        TextureFormat::Rgba8Unorm => "rgba8unorm",
        TextureFormat::Bgra8Unorm => "bgra8unorm",
        TextureFormat::Rgba8UnormSrgb => "rgba8unorm-srgb",
        TextureFormat::Bgra8UnormSrgb => "bgra8unorm-srgb",
        TextureFormat::Rgba16Float => "rgba16float",
        TextureFormat::Rgba32Float => "rgba32float",
        TextureFormat::R32Float => "r32float",
        TextureFormat::R32Uint => "r32uint",
        TextureFormat::Depth24Unorm => "depth24plus",
        TextureFormat::Depth24UnormStencil8 => "depth24plus-stencil8",
        TextureFormat::Depth32Float => "depth32float",
    })
}

pub(super) fn buffer_usage(usage: BufferUsage) -> u32 {
    let flags = [
        (BufferUsage::TRANSFER_SOURCE, 0x04),
        (BufferUsage::TRANSFER_DESTINATION, 0x08),
        (BufferUsage::INDEX, 0x10),
        (BufferUsage::VERTEX, 0x20),
        (BufferUsage::UNIFORM, 0x40),
        (BufferUsage::STORAGE_READ | BufferUsage::STORAGE_WRITE, 0x80),
        (BufferUsage::INDIRECT, 0x100),
    ];
    flags
        .into_iter()
        .filter(|(flag, _)| usage.intersects(*flag))
        .map(|(_, value)| value)
        .fold(0, |lhs, rhs| lhs | rhs)
}

pub(super) fn texture_usage(usage: TextureUsage) -> u32 {
    let flags = [
        (TextureUsage::TRANSFER_SOURCE, 0x01),
        (TextureUsage::TRANSFER_DESTINATION, 0x02),
        (TextureUsage::SAMPLED, 0x04),
        (
            TextureUsage::STORAGE_READ | TextureUsage::STORAGE_WRITE,
            0x08,
        ),
        (
            TextureUsage::COLOR_TARGET | TextureUsage::DEPTH_STENCIL_TARGET,
            0x10,
        ),
    ];
    flags
        .into_iter()
        .filter(|(flag, _)| usage.intersects(*flag))
        .map(|(_, value)| value)
        .fold(0, |lhs, rhs| lhs | rhs)
}

pub(super) fn color_target(target: &ColorAttachmentDescriptor) -> RhiResult<Value> {
    let mut descriptor = json!({ "format": texture_format(target.format)? });
    let blend = target.blend;
    if blend.enabled {
        descriptor["blend"] = json!({
            "color": blend_component(blend.source_color_blend_factor, blend.destination_color_blend_factor, blend.color_blend_operation),
            "alpha": blend_component(blend.source_alpha_blend_factor, blend.destination_alpha_blend_factor, blend.alpha_blend_operation),
        });
    }
    Ok(descriptor)
}

fn blend_component(src: BlendFactor, dst: BlendFactor, op: BlendOperation) -> Value {
    let factor = |factor| match factor {
        BlendFactor::Zero => "zero",
        BlendFactor::One => "one",
        BlendFactor::SourceColor => "src",
        BlendFactor::OneMinusSourceColor => "one-minus-src",
        BlendFactor::DestinationColor => "dst",
        BlendFactor::OneMinusDestinationColor => "one-minus-dst",
        BlendFactor::SourceAlpha => "src-alpha",
        BlendFactor::OneMinusSourceAlpha => "one-minus-src-alpha",
        BlendFactor::DestinationAlpha => "dst-alpha",
        BlendFactor::OneMinusDestinationAlpha => "one-minus-dst-alpha",
        BlendFactor::BlendColor => "constant",
        BlendFactor::OneMinusBlendColor => "one-minus-constant",
        BlendFactor::SourceAlphaSaturate => "src-alpha-saturated",
    };
    let operation = match op {
        BlendOperation::Add => "add",
        BlendOperation::Subtract => "subtract",
        BlendOperation::ReverseSubtract => "reverse-subtract",
        BlendOperation::Min => "min",
        BlendOperation::Max => "max",
    };
    json!({ "srcFactor": factor(src), "dstFactor": factor(dst), "operation": operation })
}

pub(super) fn compare(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Never => "never",
        CompareOp::Less => "less",
        CompareOp::Equal => "equal",
        CompareOp::LessOrEqual => "less-equal",
        CompareOp::Greater => "greater",
        CompareOp::NotEqual => "not-equal",
        CompareOp::GreaterOrEqual => "greater-equal",
        CompareOp::Always => "always",
    }
}

pub(super) fn sampler(descriptor: SamplerDescriptor) -> RhiResult<Value> {
    if descriptor.compare_enabled {
        return Err(RhiError::unsupported_feature(
            "WebGPU comparison samplers need a comparison binding type",
        ));
    }
    let address = |mode| match mode {
        SamplerAddressMode::ClampToEdge => "clamp-to-edge",
        SamplerAddressMode::Repeat => "repeat",
        SamplerAddressMode::MirrorRepeat => "mirror-repeat",
    };
    let filter = |mode| match mode {
        SamplerFilter::Linear => "linear",
        SamplerFilter::Nearest => "nearest",
    };
    let mip = match descriptor.mip_filter {
        SamplerMipFilter::Nearest => "nearest",
        SamplerMipFilter::Linear => "linear",
    };
    Ok(json!({
        "minFilter": filter(descriptor.min_filter), "magFilter": filter(descriptor.mag_filter),
        "mipmapFilter": mip,
        "addressModeU": address(descriptor.address_mode_u),
        "addressModeV": address(descriptor.address_mode_v),
        "addressModeW": address(descriptor.address_mode_w),
    }))
}

pub(super) fn vertex_layout(layout: Option<&VertexLayoutDescriptor>) -> Vec<Value> {
    let Some(layout) = layout else {
        return Vec::new();
    };
    layout.buffer_layouts.iter().enumerate().map(|(index, buffer)| {
        let attributes: Vec<_> = layout.attributes.iter().filter(|attribute| attribute.buffer_index as usize == index).map(|attribute| {
            let format = match attribute.format {
                VertexFormat::Float => "float32",
                VertexFormat::Float2 => "float32x2",
                VertexFormat::Float3 => "float32x3",
                VertexFormat::Float4 => "float32x4",
                VertexFormat::Unorm8x4 => "unorm8x4",
            };
            json!({ "format": format, "offset": attribute.offset, "shaderLocation": attribute.location })
        }).collect();
        let step = match buffer.step_rate { VertexInputRate::PerVertex => "vertex", VertexInputRate::PerInstance => "instance" };
        json!({ "arrayStride": buffer.stride, "stepMode": step, "attributes": attributes })
    }).collect()
}

pub(super) fn primitive(descriptor: &GraphicsPipelineDescriptor) -> RhiResult<Value> {
    if descriptor.rasterization.fill_mode != FillMode::Fill {
        return Err(RhiError::unsupported_feature("WebGPU wireframe fill"));
    }
    let topology = match descriptor.primitive {
        PrimitiveType::TriangleList => "triangle-list",
        PrimitiveType::TriangleStrip => "triangle-strip",
        PrimitiveType::LineList => "line-list",
        PrimitiveType::LineStrip => "line-strip",
        PrimitiveType::PointList => "point-list",
    };
    let front = match descriptor.rasterization.front_winding {
        FrontWinding::Clockwise => "cw",
        FrontWinding::CounterClockwise => "ccw",
    };
    let cull = match descriptor.rasterization.cull_mode {
        CullMode::None => "none",
        CullMode::Front => "front",
        CullMode::Back => "back",
    };
    let mut value = json!({ "topology": topology, "frontFace": front, "cullMode": cull });
    if matches!(
        descriptor.primitive,
        PrimitiveType::LineStrip | PrimitiveType::TriangleStrip
    ) {
        value["stripIndexFormat"] = json!("uint32");
    }
    Ok(value)
}
