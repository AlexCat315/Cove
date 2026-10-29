//! Translation of NativeRHI/Resources/PipelineInterface.swift.
use super::{rhi_require, RhiResult, ShaderStage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct BufferBindingLayout {
    pub read_only: bool,
    /// When true, the binding carries no base offset; the caller supplies a
    /// dynamic offset at bind time (Vulkan `*_DYNAMIC` descriptor, Metal
    /// `setVertexBuffer` offset, DX12 root offset). Used for per-frame ring
    /// data. This is an RHI-side concept never reflected from shaders, so it
    /// defaults to false when absent from a serialized shader interface.
    pub has_dynamic_offset: bool,
    /// Zero denotes byte-address storage. Positive values describe structured
    /// elements.
    pub element_stride: i64,
}
impl BufferBindingLayout {
    /// The Swift memberwise default, usable from `const fn` descriptor
    /// builders.
    pub const DEFAULT: Self =
        Self { read_only: false, has_dynamic_offset: false, element_stride: 0 };
}

/// A logical small-constant slot. Vulkan packs ranges into push-constant bytes;
/// Metal uses buffer arguments and DX12 uses root constants at `b[slot]`,
/// space 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PushConstantRange {
    pub stage: ShaderStage,
    pub slot: u32,
    pub byte_count: i64,
}
impl PushConstantRange {
    pub fn new(stage: ShaderStage, slot: u32, byte_count: i64) -> Self {
        Self { stage, slot, byte_count }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PipelineInterfaceKey {
    pub set_layouts: Vec<u32>,
    pub push_constants: Vec<PushConstantRange>,
}

/// Backend-independent immutable layout definitions, owned until device release.
#[derive(Clone, Debug, Default)]
pub struct PipelineInterfaces {
    pub uses: std::collections::HashMap<u32, super::PipelineUse>,
    pub bindings: std::collections::HashMap<u32, super::BindingLayoutDescriptor>,
    pub pipelines: std::collections::HashMap<u32, super::PipelineLayoutDescriptor>,
}
impl PipelineInterfaces {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self, descriptor: &super::PipelineLayoutDescriptor) -> RhiResult<()> {
        validate_push_constants(&descriptor.push_constants)
    }
}

/// Validates a pipeline layout's push-constant budget: each range is a positive
/// multiple of four bytes, stages and slots are unique, and the portable total
/// stays within 128 bytes.
pub fn validate_push_constants(ranges: &[PushConstantRange]) -> RhiResult<()> {
    let mut stages: Vec<ShaderStage> = Vec::new();
    let mut slots: Vec<u32> = Vec::new();
    let mut total: i64 = 0;
    for range in ranges {
        rhi_require(
            range.byte_count > 0 && range.byte_count % 4 == 0,
            "push constants require a positive multiple of four bytes",
        )?;
        rhi_require(
            !stages.contains(&range.stage) && !slots.contains(&range.slot),
            "push constant slots and stages must be unique",
        )?;
        stages.push(range.stage);
        slots.push(range.slot);
        total = total.saturating_add(range.byte_count);
        rhi_require(total <= 128, "portable push constant budget is 128 bytes")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_constant_validation_enforces_alignment_uniqueness_and_budget() {
        assert!(validate_push_constants(&[PushConstantRange::new(ShaderStage::Vertex, 0, 64)]).is_ok());
        assert!(validate_push_constants(&[PushConstantRange::new(ShaderStage::Vertex, 0, 3)]).is_err());
        assert!(
            validate_push_constants(&[
                PushConstantRange::new(ShaderStage::Vertex, 0, 64),
                PushConstantRange::new(ShaderStage::Vertex, 1, 64),
            ])
            .is_err()
        );
        assert!(
            validate_push_constants(&[
                PushConstantRange::new(ShaderStage::Vertex, 0, 64),
                PushConstantRange::new(ShaderStage::Fragment, 1, 68),
            ])
            .is_err()
        );
    }

    #[test]
    fn buffer_layout_defaults_match_the_swift_decoder() {
        let layout = BufferBindingLayout::default();
        assert!(!layout.read_only && !layout.has_dynamic_offset);
        assert_eq!(layout.element_stride, 0);
    }
}
