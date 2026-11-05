//! Translation of the shared "is this a drag yet" rule.
//!
//! Every component that must distinguish a tap from a drag reads its dead zone
//! from here, so raising or lowering drag sensitivity is one change rather
//! than a per-component constant that silently drifts.

/// Movement, in points, before a press reads as a drag rather than a tap.
pub const DRAG_THRESHOLD: f32 = 6.0;

/// True once the pointer has left the tap dead zone in any direction.
pub fn crossed(dx: f32, dy: f32) -> bool {
    (dx * dx + dy * dy).sqrt() >= DRAG_THRESHOLD
}

/// True once the pointer has travelled `distance` along the gesture's axis.
pub fn crossed_along_axis(distance: f32) -> bool {
    distance.abs() >= DRAG_THRESHOLD
}
