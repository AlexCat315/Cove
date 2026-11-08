//! Folds, hinges and the window segments they create.
use serde::{Deserialize, Serialize};

use crate::{Point, Rect};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Posture {
    #[default]
    Flat,
    /// Half open with a horizontal fold: content above, controls below.
    Tabletop,
    /// Half open with a vertical fold: two facing pages.
    Book,
}

impl Posture {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Tabletop => "tabletop",
            Self::Book => "book",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Flat, Self::Tabletop, Self::Book]
            .into_iter()
            .find(|posture| posture.name().eq_ignore_ascii_case(value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayFeatureKind {
    /// A flexible display crease.
    Fold,
    /// A physical gap between two panels.
    Hinge,
    Cutout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureOrientation {
    /// Runs left to right, splitting the window into top and bottom.
    Horizontal,
    /// Runs top to bottom, splitting the window into left and right.
    Vertical,
}

/// A region of the window, in logical window coordinates, that content must
/// respect. Coordinates are physical (left to right), not leading/trailing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayFeature {
    pub kind: DisplayFeatureKind,
    pub bounds: Rect,
    pub orientation: FeatureOrientation,
    /// Nothing can be drawn inside `bounds`.
    #[serde(default)]
    pub occludes: bool,
    /// The feature divides the window into two logical segments.
    #[serde(default)]
    pub separates: bool,
}

impl DisplayFeature {
    pub fn fold(bounds: Rect, orientation: FeatureOrientation, separates: bool) -> Self {
        Self {
            kind: DisplayFeatureKind::Fold,
            bounds,
            orientation,
            occludes: false,
            separates,
        }
    }

    pub fn hinge(bounds: Rect, orientation: FeatureOrientation) -> Self {
        Self {
            kind: DisplayFeatureKind::Hinge,
            bounds,
            orientation,
            occludes: true,
            separates: true,
        }
    }

    /// The feature with non-finite or negative extents removed.
    pub fn normalized(self) -> Option<Self> {
        let Rect {
            x,
            y,
            width,
            height,
        } = self.bounds;
        if !(x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()) {
            return None;
        }
        Some(Self {
            bounds: Rect::new(x, y, width.max(0.0), height.max(0.0)),
            ..self
        })
    }
}

/// Splits `window` along each separating feature. Segments are ordered top to
/// bottom, then left to right. An occluding feature's area belongs to no
/// segment; a zero-width crease divides without removing space.
pub fn window_segments(window: Rect, features: &[DisplayFeature]) -> Vec<Rect> {
    let mut segments = vec![window];
    for feature in features.iter().filter(|feature| feature.separates) {
        let bounds = feature.bounds;
        segments = segments
            .into_iter()
            .flat_map(|segment| split(segment, bounds, feature.orientation))
            .collect();
    }
    segments.retain(|segment| segment.width > 0.0 && segment.height > 0.0);
    segments.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    segments
}

fn split(segment: Rect, bounds: Rect, orientation: FeatureOrientation) -> Vec<Rect> {
    match orientation {
        FeatureOrientation::Vertical => {
            let start = bounds.x;
            let end = bounds.max_x();
            if end <= segment.x || start >= segment.max_x() || !overlaps_y(segment, bounds) {
                return vec![segment];
            }
            vec![
                Rect::new(segment.x, segment.y, start - segment.x, segment.height),
                Rect::new(end, segment.y, segment.max_x() - end, segment.height),
            ]
        }
        FeatureOrientation::Horizontal => {
            let start = bounds.y;
            let end = bounds.max_y();
            if end <= segment.y || start >= segment.max_y() || !overlaps_x(segment, bounds) {
                return vec![segment];
            }
            vec![
                Rect::new(segment.x, segment.y, segment.width, start - segment.y),
                Rect::new(segment.x, end, segment.width, segment.max_y() - end),
            ]
        }
    }
}

fn overlaps_y(segment: Rect, bounds: Rect) -> bool {
    bounds.y < segment.max_y() && bounds.max_y() > segment.y
}

fn overlaps_x(segment: Rect, bounds: Rect) -> bool {
    bounds.x < segment.max_x() && bounds.max_x() > segment.x
}

/// The segment that should host `rect`: the one containing its center, else
/// the one it overlaps most, else the first.
pub fn segment_for(segments: &[Rect], rect: Rect) -> Option<Rect> {
    let center = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
    segments
        .iter()
        .copied()
        .find(|segment| contains_inclusive(*segment, center))
        .or_else(|| {
            // Ties keep the earlier segment, so a rect centered on a hinge
            // lands in the leading one.
            let mut best: Option<(Rect, f32)> = None;
            for segment in segments {
                let Some(overlap) = segment.intersection(rect) else {
                    continue;
                };
                let area = overlap.width * overlap.height;
                if best.is_none_or(|(_, current)| area > current) {
                    best = Some((*segment, area));
                }
            }
            best.map(|(segment, _)| segment)
        })
        .or_else(|| segments.first().copied())
}

fn contains_inclusive(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x <= rect.max_x() && point.y >= rect.y && point.y <= rect.max_y()
}

/// Moves `rect` the least distance needed to sit inside `segment`, shrinking it
/// only when it is larger than the segment.
pub fn confine(rect: Rect, segment: Rect) -> Rect {
    let width = rect.width.min(segment.width).max(0.0);
    let height = rect.height.min(segment.height).max(0.0);
    let x = rect.x.clamp(segment.x, segment.max_x() - width);
    let y = rect.y.clamp(segment.y, segment.max_y() - height);
    Rect::new(x, y, width, height)
}
