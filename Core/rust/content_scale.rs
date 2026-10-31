//! Translation of CoveUICore/ContentScale.swift.
use std::sync::atomic::{AtomicU32, Ordering};
static CONTENT_SCALE: AtomicU32 = AtomicU32::new(1.0_f32.to_bits());
pub fn content_scale() -> f32 {
    f32::from_bits(CONTENT_SCALE.load(Ordering::Relaxed))
}
pub fn set_content_scale(scale: f32) {
    CONTENT_SCALE.store(
        if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        }
        .to_bits(),
        Ordering::Relaxed,
    );
}
