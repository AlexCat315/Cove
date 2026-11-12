//! Translation of NativeRHI/Device/Descriptors.swift and
//! NativeRHI/Resources/{MeshPipeline,TextureBufferUpload}.swift: resource and
//! pipeline descriptors.
//!
//! Swift's `OptionSet` types become newtypes over the raw bits, and the
//! convenience initializers become `new`/builder-style associated functions so a
//! descriptor can still be written in one expression.
use super::{
    AccelerationStructure, AttachmentBlendState, Buffer, BufferBindingLayout, CompareOp, CullMode,
    FillMode, FrontWinding, PipelineLayout, PrimitiveType, RhiResult, Sampler, SamplerAddressMode,
    SamplerFilter, SamplerMipFilter, ShaderFormat, ShaderModule, ShaderSpecializationConstant,
    ShaderStage, ShaderVisibility, Texture, TextureDimension, TextureFormat, ThreadgroupSize,
    VertexFormat, VertexInputRate, rhi_require,
};

// MARK: - Device configuration

/// Frontend device configuration. `preferred_backends` is ordered: the first
/// backend that constructs successfully wins and later entries are fallbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceConfig {
    pub preferred_backends: Vec<super::GraphicsApi>,
    /// Requests the Khronos validation layer where it is installed; ignored
    /// where it is not.
    pub enable_validation: bool,
    pub frames_in_flight: usize,
    pub prefer_low_power: bool,
    pub vsync_enabled: bool,
}
impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            preferred_backends: super::platform_default_backends(),
            enable_validation: true,
            frames_in_flight: 2,
            prefer_low_power: false,
            vsync_enabled: true,
        }
    }
}
impl DeviceConfig {
    pub fn validate(&self) -> RhiResult<()> {
        rhi_require(
            !self.preferred_backends.is_empty(),
            "choose at least one graphics backend",
        )?;
        rhi_require(
            self.frames_in_flight > 0,
            "frames in flight must be positive",
        )
    }
}

/// Maximum number of queues per class the backend can expose simultaneously.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueLimits {
    pub graphics: u8,
    pub compute: u8,
    pub transfer: u8,
}
impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            graphics: 1,
            compute: 1,
            transfer: 1,
        }
    }
}

// MARK: - Buffers

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferDescriptor {
    pub size: usize,
    pub usage: BufferUsage,
    pub label: Option<&'static str>,
}
impl BufferDescriptor {
    pub const fn new(size: usize, usage: BufferUsage) -> Self {
        Self {
            size,
            usage,
            label: None,
        }
    }
    pub const fn labelled(size: usize, usage: BufferUsage, label: &'static str) -> Self {
        Self {
            size,
            usage,
            label: Some(label),
        }
    }
}

crate::rhi_option_set!(BufferUsage, u32, {
    const VERTEX = 1 << 0;
    const INDEX = 1 << 1;
    const INDIRECT = 1 << 2;
    const UNIFORM = 1 << 3;
    const STORAGE_READ = 1 << 4;
    const STORAGE_WRITE = 1 << 5;
    const TRANSFER_SOURCE = 1 << 6;
    const TRANSFER_DESTINATION = 1 << 7;
});

// MARK: - Textures

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureDescriptor {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub layers: usize,
    pub mip_levels: usize,
    pub sample_count: usize,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub dimension: TextureDimension,
    /// Multisample attachment contents are discarded at the end of each pass.
    /// Supporting backends may use tile-only storage; other devices retain
    /// device-local storage.
    pub transient_attachment: bool,
    pub label: Option<&'static str>,
}
impl TextureDescriptor {
    pub const fn new(
        width: usize,
        height: usize,
        format: TextureFormat,
        usage: TextureUsage,
    ) -> Self {
        Self {
            width,
            height,
            depth: 1,
            layers: 1,
            mip_levels: 1,
            sample_count: 1,
            format,
            usage,
            dimension: TextureDimension::Texture2D,
            transient_attachment: false,
            label: None,
        }
    }

    pub const fn with_dimension(mut self, dimension: TextureDimension) -> Self {
        self.dimension = dimension;
        self
    }
    pub const fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }
    pub const fn with_layers(mut self, layers: usize) -> Self {
        self.layers = layers;
        self
    }
    pub const fn with_mip_levels(mut self, mip_levels: usize) -> Self {
        self.mip_levels = mip_levels;
        self
    }
    pub const fn with_sample_count(mut self, sample_count: usize) -> Self {
        self.sample_count = sample_count;
        self
    }
    pub const fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }
    pub const fn transient(mut self) -> Self {
        self.transient_attachment = true;
        self
    }
}

/// One 2D slice of a texture mip. Cube faces use layer indices 0...5.
/// Upload regions are relative to this mip; readback starts at (0, 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureSubresource {
    pub mip_level: usize,
    pub layer: usize,
}
impl TextureSubresource {
    pub const fn new(mip_level: usize, layer: usize) -> Self {
        Self { mip_level, layer }
    }
}

crate::rhi_option_set!(TextureUsage, u32, {
    const SAMPLED = 1 << 0;
    const COLOR_TARGET = 1 << 1;
    const DEPTH_STENCIL_TARGET = 1 << 2;
    const STORAGE_READ = 1 << 3;
    const STORAGE_WRITE = 1 << 4;
    const TRANSFER_SOURCE = 1 << 5;
    const TRANSFER_DESTINATION = 1 << 6;
    const PRESENT = 1 << 7;
});

// MARK: - Samplers

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplerDescriptor {
    pub min_filter: SamplerFilter,
    pub mag_filter: SamplerFilter,
    pub mip_filter: SamplerMipFilter,
    pub address_mode_u: SamplerAddressMode,
    pub address_mode_v: SamplerAddressMode,
    pub address_mode_w: SamplerAddressMode,
    pub compare_enabled: bool,
    pub compare_op: CompareOp,
}
impl Default for SamplerDescriptor {
    fn default() -> Self {
        Self {
            min_filter: SamplerFilter::Linear,
            mag_filter: SamplerFilter::Linear,
            mip_filter: SamplerMipFilter::Linear,
            address_mode_u: SamplerAddressMode::Repeat,
            address_mode_v: SamplerAddressMode::Repeat,
            address_mode_w: SamplerAddressMode::Repeat,
            compare_enabled: false,
            compare_op: CompareOp::Always,
        }
    }
}

// MARK: - Shader modules

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderModuleDescriptor {
    pub stage: ShaderStage,
    pub format: ShaderFormat,
    pub code: Vec<u8>,
    pub threadgroup_size: ThreadgroupSize,
    pub specialization_constants: Vec<ShaderSpecializationConstant>,
    pub entry_point: String,
}
impl ShaderModuleDescriptor {
    pub fn new(stage: ShaderStage, format: ShaderFormat, code: impl Into<Vec<u8>>) -> Self {
        Self {
            stage,
            format,
            code: code.into(),
            threadgroup_size: ThreadgroupSize::default(),
            specialization_constants: Vec::new(),
            entry_point: "main".to_owned(),
        }
    }

    pub fn with_entry_point(mut self, entry_point: impl Into<String>) -> Self {
        self.entry_point = entry_point.into();
        self
    }

    /// Convenience for MSL source shipped as a string.
    pub fn metal_source(source: impl Into<String>, stage: ShaderStage) -> Self {
        Self::new(stage, ShaderFormat::MslSource, source.into().into_bytes())
    }
}

// MARK: - Binding layouts

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BindingType {
    Sampler,
    Texture,
    StorageTexture,
    UniformBuffer,
    StorageBuffer,
    AccelerationStructure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindingLayoutEntry {
    pub buffer: BufferBindingLayout,
    pub slot: u32,
    pub binding_type: BindingType,
    pub visibility: ShaderVisibility,
    pub array_size: u32,
}
impl BindingLayoutEntry {
    pub const fn new(slot: u32, binding_type: BindingType, visibility: ShaderVisibility) -> Self {
        Self {
            buffer: BufferBindingLayout::DEFAULT,
            slot,
            binding_type,
            visibility,
            array_size: 1,
        }
    }
    pub const fn with_array_size(mut self, array_size: u32) -> Self {
        self.array_size = array_size;
        self
    }
    pub const fn with_buffer_layout(mut self, buffer: BufferBindingLayout) -> Self {
        self.buffer = buffer;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindingLayoutDescriptor {
    pub entries: Vec<BindingLayoutEntry>,
    pub label: Option<&'static str>,
}
impl BindingLayoutDescriptor {
    pub fn new(entries: Vec<BindingLayoutEntry>) -> Self {
        Self {
            entries,
            label: None,
        }
    }
}

/// A concrete resource bound at one slot of a binding set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindingResource {
    Sampler(Sampler),
    Texture(Texture),
    StorageTexture(Texture),
    /// A bounded range is required when a large transient upload buffer is
    /// bound as a small uniform block. `None` exposes the remaining buffer.
    UniformBuffer {
        buffer: Buffer,
        offset: usize,
        size: Option<usize>,
    },
    StorageBuffer {
        buffer: Buffer,
        offset: usize,
    },
    AccelerationStructure(AccelerationStructure),
}
impl BindingResource {
    pub const fn uniform_buffer(buffer: Buffer) -> Self {
        Self::UniformBuffer {
            buffer,
            offset: 0,
            size: None,
        }
    }
    pub const fn storage_buffer(buffer: Buffer) -> Self {
        Self::StorageBuffer { buffer, offset: 0 }
    }

    /// The resource kind the state tracker should observe, or `None` for a
    /// sampler (samplers carry no tracked state).
    pub fn tracks_as(&self) -> Option<(super::ResourceRef, BindingType)> {
        match *self {
            Self::Sampler(_) => None,
            Self::Texture(texture) => Some((
                super::ResourceRef::texture(texture.id()),
                BindingType::Texture,
            )),
            Self::StorageTexture(texture) => Some((
                super::ResourceRef::texture(texture.id()),
                BindingType::StorageTexture,
            )),
            Self::UniformBuffer { buffer, .. } => Some((
                super::ResourceRef::buffer(buffer.id()),
                BindingType::UniformBuffer,
            )),
            Self::StorageBuffer { buffer, .. } => Some((
                super::ResourceRef::buffer(buffer.id()),
                BindingType::StorageBuffer,
            )),
            Self::AccelerationStructure(structure) => Some((
                super::ResourceRef::acceleration_structure(structure.id()),
                BindingType::AccelerationStructure,
            )),
        }
    }

    /// Base offset of a buffer binding, or 0 for non-buffer resources.
    pub fn buffer_offset(&self) -> usize {
        match *self {
            Self::UniformBuffer { offset, .. } | Self::StorageBuffer { offset, .. } => offset,
            _ => 0,
        }
    }

    /// The bound buffer handle, or `None` for non-buffer resources.
    pub fn buffer(&self) -> Option<Buffer> {
        match *self {
            Self::UniformBuffer { buffer, .. } | Self::StorageBuffer { buffer, .. } => Some(buffer),
            _ => None,
        }
    }

    /// The tracked handle for this resource kind, if it has one.
    pub fn texture(&self) -> Option<Texture> {
        match *self {
            Self::Texture(texture) | Self::StorageTexture(texture) => Some(texture),
            _ => None,
        }
    }

    /// Normalizes the convenience `offset = 0` default so two entries that
    /// differ only by an omitted vs explicit zero offset compare equal, which
    /// keeps the binding-set cache key stable.
    pub fn normalized(self) -> Self {
        match self {
            Self::UniformBuffer {
                buffer,
                offset: 0,
                size,
            } => Self::UniformBuffer {
                buffer,
                offset: 0,
                size,
            },
            Self::StorageBuffer { buffer, offset: 0 } => Self::StorageBuffer { buffer, offset: 0 },
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindingSetEntry {
    pub slot: u32,
    pub resource: BindingResource,
}
impl BindingSetEntry {
    pub fn new(slot: u32, resource: BindingResource) -> Self {
        Self {
            slot,
            resource: resource.normalized(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindingSetDescriptor {
    pub entries: Vec<BindingSetEntry>,
    pub label: Option<&'static str>,
}
impl BindingSetDescriptor {
    pub fn new(entries: Vec<BindingSetEntry>) -> Self {
        Self {
            entries,
            label: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PipelineLayoutDescriptor {
    pub set_layouts: Vec<super::BindingLayout>,
    pub push_constants: Vec<super::PushConstantRange>,
    pub label: Option<&'static str>,
}
impl PipelineLayoutDescriptor {
    pub fn new(set_layouts: Vec<super::BindingLayout>) -> Self {
        Self {
            set_layouts,
            push_constants: Vec::new(),
            label: None,
        }
    }
}

/// Backend-neutral view of a resolved binding set entry. The backend encoder
/// uses this to bind concrete GPU resources at argument `slot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResolvedBindingEntry {
    pub slot: u32,
    pub binding_type: BindingType,
    pub visibility: ShaderVisibility,
    pub resource: BindingResource,
}
impl ResolvedBindingEntry {
    pub const fn new(
        slot: u32,
        binding_type: BindingType,
        visibility: ShaderVisibility,
        resource: BindingResource,
    ) -> Self {
        Self {
            slot,
            binding_type,
            visibility,
            resource,
        }
    }
}

// MARK: - Pipeline state

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepthStencilState {
    pub depth_compare: CompareOp,
    pub depth_write_enabled: bool,
}
impl Default for DepthStencilState {
    fn default() -> Self {
        Self {
            depth_compare: CompareOp::Less,
            depth_write_enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterizationState {
    pub fill_mode: FillMode,
    pub cull_mode: CullMode,
    pub front_winding: FrontWinding,
}
impl Default for RasterizationState {
    fn default() -> Self {
        Self {
            fill_mode: FillMode::Fill,
            cull_mode: CullMode::None,
            front_winding: FrontWinding::CounterClockwise,
        }
    }
}

/// HLSL input semantic for DXIL. Slang's default user varying mapping uses
/// TEXCOORD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexSemantic {
    pub name: &'static str,
    pub index: u32,
}
impl VertexSemantic {
    pub const fn new(index: u32) -> Self {
        Self {
            name: "TEXCOORD",
            index,
        }
    }
}
impl Default for VertexSemantic {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexAttribute {
    pub semantic: VertexSemantic,
    pub location: u32,
    pub format: VertexFormat,
    pub offset: usize,
    pub buffer_index: u32,
}
impl VertexAttribute {
    pub const fn new(location: u32, format: VertexFormat, offset: usize) -> Self {
        Self {
            semantic: VertexSemantic::new(location),
            location,
            format,
            offset,
            buffer_index: 0,
        }
    }
    pub const fn in_buffer(mut self, buffer_index: u32) -> Self {
        self.buffer_index = buffer_index;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexBufferLayout {
    pub stride: usize,
    pub step_rate: VertexInputRate,
}
impl VertexBufferLayout {
    pub const fn new(stride: usize) -> Self {
        Self {
            stride,
            step_rate: VertexInputRate::PerVertex,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VertexLayoutDescriptor {
    pub attributes: Vec<VertexAttribute>,
    pub buffer_layouts: Vec<VertexBufferLayout>,
}
impl VertexLayoutDescriptor {
    pub fn new(attributes: Vec<VertexAttribute>, buffer_layouts: Vec<VertexBufferLayout>) -> Self {
        Self {
            attributes,
            buffer_layouts,
        }
    }
}

/// One color attachment slot of a graphics pipeline. Multiple entries give MRT
/// rendering (the Zig original supported a single color target).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColorAttachmentDescriptor {
    pub format: TextureFormat,
    pub blend: AttachmentBlendState,
}
impl ColorAttachmentDescriptor {
    pub const fn new(format: TextureFormat) -> Self {
        Self {
            format,
            blend: AttachmentBlendState::OPAQUE,
        }
    }
    pub const fn blended(format: TextureFormat) -> Self {
        Self {
            format,
            blend: AttachmentBlendState::ALPHA_BLEND,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphicsPipelineDescriptor {
    pub sample_count: usize,
    pub layout: PipelineLayout,
    pub vertex: ShaderModule,
    pub fragment: Option<ShaderModule>,
    pub color_attachments: Vec<ColorAttachmentDescriptor>,
    pub depth_format: Option<TextureFormat>,
    pub stencil_format: Option<TextureFormat>,
    pub primitive: PrimitiveType,
    pub rasterization: RasterizationState,
    pub depth_stencil: Option<DepthStencilState>,
    pub vertex_layout: Option<VertexLayoutDescriptor>,
    pub label: Option<&'static str>,
}
impl GraphicsPipelineDescriptor {
    /// The Swift default keeps depth testing enabled against `depth32Float`.
    pub fn new(layout: PipelineLayout, vertex: ShaderModule) -> Self {
        Self {
            sample_count: 1,
            layout,
            vertex,
            fragment: None,
            color_attachments: Vec::new(),
            depth_format: Some(TextureFormat::Depth32Float),
            stencil_format: None,
            primitive: PrimitiveType::TriangleList,
            rasterization: RasterizationState::default(),
            depth_stencil: Some(DepthStencilState::default()),
            vertex_layout: None,
            label: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputePipelineDescriptor {
    pub layout: PipelineLayout,
    pub shader: ShaderModule,
    pub label: Option<&'static str>,
}
impl ComputePipelineDescriptor {
    pub fn new(layout: PipelineLayout, shader: ShaderModule) -> Self {
        Self {
            layout,
            shader,
            label: None,
        }
    }
}

/// Mesh pipelines share attachment and rasterization semantics with graphics
/// pipelines, but never contain a vertex layout or vertex shader.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPipelineDescriptor {
    pub layout: PipelineLayout,
    pub mesh: ShaderModule,
    pub task: Option<ShaderModule>,
    pub fragment: Option<ShaderModule>,
    pub color_attachments: Vec<ColorAttachmentDescriptor>,
    pub depth_format: Option<TextureFormat>,
    pub rasterization: RasterizationState,
    pub depth_stencil: Option<DepthStencilState>,
    pub label: Option<&'static str>,
}
impl MeshPipelineDescriptor {
    pub fn new(layout: PipelineLayout, mesh: ShaderModule) -> Self {
        Self {
            layout,
            mesh,
            task: None,
            fragment: None,
            color_attachments: Vec::new(),
            depth_format: None,
            rasterization: RasterizationState::default(),
            depth_stencil: None,
            label: None,
        }
    }
}

// MARK: - Surfaces

/// The platform object represented by an unmanaged native surface handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeSurfaceKind {
    MetalLayer,
    Win32Window,
    XlibWindow,
    WaylandSurface,
    AndroidWindow,
    WebCanvas,
}
impl NativeSurfaceKind {
    pub fn platform_default() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::Win32Window
        }
        #[cfg(target_os = "android")]
        {
            Self::AndroidWindow
        }
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            Self::XlibWindow
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self::WebCanvas
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "android",
            target_os = "linux",
            target_os = "freebsd",
            target_arch = "wasm32"
        )))]
        {
            Self::MetalLayer
        }
    }
}
impl Default for NativeSurfaceKind {
    fn default() -> Self {
        Self::platform_default()
    }
}

/// Backend-neutral swapchain surface. `kind` identifies the platform handle.
///
/// The native handle is stored as a `usize` address (0 means "absent") rather
/// than a raw pointer, so the descriptor stays `Send` without an unchecked
/// impl. The platform object's lifetime is owned by the caller and must remain
/// valid until the device releases the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceDescriptor {
    pub kind: NativeSurfaceKind,
    /// Xlib `Display*` or Wayland `wl_display*`. For Xlib the native handle
    /// holds the `Window` integer as a pointer bit pattern.
    pub display: usize,
    pub native_handle: usize,
    pub width: usize,
    pub height: usize,
    pub color_format: TextureFormat,
    /// The platform compositor blends premultiplied pixels over its backdrop.
    pub premultiplied_alpha: bool,
    pub vsync_enabled: bool,
}
impl SurfaceDescriptor {
    pub fn new(native_handle: usize) -> Self {
        Self {
            kind: NativeSurfaceKind::platform_default(),
            display: 0,
            native_handle,
            width: 0,
            height: 0,
            color_format: super::platform_default_surface_format(),
            premultiplied_alpha: false,
            vsync_enabled: true,
        }
    }
    pub const fn with_size(mut self, width: usize, height: usize) -> Self {
        self.width = width;
        self.height = height;
        self
    }
    pub const fn with_color_format(mut self, color_format: TextureFormat) -> Self {
        self.color_format = color_format;
        self
    }
    pub const fn with_vsync(mut self, vsync_enabled: bool) -> Self {
        self.vsync_enabled = vsync_enabled;
        self
    }
    pub const fn with_display(mut self, display: usize) -> Self {
        self.display = display;
        self
    }
    pub const fn with_kind(mut self, kind: NativeSurfaceKind) -> Self {
        self.kind = kind;
        self
    }
}

// MARK: - Recorded texture upload

/// A recorded rectangular upload from frame-owned or persistent buffer storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureBufferUpload {
    pub buffer: Buffer,
    pub texture: Texture,
    pub region: super::TextureUploadRegion,
    pub bytes_per_row: usize,
    pub offset: usize,
    pub subresource: TextureSubresource,
}
impl TextureBufferUpload {
    pub fn new(
        buffer: Buffer,
        bytes_per_row: usize,
        texture: Texture,
        region: super::TextureUploadRegion,
    ) -> Self {
        Self {
            buffer,
            bytes_per_row,
            texture,
            region,
            offset: 0,
            subresource: TextureSubresource::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_config_rejects_empty_backend_lists() {
        let mut config = DeviceConfig::default();
        config.preferred_backends.clear();
        assert!(config.validate().is_err());
        let config = DeviceConfig {
            frames_in_flight: 0,
            ..DeviceConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn buffer_usage_composes_like_an_option_set() {
        let usage = BufferUsage::VERTEX | BufferUsage::TRANSFER_DESTINATION;
        assert!(usage.contains(BufferUsage::VERTEX));
        assert!(!usage.contains(BufferUsage::INDEX));
        assert_eq!(BufferUsage::default(), BufferUsage::EMPTY);
    }

    #[test]
    fn binding_entries_normalize_the_default_offset() {
        let buffer = Buffer::new(4);
        let omitted = BindingSetEntry::new(0, BindingResource::uniform_buffer(buffer));
        let explicit = BindingSetEntry::new(
            0,
            BindingResource::UniformBuffer {
                buffer,
                offset: 0,
                size: None,
            },
        );
        assert_eq!(omitted, explicit);
        assert_eq!(omitted.resource.buffer_offset(), 0);
        assert_eq!(omitted.resource.buffer(), Some(buffer));
        assert_eq!(BindingResource::Sampler(Sampler::new(1)).tracks_as(), None);
    }

    #[test]
    fn tracked_resources_report_their_kind() {
        let texture = Texture::new(9);
        let (reference, kind) = BindingResource::StorageTexture(texture)
            .tracks_as()
            .expect("tracked");
        assert_eq!(kind, BindingType::StorageTexture);
        assert_eq!(reference.kind, super::super::ResourceKind::Texture);
        assert_eq!(reference.id, 9);
    }

    #[test]
    fn surface_descriptor_defaults_to_the_platform_format() {
        let surface = SurfaceDescriptor::new(0x1000);
        assert_eq!(
            surface.color_format,
            super::super::platform_default_surface_format()
        );
        assert!(surface.vsync_enabled);
        assert_eq!(surface.kind, NativeSurfaceKind::platform_default());
    }
}
