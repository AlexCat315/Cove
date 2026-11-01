//! Typed lengths. Percentages use 0..100, matching the public layout API.
use std::cell::RefCell;
use taffy::{Dimension, LengthPercentage, LengthPercentageAuto};

/// `percent * containing_size / 100 + points`, optionally clamped in points.
/// Stored by value in authored styles; no raw handles escape the engine.
#[repr(align(8))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutCalc {
    pub percent: f32,
    pub points: f32,
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,
}
impl LayoutCalc {
    pub const fn new(percent: f32, points: f32) -> Self {
        Self { percent, points, minimum: None, maximum: None }
    }
    pub fn with_minimum(mut self, minimum: f32) -> Self {
        self.minimum = Some(self.minimum.map_or(minimum, |old| old.max(minimum)));
        self
    }
    pub fn with_maximum(mut self, maximum: f32) -> Self {
        self.maximum = Some(self.maximum.map_or(maximum, |old| old.min(maximum)));
        self
    }
    pub fn resolve(self, basis: f32) -> f32 {
        let mut value = self.percent / 100.0 * basis + self.points;
        if let Some(maximum) = self.maximum { value = value.min(maximum); }
        if let Some(minimum) = self.minimum { value = value.max(minimum); }
        value
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LayoutStyleValue {
    #[default]
    Auto,
    Points(f32),
    Percent(f32),
    Calc(LayoutCalc),
    /// Intrinsic keywords are valid for width, height and flex basis.
    MinContent,
    MaxContent,
    FitContent,
    FitContentLimit(LayoutCalc),
}
impl LayoutStyleValue {
    pub const fn calc(percent: f32, points: f32) -> Self {
        Self::Calc(LayoutCalc::new(percent, points))
    }
    pub(super) fn is_auto(self) -> bool { matches!(self, Self::Auto) }
    pub fn points(self) -> Option<f32> {
        if let Self::Points(value) = self { Some(value) } else { None }
    }
    pub(super) fn as_calc(self) -> LayoutCalc {
        match self {
            Self::Points(value) => LayoutCalc::new(0.0, value),
            Self::Percent(value) => LayoutCalc::new(value, 0.0),
            Self::Calc(value) => value,
            _ => panic!("this property requires a length, percentage or calc value"),
        }
    }
    pub(super) fn dimension(self, calc: &CalcStorage) -> Dimension {
        match self {
            Self::Auto => Dimension::auto(),
            Self::Points(value) => Dimension::length(value),
            Self::Percent(value) => Dimension::percent(value / 100.0),
            Self::Calc(value) => Dimension::calc(calc.insert(value)),
            Self::MinContent => Dimension::min_content(),
            Self::MaxContent => Dimension::max_content(),
            Self::FitContent => Dimension::fit_content(),
            Self::FitContentLimit(value) => Dimension::fit_content_calc(calc.insert(value)),
        }
    }
    pub(super) fn length_percentage(self, calc: &CalcStorage) -> LengthPercentage {
        match self {
            Self::Auto => LengthPercentage::length(0.0),
            Self::Points(value) => LengthPercentage::length(value),
            Self::Percent(value) => LengthPercentage::percent(value / 100.0),
            Self::Calc(value) => LengthPercentage::calc(calc.insert(value)),
            _ => panic!("intrinsic sizing keywords are not valid on edges, gaps or min/max constraints"),
        }
    }
    pub(super) fn length_percentage_auto(self, calc: &CalcStorage) -> LengthPercentageAuto {
        match self {
            Self::Auto => LengthPercentageAuto::auto(),
            Self::Points(value) => LengthPercentageAuto::length(value),
            Self::Percent(value) => LengthPercentageAuto::percent(value / 100.0),
            Self::Calc(value) => LengthPercentageAuto::calc(calc.insert(value)),
            _ => panic!("intrinsic sizing keywords are not valid on edges, gaps or min/max constraints"),
        }
    }
    pub(super) fn margin(self, calc: &CalcStorage) -> LengthPercentageAuto {
        // Preserve Cove's existing unset-margin contract.
        if self.is_auto() { LengthPercentageAuto::length(0.0) }
        else { self.length_percentage_auto(calc) }
    }
}

/// Boxes keep aligned handles stable as the vector grows. This storage is
/// rebuilt with each engine style, and lives as long as that style's handles.
#[derive(Default)]
pub(super) struct CalcStorage(RefCell<Vec<Box<LayoutCalc>>>);
impl CalcStorage {
    pub fn insert(&self, value: LayoutCalc) -> *const () {
        assert!(value.percent.is_finite() && value.points.is_finite());
        assert!(value.minimum.is_none_or(f32::is_finite) && value.maximum.is_none_or(f32::is_finite));
        let value = Box::new(value);
        let ptr = (&*value as *const LayoutCalc).cast();
        self.0.borrow_mut().push(value);
        ptr
    }
}

/// Called only by Taffy with handles produced by CalcStorage. The engine
/// owns those boxes and never refreshes a style while it is being evaluated.
pub(super) fn resolve_calc(ptr: *const (), basis: f32) -> f32 {
    // SAFETY: all calc handles in this private tree originate in CalcStorage.
    unsafe { &*ptr.cast::<LayoutCalc>() }.resolve(basis)
}
