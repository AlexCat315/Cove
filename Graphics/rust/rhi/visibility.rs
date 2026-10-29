//! Translation of NativeRHI/Resources/ShaderVisibility.swift.
//!
//! Resource visibility may span several shader stages; a module still has one
//! stage.
use super::ShaderStage;

crate::rhi_option_set!(ShaderVisibility, u32, {
    const VERTEX = 1 << 0;
    const FRAGMENT = 1 << 1;
    const COMPUTE = 1 << 2;
    const TASK = 1 << 3;
    const MESH = 1 << 4;
});

impl ShaderVisibility {
    pub const GRAPHICS: Self = Self(Self::VERTEX.0 | Self::FRAGMENT.0);
    pub const ALL: Self = Self(Self::GRAPHICS.0 | Self::COMPUTE.0 | Self::TASK.0 | Self::MESH.0);
    pub const fn from_stage(stage: ShaderStage) -> Self {
        match stage {
            ShaderStage::Vertex => Self::VERTEX,
            ShaderStage::Fragment => Self::FRAGMENT,
            ShaderStage::Compute => Self::COMPUTE,
            ShaderStage::Task => Self::TASK,
            ShaderStage::Mesh => Self::MESH,
        }
    }
    /// Every stage named by this visibility set, in declaration order.
    pub fn stages(self) -> Vec<ShaderStage> {
        [ShaderStage::Vertex, ShaderStage::Fragment, ShaderStage::Compute, ShaderStage::Task, ShaderStage::Mesh]
            .into_iter()
            .filter(|stage| self.contains(Self::from_stage(*stage)))
            .collect()
    }
}
impl From<ShaderStage> for ShaderVisibility {
    fn from(stage: ShaderStage) -> Self {
        Self::from_stage(stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_sets_expand_in_order() {
        assert_eq!(ShaderVisibility::GRAPHICS.stages(), vec![ShaderStage::Vertex, ShaderStage::Fragment]);
        assert_eq!(ShaderVisibility::ALL.stages().len(), 5);
        assert_eq!(ShaderVisibility::from(ShaderStage::Compute), ShaderVisibility::COMPUTE);
    }
}
