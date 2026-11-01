//! Translation of Theme/Tokens/ElevationScale.swift.
use cove_core::{LayeredShadow, SurfaceShadow};
/// Four-step elevation scale plus the two-layer shadow floating surfaces use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ElevationScale {
    pub none: SurfaceShadow,
    pub low: SurfaceShadow,
    pub medium: SurfaceShadow,
    pub high: SurfaceShadow,
    pub floating: LayeredShadow,
}
impl ElevationScale {
    pub fn new(
        none: SurfaceShadow,
        low: SurfaceShadow,
        medium: SurfaceShadow,
        high: SurfaceShadow,
        floating: LayeredShadow,
    ) -> Self {
        Self {
            none,
            low,
            medium,
            high,
            floating,
        }
    }
}
