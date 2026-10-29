//! Translation of Theme/Tokens/SpacingScale.swift.
/// Six-step linear spacing scale in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpacingScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}
impl SpacingScale {
    pub fn new(xs: f32, sm: f32, md: f32, lg: f32, xl: f32, xxl: f32) -> Self {
        Self { xs, sm, md, lg, xl, xxl }
    }
}
