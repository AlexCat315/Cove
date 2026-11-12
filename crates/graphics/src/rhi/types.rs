//! Translation of NativeRHI/RHITypes.swift: public enumerations, bit flags and
//! small value types.
use serde::{Deserialize, Serialize};

/// A packed set of flags. Swift's `OptionSet` maps to a newtype over the raw
/// bits, so an illegal combination can never be constructed.
#[macro_export]
macro_rules! rhi_option_set {
    ($name:ident, $backing:ty, { $($(#[$meta:meta])* const $flag:ident = $value:expr;)* }) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $backing);
        impl $name {
            pub const EMPTY: Self = Self(0);
            $( $(#[$meta])* pub const $flag: Self = Self($value); )*
            pub const fn bits(self) -> $backing { self.0 }
            pub const fn from_bits_truncate(bits: $backing) -> Self { Self(bits) }
            pub const fn is_empty(self) -> bool { self.0 == 0 }
            pub const fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
            pub const fn intersects(self, other: Self) -> bool { (self.0 & other.0) != 0 }
            pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
            pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
        }
        impl std::ops::BitOr for $name {
            type Output = Self;
            fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
        }
        impl std::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
        }
        impl std::ops::BitAnd for $name {
            type Output = Self;
            fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
        }
        impl std::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, rhs: Self) { self.0 &= rhs.0; }
        }
        impl std::ops::Not for $name {
            type Output = Self;
            fn not(self) -> Self { Self(!self.0) }
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphicsApi {
    Metal,
    Vulkan,
    Dx12,
    WebGpu,
}
impl GraphicsApi {
    pub const ALL: [Self; 4] = [Self::Metal, Self::Vulkan, Self::Dx12, Self::WebGpu];
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Vulkan => "Vulkan",
            Self::Dx12 => "DirectX 12",
            Self::WebGpu => "WebGPU",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShaderFormat {
    MslSource,
    Metallib,
    Spirv,
    Dxil,
    Wgsl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
    Task,
    Mesh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrimitiveType {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
    PointList,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FillMode {
    Fill,
    Lines,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CullMode {
    None,
    Front,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrontWinding {
    Clockwise,
    CounterClockwise,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompareOp {
    Never,
    Less,
    Equal,
    LessOrEqual,
    Greater,
    NotEqual,
    GreaterOrEqual,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlendFactor {
    Zero,
    One,
    SourceColor,
    OneMinusSourceColor,
    DestinationColor,
    OneMinusDestinationColor,
    SourceAlpha,
    OneMinusSourceAlpha,
    DestinationAlpha,
    OneMinusDestinationAlpha,
    BlendColor,
    OneMinusBlendColor,
    SourceAlphaSaturate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttachmentBlendState {
    pub enabled: bool,
    pub source_color_blend_factor: BlendFactor,
    pub destination_color_blend_factor: BlendFactor,
    pub color_blend_operation: BlendOperation,
    pub source_alpha_blend_factor: BlendFactor,
    pub destination_alpha_blend_factor: BlendFactor,
    pub alpha_blend_operation: BlendOperation,
}
impl AttachmentBlendState {
    pub const ALPHA_BLEND: Self = Self {
        enabled: true,
        source_color_blend_factor: BlendFactor::SourceAlpha,
        destination_color_blend_factor: BlendFactor::OneMinusSourceAlpha,
        color_blend_operation: BlendOperation::Add,
        source_alpha_blend_factor: BlendFactor::One,
        destination_alpha_blend_factor: BlendFactor::OneMinusSourceAlpha,
        alpha_blend_operation: BlendOperation::Add,
    };
    pub const OPAQUE: Self = Self {
        enabled: false,
        source_color_blend_factor: BlendFactor::SourceAlpha,
        destination_color_blend_factor: BlendFactor::OneMinusSourceAlpha,
        color_blend_operation: BlendOperation::Add,
        source_alpha_blend_factor: BlendFactor::One,
        destination_alpha_blend_factor: BlendFactor::OneMinusSourceAlpha,
        alpha_blend_operation: BlendOperation::Add,
    };
}
impl Default for AttachmentBlendState {
    fn default() -> Self {
        Self::OPAQUE
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SamplerFilter {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SamplerMipFilter {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SamplerAddressMode {
    Repeat,
    MirrorRepeat,
    ClampToEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VertexInputRate {
    PerVertex,
    PerInstance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VertexFormat {
    Float,
    Float2,
    Float3,
    Float4,
    Unorm8x4,
}
impl VertexFormat {
    /// Bytes a single vertex attribute occupies in the vertex buffer.
    pub fn size(self) -> usize {
        match self {
            Self::Float => 4,
            Self::Float2 => 8,
            Self::Float3 => 12,
            Self::Float4 => 16,
            Self::Unorm8x4 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexType {
    Uint16,
    Uint32,
}
impl IndexType {
    pub fn size(self) -> usize {
        match self {
            Self::Uint16 => 2,
            Self::Uint32 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureDimension {
    Texture2D,
    Texture3D,
    Cube,
    Texture2DArray,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureFormat {
    Invalid,
    R8Unorm,
    Rgba8Unorm,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgba8UnormSrgb,
    Rgba16Float,
    Rgba32Float,
    R32Uint,
    R32Float,
    Depth24Unorm,
    Depth24UnormStencil8,
    Depth32Float,
}
impl TextureFormat {
    /// Bytes per texel for the color formats the UI pipeline uses. `Invalid`
    /// reports zero so transfer validation rejects it; depth formats report the
    /// conservative four-byte estimate.
    pub fn bytes_per_texel(self) -> usize {
        match self {
            Self::Invalid => 0,
            Self::R8Unorm => 1,
            Self::Rgba16Float => 8,
            Self::Rgba32Float => 16,
            _ => 4,
        }
    }
    pub fn is_depth(self) -> bool {
        matches!(
            self,
            Self::Depth24Unorm | Self::Depth24UnormStencil8 | Self::Depth32Float
        )
    }
}

rhi_option_set!(ResourceState, u32, {
    const CONSTANT_BUFFER = 1 << 0;
    const VERTEX_BUFFER = 1 << 1;
    const INDEX_BUFFER = 1 << 2;
    const INDIRECT_ARGUMENT = 1 << 3;
    const SHADER_RESOURCE = 1 << 4;
    const UNORDERED_ACCESS = 1 << 5;
    const RENDER_TARGET = 1 << 6;
    const DEPTH_WRITE = 1 << 7;
    const DEPTH_READ = 1 << 8;
    const COPY_DESTINATION = 1 << 9;
    const COPY_SOURCE = 1 << 10;
    const PRESENT = 1 << 11;
    const ACCELERATION_STRUCTURE_READ = 1 << 12;
    const ACCELERATION_STRUCTURE_WRITE = 1 << 13;
    const RESOLVE_DESTINATION = 1 << 14;
    const RESOLVE_SOURCE = 1 << 15;
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueClass {
    Graphics,
    Compute,
    Transfer,
}
impl QueueClass {
    pub const ALL: [Self; 3] = [Self::Graphics, Self::Compute, Self::Transfer];
    pub fn index(self) -> usize {
        match self {
            Self::Graphics => 0,
            Self::Compute => 1,
            Self::Transfer => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarrierSyncAction {
    Full,
    Acquire,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarrierPassScope {
    OutsidePass,
    BeforePass,
    AfterPass,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_state_behaves_like_an_option_set() {
        let state = ResourceState::VERTEX_BUFFER | ResourceState::SHADER_RESOURCE;
        assert!(state.contains(ResourceState::VERTEX_BUFFER));
        assert!(!state.contains(ResourceState::INDEX_BUFFER));
        assert!(state.intersects(ResourceState::SHADER_RESOURCE));
        assert!(ResourceState::EMPTY.is_empty());
        let mut mutable = state;
        mutable.remove(ResourceState::VERTEX_BUFFER);
        assert_eq!(mutable, ResourceState::SHADER_RESOURCE);
    }

    #[test]
    fn queue_indices_follow_the_swift_order() {
        assert_eq!(QueueClass::Graphics.index(), 0);
        assert_eq!(QueueClass::Transfer.index(), 2);
    }

    #[test]
    fn format_sizes_cover_the_ui_pipeline() {
        assert_eq!(TextureFormat::R8Unorm.bytes_per_texel(), 1);
        assert_eq!(TextureFormat::Rgba16Float.bytes_per_texel(), 8);
        assert_eq!(TextureFormat::Rgba8Unorm.bytes_per_texel(), 4);
        assert!(TextureFormat::Depth32Float.is_depth());
        assert_eq!(VertexFormat::Float4.size(), 16);
    }
}
