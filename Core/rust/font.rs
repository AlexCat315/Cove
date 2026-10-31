//! Translation of CoveUICore/Font.swift.
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontWeight {
    #[default]
    Regular,
    Medium,
    Semibold,
    Bold,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontDesign {
    #[default]
    Standard,
    Monospaced,
}

#[derive(Clone, Debug)]
pub struct Font {
    size: f32,
    weight: FontWeight,
    design: FontDesign,
    family: Option<String>,
    is_italic: bool,
}
impl Font {
    pub fn new(
        size: f32,
        weight: FontWeight,
        design: FontDesign,
        family: Option<String>,
        is_italic: bool,
    ) -> Self {
        Self {
            size: if size.is_finite() { size.max(1.0) } else { 1.0 },
            weight,
            design,
            family,
            is_italic,
        }
    }
    pub fn system(size: f32, weight: FontWeight) -> Self {
        Self::new(size, weight, FontDesign::Standard, None, false)
    }
    pub fn named(family: impl Into<String>, size: f32, weight: FontWeight) -> Self {
        Self::new(
            size,
            weight,
            FontDesign::Standard,
            Some(family.into()),
            false,
        )
    }
    pub fn monospaced(size: f32, weight: FontWeight) -> Self {
        Self::new(size, weight, FontDesign::Monospaced, None, false)
    }
    pub fn with_italic(&self, is_italic: bool) -> Self {
        Self {
            is_italic,
            ..self.clone()
        }
    }
    pub fn with_weight(&self, weight: FontWeight) -> Self {
        Self {
            weight,
            ..self.clone()
        }
    }
    pub fn with_size(&self, size: f32) -> Self {
        Self::new(
            size,
            self.weight,
            self.design,
            self.family.clone(),
            self.is_italic,
        )
    }
    pub fn size(&self) -> f32 {
        self.size
    }
    pub fn weight(&self) -> FontWeight {
        self.weight
    }
    pub fn design(&self) -> FontDesign {
        self.design
    }
    pub fn family(&self) -> Option<&str> {
        self.family.as_deref()
    }
    pub fn is_italic(&self) -> bool {
        self.is_italic
    }
}
impl PartialEq for Font {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.weight == other.weight
            && self.design == other.design
            && self.family == other.family
            && self.is_italic == other.is_italic
    }
}
impl Eq for Font {}
impl Hash for Font {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.size.to_bits().hash(state);
        self.weight.hash(state);
        self.design.hash(state);
        self.family.hash(state);
        self.is_italic.hash(state);
    }
}
