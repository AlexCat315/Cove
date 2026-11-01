//! Translation of Theme/Tokens/RadiusScale.swift.
/// Corner-radius scale in logical pixels. `pill` is large enough to fully
/// round any reasonable control height.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RadiusScale {
    pub none: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub pill: f32,
}
impl RadiusScale {
    pub fn new(none: f32, sm: f32, md: f32, lg: f32, xl: f32, pill: f32) -> Self {
        Self {
            none,
            sm,
            md,
            lg,
            xl,
            pill,
        }
    }
}
