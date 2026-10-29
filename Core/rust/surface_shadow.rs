//! Translation of CoveUICore/SurfaceShadow.swift.
use crate::Color;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceShadow { pub color: Option<Color>, pub offset_x: f32, pub offset_y: f32, pub blur: f32 }
impl SurfaceShadow {
    pub const NONE: Self = Self { color: None, offset_x: 0.0, offset_y: 0.0, blur: 0.0 };
    pub fn is_empty(self) -> bool { self.color.is_none_or(|color| color.a <= 0.0) }
    /// Resolve authored lengths at paint time without changing the node's style.
    pub fn scaled(self, factor: f32) -> Self {
        Self { offset_x: self.offset_x * factor, offset_y: self.offset_y * factor, blur: self.blur * factor.abs(), ..self }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayeredShadow { pub ambient: SurfaceShadow, pub key: SurfaceShadow }
impl LayeredShadow {
    pub const NONE: Self = Self { ambient: SurfaceShadow::NONE, key: SurfaceShadow::NONE };
    pub const fn single(key: SurfaceShadow) -> Self { Self { ambient: SurfaceShadow::NONE, key } }
}
