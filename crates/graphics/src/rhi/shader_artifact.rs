//! Translation of NativeRHI/Resources/{ShaderArtifact,ShaderArtifactLoading}.swift
//! and the reflected-layout merge in Validation/ReflectedBindingLayout.swift.
//!
//! An offline compiler's output. The RHI consumes this contract without loading
//! Slang or requiring its crates/runtime/reflection types in engine code.
use std::collections::HashMap;

use super::{
    BindingLayoutDescriptor, BindingLayoutEntry, BindingType, BufferBindingLayout,
    PushConstantRange, ReflectedShaderConstant, RhiError, RhiResult, ShaderConstantType,
    ShaderConstantValue, ShaderFormat, ShaderModuleDescriptor, ShaderSpecializationConstant,
    ShaderStage, ShaderVisibility, ThreadgroupSize, ThreadgroupSpecialization, rhi_require,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectedShaderBinding {
    pub buffer: BufferBindingLayout,
    pub name: String,
    pub slot: u32,
    pub space: u32,
    #[serde(rename = "type")]
    pub binding_type: BindingType,
}
impl ReflectedShaderBinding {
    pub fn new(name: impl Into<String>, slot: u32, binding_type: BindingType) -> Self {
        Self {
            buffer: BufferBindingLayout::DEFAULT,
            name: name.into(),
            slot,
            space: 0,
            binding_type,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShaderInterface {
    pub threadgroup_size: ThreadgroupSize,
    pub bindings: Vec<ReflectedShaderBinding>,
    pub push_constants: Vec<PushConstantRange>,
    pub specialization_constants: Vec<ReflectedShaderConstant>,
    pub threadgroup_specialization: ThreadgroupSpecialization,
}
impl ShaderInterface {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The offline compiler's artifact for one shader stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderArtifact {
    pub stage: ShaderStage,
    pub format: ShaderFormat,
    pub entry_point: String,
    pub code: Vec<u8>,
    pub interface: ShaderInterface,
    pub compiler: String,
    /// Source artifact code hash for an optional platform-precompiled library.
    pub source_fingerprint: Option<String>,
}
impl ShaderArtifact {
    pub fn new(
        stage: ShaderStage,
        format: ShaderFormat,
        entry_point: impl Into<String>,
        code: Vec<u8>,
        compiler: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            format,
            entry_point: entry_point.into(),
            code,
            interface: ShaderInterface::default(),
            compiler: compiler.into(),
            source_fingerprint: None,
        }
    }

    /// Builds the module descriptor, materializing specialization defaults and
    /// resolving the physical workgroup size.
    pub fn module_descriptor(
        &self,
        specialization: &[ShaderSpecializationConstant],
    ) -> RhiResult<ShaderModuleDescriptor> {
        self.interface.threadgroup_size.validate()?;
        rhi_require(
            !self.entry_point.is_empty() && !self.code.is_empty(),
            "shader artifact is empty",
        )?;

        let mut descriptor =
            ShaderModuleDescriptor::new(self.stage, self.format, self.code.clone())
                .with_entry_point(self.entry_point.clone());

        let mut ids: Vec<u32> = specialization.iter().map(|constant| constant.id).collect();
        let requested = ids.len();
        ids.sort_unstable();
        ids.dedup();
        rhi_require(ids.len() == requested, "duplicate shader specialization ID")?;

        for constant in specialization {
            rhi_require(
                self.interface
                    .specialization_constants
                    .iter()
                    .any(|reflected| {
                        reflected.id == constant.id
                            && reflected.constant_type == constant.value.constant_type()
                    }),
                "unknown shader specialization ID or incompatible type",
            )?;
        }

        let mut values = specialization.to_vec();
        // Materialize artifact defaults on every target so physical dispatch
        // dimensions also agree when the offline default was customized.
        let defaults = [
            (
                self.interface.threadgroup_specialization.x,
                self.interface.threadgroup_size.x,
            ),
            (
                self.interface.threadgroup_specialization.y,
                self.interface.threadgroup_size.y,
            ),
            (
                self.interface.threadgroup_specialization.z,
                self.interface.threadgroup_size.z,
            ),
        ];
        for (id, size) in defaults {
            let Some(id) = id else { continue };
            let Some(reflected) = self
                .interface
                .specialization_constants
                .iter()
                .find(|constant| constant.id == id)
            else {
                return Err(RhiError::layout_mismatch(
                    "workgroup constant needs a reflected integer ID",
                ));
            };
            if reflected.constant_type != ShaderConstantType::Uint32
                && reflected.constant_type != ShaderConstantType::Int32
            {
                return Err(RhiError::layout_mismatch(
                    "workgroup constant needs a reflected integer ID",
                ));
            }
            if values.iter().any(|constant| constant.id == id) {
                continue;
            }
            let value = match reflected.constant_type {
                ShaderConstantType::Uint32 => match u32::try_from(size) {
                    Ok(size) => ShaderConstantValue::Uint32(size),
                    Err(_) => {
                        return Err(RhiError::layout_mismatch(
                            "workgroup default is outside the constant type's range",
                        ));
                    }
                },
                ShaderConstantType::Int32 => match i32::try_from(size) {
                    Ok(size) => ShaderConstantValue::Int32(size),
                    Err(_) => {
                        return Err(RhiError::layout_mismatch(
                            "workgroup default is outside the constant type's range",
                        ));
                    }
                },
                _ => unreachable!("integer types were checked above"),
            };
            values.push(ShaderSpecializationConstant::new(id, value));
        }

        descriptor.specialization_constants = values.clone();
        descriptor.threadgroup_size = self
            .interface
            .threadgroup_specialization
            .resolve(self.interface.threadgroup_size, &values)?;
        Ok(descriptor)
    }

    /// Target-specific reflected slots. Cross-target logical bindings must be
    /// lowered by the toolchain; the backend never guesses register remappings.
    pub fn binding_layout_descriptor(&self, space: u32) -> RhiResult<BindingLayoutDescriptor> {
        let bindings: Vec<&ReflectedShaderBinding> = self
            .interface
            .bindings
            .iter()
            .filter(|binding| binding.space == space)
            .collect();
        let mut slots: Vec<u32> = bindings.iter().map(|binding| binding.slot).collect();
        let count = slots.len();
        slots.sort_unstable();
        slots.dedup();
        rhi_require(
            count == slots.len(),
            "reflected resources need distinct RHI binding slots",
        )?;
        let entries = bindings
            .into_iter()
            .map(|binding| {
                BindingLayoutEntry::new(
                    binding.slot,
                    binding.binding_type,
                    ShaderVisibility::from_stage(self.stage),
                )
                .with_buffer_layout(binding.buffer)
            })
            .collect();
        Ok(BindingLayoutDescriptor::new(entries))
    }
}

/// Merge stage visibility while preserving the reflected resource ABI.
pub fn reflected_binding_layout(
    artifacts: &[ShaderArtifact],
) -> RhiResult<BindingLayoutDescriptor> {
    let mut entries: HashMap<u32, BindingLayoutEntry> = HashMap::new();
    for artifact in artifacts {
        let mut slots: Vec<u32> = artifact
            .interface
            .bindings
            .iter()
            .map(|binding| binding.slot)
            .collect();
        let count = slots.len();
        slots.sort_unstable();
        slots.dedup();
        if slots.len() != count {
            return Err(RhiError::layout_mismatch(
                "duplicate reflected resource slot in one shader",
            ));
        }
        for binding in &artifact.interface.bindings {
            if binding.space != 0 {
                return Err(RhiError::layout_mismatch("reflected layout expects space0"));
            }
            let visibility = ShaderVisibility::from_stage(artifact.stage);
            match entries.get_mut(&binding.slot) {
                Some(existing) => {
                    if existing.binding_type != binding.binding_type
                        || existing.buffer != binding.buffer
                    {
                        return Err(RhiError::layout_mismatch(format!(
                            "inconsistent reflected resource at {}",
                            binding.slot
                        )));
                    }
                    existing.visibility.insert(visibility);
                }
                None => {
                    let entry =
                        BindingLayoutEntry::new(binding.slot, binding.binding_type, visibility)
                            .with_buffer_layout(binding.buffer);
                    entries.insert(binding.slot, entry);
                }
            }
        }
    }
    let mut ordered: Vec<BindingLayoutEntry> = entries.into_values().collect();
    ordered.sort_by_key(|entry| entry.slot);
    Ok(BindingLayoutDescriptor::new(ordered))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact() -> ShaderArtifact {
        let mut artifact = ShaderArtifact::new(
            ShaderStage::Fragment,
            ShaderFormat::Spirv,
            "main",
            vec![0x03, 0x02, 0x23, 0x07],
            "slang",
        );
        artifact
            .interface
            .bindings
            .push(ReflectedShaderBinding::new("ui", 0, BindingType::Texture));
        artifact
            .interface
            .push_constants
            .push(PushConstantRange::new(ShaderStage::Fragment, 0, 16));
        artifact
            .interface
            .specialization_constants
            .push(ReflectedShaderConstant::new(
                3,
                "workgroupX",
                ShaderConstantType::Uint32,
            ));
        artifact.interface.threadgroup_specialization.x = Some(3);
        artifact.interface.threadgroup_size = ThreadgroupSize::new(8, 4, 1);
        artifact
    }

    #[test]
    fn module_descriptors_materialize_workgroup_defaults() {
        let artifact = artifact();
        let descriptor = artifact.module_descriptor(&[]).expect("descriptor");
        assert_eq!(descriptor.entry_point, "main");
        assert_eq!(descriptor.code, vec![0x03, 0x02, 0x23, 0x07]);
        assert_eq!(descriptor.threadgroup_size, ThreadgroupSize::new(8, 4, 1));
        assert_eq!(descriptor.specialization_constants.len(), 1);
        assert_eq!(
            descriptor.specialization_constants[0].value,
            ShaderConstantValue::Uint32(8)
        );
    }

    #[test]
    fn an_explicit_constant_overrides_the_default() {
        let artifact = artifact();
        let descriptor = artifact
            .module_descriptor(&[ShaderSpecializationConstant::new(
                3,
                ShaderConstantValue::Uint32(16),
            )])
            .expect("descriptor");
        assert_eq!(descriptor.threadgroup_size, ThreadgroupSize::new(16, 4, 1));
    }

    #[test]
    fn unknown_or_duplicate_constants_are_rejected() {
        let artifact = artifact();
        assert!(
            artifact
                .module_descriptor(&[ShaderSpecializationConstant::new(
                    99,
                    ShaderConstantValue::Uint32(1)
                )])
                .is_err()
        );
        assert!(
            artifact
                .module_descriptor(&[ShaderSpecializationConstant::new(
                    3,
                    ShaderConstantValue::Bool(true)
                )])
                .is_err()
        );
        assert!(
            artifact
                .module_descriptor(&[
                    ShaderSpecializationConstant::new(3, ShaderConstantValue::Uint32(1)),
                    ShaderSpecializationConstant::new(3, ShaderConstantValue::Uint32(2)),
                ])
                .is_err()
        );
    }

    #[test]
    fn reflected_layouts_merge_visibility_across_stages() {
        let fragment = artifact();
        let mut vertex = artifact();
        vertex.stage = ShaderStage::Vertex;
        let layout = reflected_binding_layout(&[fragment, vertex]).expect("layout");
        assert_eq!(layout.entries.len(), 1);
        assert_eq!(layout.entries[0].slot, 0);
        assert_eq!(
            layout.entries[0].visibility,
            ShaderVisibility::VERTEX | ShaderVisibility::FRAGMENT
        );
    }

    #[test]
    fn binding_layout_descriptors_filter_by_space() {
        let artifact = artifact();
        let layout = artifact.binding_layout_descriptor(0).expect("layout");
        assert_eq!(layout.entries.len(), 1);
        assert_eq!(layout.entries[0].binding_type, BindingType::Texture);
        assert_eq!(
            artifact
                .binding_layout_descriptor(1)
                .expect("layout")
                .entries
                .len(),
            0
        );
    }
}
