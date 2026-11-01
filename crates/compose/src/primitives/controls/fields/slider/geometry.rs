//! Translation of `Primitives/Controls/Fields/Slider/Geometry.swift`.
//!
//! One axis mapping shared by pointer input, fill, thumb and ticks. Swift's
//! `Slider.Axis` is a nested type; the Rust image names it `SliderAxis` so the
//! geometry module can be used before the `Slider` control itself is ported.

use cove_core::scene::{Direction, Node, Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SliderAxis {
    Horizontal,
    Vertical,
}

impl SliderAxis {
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderGeometry {
    pub axis: SliderAxis,
    pub length: f32,
    pub breadth: f32,
    pub is_reversed: bool,
}

impl SliderGeometry {
    pub fn new(node: &Node, axis: SliderAxis) -> Self {
        let frame = node.frame();
        let (length, breadth) = if axis.is_horizontal() {
            (frame.width, frame.height)
        } else {
            (frame.height, frame.width)
        };

        // The nearest explicit layout direction decides reversal; inherited
        // directions keep walking up until an ancestor resolves one.
        let mut layout = node.layout_node();
        let mut direction = Direction::Inherit;
        while let Some(current) = layout {
            direction = current.direction();
            if direction != Direction::Inherit {
                break;
            }
            layout = current.parent();
        }

        Self {
            axis,
            length,
            breadth,
            is_reversed: axis == SliderAxis::Vertical || direction == Direction::Rtl,
        }
    }

    pub fn value_fraction(&self, position: f32) -> f32 {
        let fraction = (position / self.length.max(1.0)).clamp(0.0, 1.0);
        if self.is_reversed {
            1.0 - fraction
        } else {
            fraction
        }
    }

    pub fn position(&self, fraction: f32) -> f32 {
        let fraction = if self.is_reversed {
            1.0 - fraction
        } else {
            fraction
        };
        fraction * self.length
    }

    pub fn rect(&self, start: f32, extent: f32, thickness: f32, origin: Point) -> Rect {
        let cross = (self.breadth - thickness) / 2.0;
        if self.axis.is_horizontal() {
            Rect::new(origin.x + start, origin.y + cross, extent, thickness)
        } else {
            Rect::new(origin.x + cross, origin.y + start, thickness, extent)
        }
    }
}

impl SliderGeometry {
    /// Knob rectangle centred on `center` along the axis.
    pub fn thumb_rect(&self, center: f32, length: f32, diameter: f32, origin: Point) -> Rect {
        if self.axis.is_horizontal() {
            Rect::new(
                origin.x + center - length / 2.0,
                origin.y + (self.breadth - diameter) / 2.0,
                length,
                diameter,
            )
        } else {
            Rect::new(
                origin.x + (self.breadth - diameter) / 2.0,
                origin.y + center - length / 2.0,
                diameter,
                length,
            )
        }
    }
}
