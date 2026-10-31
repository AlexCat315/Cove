//! Translation of Primitives/Text/Code/Minimap.swift.
//!
//! A scaled overview of the whole document on the trailing edge, with the visible
//! region marked. Only the miniatures intersecting the viewport are drawn, so a
//! large document costs one row of work per visible row.
use cove_core::{Color, Rect};

/// One miniature row: its vertical span and the visible fraction of its ink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapRow {
    pub top: f32,
    pub height: f32,
    /// Fraction of the row that carries characters, used to shorten the rule.
    pub ink: f32,
}

/// Geometry and drawing for a minimap column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapGeometry {
    pub width: f32,
    pub height: f32,
    /// Total content height in document points, before scaling.
    pub content_height: f32,
    /// Current scroll offset of the editor viewport.
    pub offset: f32,
    /// Height of the editor viewport.
    pub viewport_height: f32,
}

impl MinimapGeometry {
    pub fn new(width: f32, height: f32, content_height: f32) -> Self {
        Self {
            width: if width.is_finite() {
                width.max(0.0)
            } else {
                0.0
            },
            height: if height.is_finite() {
                height.max(0.0)
            } else {
                0.0
            },
            content_height: if content_height.is_finite() {
                content_height.max(0.0)
            } else {
                0.0
            },
            offset: 0.0,
            viewport_height: 0.0,
        }
    }

    pub fn scrolled(mut self, offset: f32, viewport_height: f32) -> Self {
        self.offset = if offset.is_finite() {
            offset.max(0.0)
        } else {
            0.0
        };
        self.viewport_height = if viewport_height.is_finite() {
            viewport_height.max(0.0)
        } else {
            0.0
        };
        self
    }

    /// Document points to minimap points.
    pub fn scale(&self) -> f32 {
        if self.content_height <= 0.0 {
            0.0
        } else {
            self.height / self.content_height
        }
    }

    /// The visible region, clamped to the minimap.
    pub fn visible_band(&self) -> Option<Rect> {
        let scale = self.scale();
        if scale <= 0.0 || self.viewport_height <= 0.0 {
            return None;
        }
        let top = (self.offset * scale).clamp(0.0, self.height);
        let height = (self.viewport_height * scale)
            .min(self.height - top)
            .max(1.0);
        Some(Rect::new(0.0, top, self.width, height))
    }

    /// Rows whose scaled band intersects the minimap viewport.
    pub fn visible_rows(&self, rows: &[MinimapRow]) -> Vec<Rect> {
        let scale = self.scale();
        if scale <= 0.0 {
            return Vec::new();
        }
        rows.iter()
            .filter_map(|row| {
                let top = row.top * scale;
                let height = (row.height * scale).max(0.75);
                if top >= self.height || top + height <= 0.0 {
                    return None;
                }
                let width = (row.ink.clamp(0.0, 1.0) * self.width).max(1.0);
                Some(Rect::new(0.0, top, width, height))
            })
            .collect()
    }
}

/// Paint the miniature rows and the visible-region marker.
pub fn draw(
    list: &mut cove_core::DrawList,
    geometry: &MinimapGeometry,
    rows: &[MinimapRow],
    origin: cove_core::Point,
    ink: Color,
    marker: Color,
    opacity: f32,
) {
    for band in geometry.visible_rows(rows) {
        let _ = list.add_rect(
            Rect::new(
                origin.x + band.x,
                origin.y + band.y,
                band.width,
                band.height,
            ),
            ink.multiplied_alpha(opacity),
        );
    }
    if let Some(region) = geometry.visible_band() {
        let _ = list.add_rect(
            Rect::new(
                origin.x,
                origin.y + region.y,
                region.width.max(geometry.width),
                region.height,
            ),
            marker.multiplied_alpha(opacity * 0.18),
        );
    }
}

/// Document offset for a minimap press, so clicking the column scrolls there.
pub fn offset_at(geometry: &MinimapGeometry, local_y: f32) -> f32 {
    if geometry.height <= 0.0 || geometry.content_height <= 0.0 {
        return 0.0;
    }
    let fraction = (local_y / geometry.height).clamp(0.0, 1.0);
    let target = fraction * geometry.content_height - geometry.viewport_height / 2.0;
    target.clamp(
        0.0,
        (geometry.content_height - geometry.viewport_height).max(0.0),
    )
}
