//! Translation of CoveUICore/UIReveal.swift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reveal {
    pub sequence: u64,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub feather: f32,
}
impl Reveal {
    pub fn new(sequence: u64, center_x: f32, center_y: f32, radius: f32, feather: f32) -> Self {
        Self {
            sequence,
            center_x,
            center_y,
            radius: radius.max(0.0),
            feather: feather.max(0.0),
        }
    }
    pub fn is_visible(self) -> bool {
        self.radius > 0.0
    }
}
