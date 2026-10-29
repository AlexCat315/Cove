//! Translation of NativeRHI/Resources/ShaderSpecialization.swift.
use super::{rhi_require, RhiError, RhiResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShaderConstantType {
    Uint32,
    Int32,
    Float32,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShaderConstantValue {
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
}
impl ShaderConstantValue {
    pub fn constant_type(self) -> ShaderConstantType {
        match self {
            Self::Uint32(_) => ShaderConstantType::Uint32,
            Self::Int32(_) => ShaderConstantType::Int32,
            Self::Float32(_) => ShaderConstantType::Float32,
            Self::Bool(_) => ShaderConstantType::Bool,
        }
    }
    pub fn bits(self) -> u32 {
        match self {
            Self::Uint32(value) => value,
            Self::Int32(value) => value as u32,
            Self::Float32(value) => value.to_bits(),
            Self::Bool(value) => u32::from(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderSpecializationConstant {
    pub id: u32,
    pub value: ShaderConstantValue,
}
impl ShaderSpecializationConstant {
    pub const fn new(id: u32, value: ShaderConstantValue) -> Self {
        Self { id, value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectedShaderConstant {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub constant_type: ShaderConstantType,
}
impl ReflectedShaderConstant {
    pub fn new(id: u32, name: impl Into<String>, constant_type: ShaderConstantType) -> Self {
        Self { id, name: name.into(), constant_type }
    }
}

/// The shader's declared local workgroup size, not the dispatch grid size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadgroupSize {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}
impl ThreadgroupSize {
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }
    pub fn validate(self) -> RhiResult<()> {
        let overflow_xy = self.x.checked_mul(self.y).is_none();
        let overflow_xyz = self.x.checked_mul(self.y).and_then(|xy| xy.checked_mul(self.z)).is_none();
        rhi_require(
            self.x > 0 && self.y > 0 && self.z > 0 && !overflow_xy && !overflow_xyz,
            "threadgroup dimensions must be positive and representable",
        )
    }
}
impl Default for ThreadgroupSize {
    fn default() -> Self {
        Self { x: 1, y: 1, z: 1 }
    }
}

/// Links physical workgroup dimensions to reflected constant IDs. A caller
/// specializes once; Metal dispatch and SPIR-V LocalSizeId then agree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadgroupSpecialization {
    pub x: Option<u32>,
    pub y: Option<u32>,
    pub z: Option<u32>,
}
impl ThreadgroupSpecialization {
    pub fn resolve(self, defaults: ThreadgroupSize, values: &[ShaderSpecializationConstant]) -> RhiResult<ThreadgroupSize> {
        let dimension = |id: Option<u32>, fallback: i64| -> RhiResult<i64> {
            let Some(id) = id else { return Ok(fallback) };
            let Some(entry) = values.iter().find(|entry| entry.id == id) else { return Ok(fallback) };
            match entry.value {
                ShaderConstantValue::Uint32(value) => Ok(i64::from(value)),
                ShaderConstantValue::Int32(value) => Ok(i64::from(value)),
                _ => Err(RhiError::invalid_argument("workgroup constant must be an integer")),
            }
        };
        let result = ThreadgroupSize {
            x: dimension(self.x, defaults.x)?,
            y: dimension(self.y, defaults.y)?,
            z: dimension(self.z, defaults.z)?,
        };
        result.validate()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_bits_round_trip_native_representations() {
        assert_eq!(ShaderConstantValue::Uint32(7).bits(), 7);
        assert_eq!(ShaderConstantValue::Int32(-1).bits(), u32::MAX);
        assert_eq!(ShaderConstantValue::Float32(1.0).bits(), 1.0f32.to_bits());
        assert_eq!(ShaderConstantValue::Bool(true).bits(), 1);
        assert_eq!(ShaderConstantValue::Float32(0.5).constant_type(), ShaderConstantType::Float32);
    }

    #[test]
    fn threadgroup_validation_rejects_empty_and_overflowing_sizes() {
        assert!(ThreadgroupSize::new(8, 8, 1).validate().is_ok());
        assert!(ThreadgroupSize::new(0, 1, 1).validate().is_err());
        assert!(ThreadgroupSize::new(i64::MAX, 2, 1).validate().is_err());
    }

    #[test]
    fn specialization_overrides_only_the_named_dimensions() {
        let values = [ShaderSpecializationConstant::new(4, ShaderConstantValue::Uint32(64))];
        let resolved = ThreadgroupSpecialization { x: Some(4), y: None, z: None }
            .resolve(ThreadgroupSize::new(8, 4, 1), &values)
            .unwrap();
        assert_eq!(resolved, ThreadgroupSize::new(64, 4, 1));
    }
}
