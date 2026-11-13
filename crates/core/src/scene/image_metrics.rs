//! Decoded-versus-display pixel sizes recorded by the image primitive.
//!
//! The image primitive writes these attachments on every paint, using the
//! decoded source extent and the destination rect scaled by the draw list's
//! pixel scale. Diagnostics read them back to report only real differences;
//! a node without these attachments is not an image or has no known source
//! size, and must never be reported as an oversized image.

use super::Node;

/// The decoded bitmap extent, in source pixels.
pub const IMAGE_SOURCE_PIXELS: &str = "CoveUI.image.sourcePixels";
/// The painted extent, in device pixels (`rect * draw list pixel scale`).
pub const IMAGE_DISPLAY_PIXELS: &str = "CoveUI.image.displayPixels";
/// Set for sources rasterized at the draw size (SVG/PDF), which cannot be
/// larger than their display rect by construction.
pub const IMAGE_VECTOR_SOURCE: &str = "CoveUI.image.vector";

/// The source and display extents one image paint recorded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePixelMetrics {
    pub source: (f32, f32),
    pub display: (f32, f32),
    pub vector: bool,
}

impl ImagePixelMetrics {
    /// Both extents are finite and positive, so an area ratio is meaningful.
    pub fn is_valid(self) -> bool {
        valid_extent(self.source) && valid_extent(self.display)
    }

    pub fn source_area(self) -> f32 {
        self.source.0 * self.source.1
    }

    pub fn display_area(self) -> f32 {
        self.display.0 * self.display.1
    }

    /// Source area divided by display area, or `None` when either extent is
    /// unknown, zero, negative or not finite.
    pub fn area_ratio(self) -> Option<f32> {
        if !self.is_valid() {
            return None;
        }
        let ratio = self.source_area() / self.display_area();
        ratio.is_finite().then_some(ratio)
    }
}

fn valid_extent((width, height): (f32, f32)) -> bool {
    width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0
}

/// Record one image paint's real source and display extents.
pub fn record_image_pixel_metrics(
    node: &Node,
    source: (f32, f32),
    display: (f32, f32),
    vector: bool,
) {
    node.set_attachment(IMAGE_SOURCE_PIXELS, source);
    node.set_attachment(IMAGE_DISPLAY_PIXELS, display);
    node.set_attachment(IMAGE_VECTOR_SOURCE, vector);
}

/// The metrics the node's most recent paint recorded.
pub fn image_pixel_metrics(node: &Node) -> Option<ImagePixelMetrics> {
    Some(ImagePixelMetrics {
        source: node.attachment::<(f32, f32)>(IMAGE_SOURCE_PIXELS)?,
        display: node.attachment::<(f32, f32)>(IMAGE_DISPLAY_PIXELS)?,
        vector: node
            .attachment::<bool>(IMAGE_VECTOR_SOURCE)
            .unwrap_or(false),
    })
}

/// Drop stale metrics when the current source has no known pixel size.
pub fn clear_image_pixel_metrics(node: &Node) {
    node.remove_attachment(IMAGE_SOURCE_PIXELS);
    node.remove_attachment(IMAGE_DISPLAY_PIXELS);
    node.remove_attachment(IMAGE_VECTOR_SOURCE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_require_finite_positive_extents() {
        let valid = ImagePixelMetrics {
            source: (400.0, 300.0),
            display: (100.0, 75.0),
            vector: false,
        };
        assert!(valid.is_valid());
        assert_eq!(valid.area_ratio(), Some(16.0));

        for display in [
            (0.0, 75.0),
            (-1.0, 75.0),
            (f32::NAN, 1.0),
            (1.0, f32::INFINITY),
        ] {
            let invalid = ImagePixelMetrics { display, ..valid };
            assert!(!invalid.is_valid());
            assert_eq!(invalid.area_ratio(), None);
        }
    }

    #[test]
    fn attachments_round_trip_and_clear() {
        let node = Node::new();
        assert!(image_pixel_metrics(&node).is_none());
        record_image_pixel_metrics(&node, (640.0, 480.0), (160.0, 120.0), false);
        assert_eq!(
            image_pixel_metrics(&node),
            Some(ImagePixelMetrics {
                source: (640.0, 480.0),
                display: (160.0, 120.0),
                vector: false,
            })
        );
        clear_image_pixel_metrics(&node);
        assert!(image_pixel_metrics(&node).is_none());
    }
}
