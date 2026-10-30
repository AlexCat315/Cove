//! Translation of Primitives/Controls/Rating/RatingGeometry.swift.
//!
//! Symbol extents and the pointer-to-value mapping for a rating row.
use cove_core::{Point, Rect};
use crate::theme::ControlSize;

use super::rating::{RatingAppearance, RatingPrecision, RatingSelection};

/// Symbol size, spacing and the value a pointer position selects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RatingGeometry {
    pub star_size: f32,
    pub spacing: f32,
    pub maximum: usize,
    pub cell_width: f32,
}

impl RatingGeometry {
    pub fn new(appearance: &RatingAppearance, selection: &RatingSelection, density: ControlSize, touch: bool, minimum_extent: f32) -> Self {
        let size = appearance.size.unwrap_or(density);
        let star_size = appearance.star_size.unwrap_or(match size {
            ControlSize::Mini => 12.0,
            ControlSize::Small => 16.0,
            ControlSize::Large => 28.0,
            _ => 20.0,
        });
        let cell_width = (star_size + 8.0).max(if selection.is_interactive() && touch {
            minimum_extent
        } else {
            0.0
        });
        Self {
            star_size,
            spacing: appearance.spacing,
            maximum: selection.maximum,
            cell_width,
        }
    }

    pub fn stride(&self) -> f32 {
        self.cell_width + self.spacing
    }

    pub fn width(&self) -> f32 {
        8.0
            + self.maximum as f32 * self.cell_width
            + self.maximum.saturating_sub(1) as f32 * self.spacing
    }

    pub fn height(&self) -> f32 {
        32.0_f32.max(self.cell_width)
    }

    /// Rect for one symbol. Direction is resolved at draw time, so a late
    /// right-to-left change cannot use a stale index.
    pub fn star_rect(&self, index: usize, origin: Point, height: f32, right_to_left: bool) -> Rect {
        let visual_index = if right_to_left {
            self.maximum.saturating_sub(1).saturating_sub(index)
        } else {
            index
        };
        Rect::new(
            origin.x + 4.0 + (self.cell_width - self.star_size) / 2.0 + visual_index as f32 * self.stride(),
            origin.y + (height - self.star_size) / 2.0,
            self.star_size,
            self.star_size,
        )
    }

    /// The value a pointer position selects.
    pub fn candidate(&self, event_x: f32, origin_x: f32, right_to_left: bool, precision: RatingPrecision) -> f64 {
        if !event_x.is_finite() || self.maximum == 0 {
            return 0.0;
        }
        let x = (event_x - origin_x - 4.0).clamp(0.0, self.width());
        let visual_index = ((x / self.stride()) as usize).min(self.maximum - 1);
        let fraction = ((x - visual_index as f32 * self.stride()
            - (self.cell_width - self.star_size) / 2.0)
            / self.star_size)
            .clamp(0.0, 1.0);
        let index = if right_to_left {
            self.maximum - 1 - visual_index
        } else {
            visual_index
        };
        index as f64 + precision.part(if right_to_left { 1.0 - fraction } else { fraction })
    }

    /// True when the point lies inside the row's painted extent.
    pub fn contains(&self, x: f32, y: f32, origin: Point, frame: cove_core::Size) -> bool {
        x >= origin.x
            && y >= origin.y
            && x < origin.x + self.width().min(frame.width)
            && y < origin.y + frame.height
    }
}
