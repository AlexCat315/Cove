//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/ImageGeometry.swift.
//!
//! Central aspect cropping keeps rounded masks on the container boundary.

use cove_core::scene::{Point, Rect};

/// Content mode shared by `Image`, `AsyncImage` and file-backed images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageContentMode {
    Stretch,
    Fit,
    Fill,
}

/// How the sampled texel becomes paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRenderingMode {
    Color,
    AlphaMask,
}

/// Destination rectangle plus the source UV window for one image draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageGeometry {
    pub rect: Rect,
    pub uv_min: Point,
    pub uv_max: Point,
}

impl ImageGeometry {
    /// Aspect crop in the centre so rounding stays on the container edge.
    ///
    /// `source` is the source pixel extent; unknown, non-finite or empty
    /// sources fall back to the full container, exactly like the Swift
    /// initializer's early return.
    pub fn new(
        container: Rect,
        source: Option<(f32, f32)>,
        mode: ImageContentMode,
    ) -> Self {
        let mut geometry = Self {
            rect: container,
            uv_min: Point::new(0.0, 0.0),
            uv_max: Point::new(1.0, 1.0),
        };
        let Some((source_width, source_height)) = source else {
            return geometry;
        };
        if !source_width.is_finite()
            || !source_height.is_finite()
            || source_width <= 0.0
            || source_height <= 0.0
            || container.width <= 0.0
            || container.height <= 0.0
            || mode == ImageContentMode::Stretch
        {
            return geometry;
        }
        let source_aspect = source_width / source_height;
        let aspect = container.width / container.height;
        if mode == ImageContentMode::Fill {
            geometry.rect = container;
            if source_aspect > aspect {
                let span = aspect / source_aspect;
                geometry.uv_min.x = (1.0 - span) / 2.0;
                geometry.uv_max.x = (1.0 + span) / 2.0;
            } else {
                let span = source_aspect / aspect;
                geometry.uv_min.y = (1.0 - span) / 2.0;
                geometry.uv_max.y = (1.0 + span) / 2.0;
            }
        } else {
            let width = if source_aspect >= aspect {
                container.width
            } else {
                container.height * source_aspect
            };
            let height = if source_aspect >= aspect {
                container.width / source_aspect
            } else {
                container.height
            };
            geometry.rect = Rect::new(
                container.x + (container.width - width) / 2.0,
                container.y + (container.height - height) / 2.0,
                width,
                height,
            );
        }
        geometry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_crops_the_wider_source_horizontally() {
        let geometry = ImageGeometry::new(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some((200.0, 100.0)),
            ImageContentMode::Fill,
        );
        assert_eq!(geometry.rect, Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(geometry.uv_min, Point::new(0.25, 0.0));
        assert_eq!(geometry.uv_max, Point::new(0.75, 1.0));
    }

    #[test]
    fn fit_letterboxes_inside_the_container() {
        let geometry = ImageGeometry::new(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some((200.0, 100.0)),
            ImageContentMode::Fit,
        );
        assert_eq!(geometry.rect, Rect::new(0.0, 25.0, 100.0, 50.0));
    }
}
