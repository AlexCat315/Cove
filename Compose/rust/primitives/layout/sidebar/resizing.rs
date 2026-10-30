//! Translation of Primitives/Layout/Sidebar/Resizing.swift.
//!
//! Bounds for a caller-owned sidebar width. Layout fitting never writes a
//! narrower temporary allocation back into the caller's requested width.
//!
//! The bounds value itself lives in `presentation::SidebarResizeOptions`, next to
//! the presentation it constrains; this module keeps the pure range arithmetic
//! so both the handle and the adaptive host share one implementation.
pub use super::presentation::SidebarResizeOptions;

/// Clamp `proposed` into the range that leaves room for the handle and the
/// minimum content width.
pub fn clamp_width(options: &SidebarResizeOptions, proposed: f32, width: f32, handle: f32) -> f32 {
    let (low, high) = options.range(width, handle);
    if proposed.is_finite() { proposed.clamp(low, high) } else { low }
}

/// The width a resolved option set settles on for a container.
pub fn resolved_width(options: &SidebarResizeOptions, requested: f32, width: f32, handle: f32) -> f32 {
    clamp_width(options, requested, width, handle)
}
