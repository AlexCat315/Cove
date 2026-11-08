//! Authored layout style and its translation into the Taffy flexbox model.
//!
//! The vocabulary is Yoga's (Swift wrapped `YGNodeRef`): column by default,
//! `flex-shrink: 0`, `align-content: flex-start`, no automatic minimum size,
//! and logical `start`/`end` edges that bind to the direction in force at
//! layout time.
use super::super::{
    Align, BoxSizing, Direction, Display, Edge, FlexDirection, Justify, Overflow, PositionType,
    Wrap,
};
use crate::Size;
use taffy::style::{
    AlignContent, AlignItems, JustifyContent, LengthPercentage, LengthPercentageAuto,
};

use super::grid::GridStyle;
use super::value::CalcStorage;
pub use super::value::{LayoutCalc, LayoutStyleValue};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
impl LayoutInsets {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
    pub(super) fn from_taffy(rect: taffy::Rect<f32>) -> Self {
        Self::new(rect.top, rect.right, rect.bottom, rect.left)
    }
}

/// Per-edge style values. Logical `start`/`end` resolve against the direction
/// in force at layout time and take precedence over `left`/`right`, as in Yoga.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StyleEdges {
    pub top: LayoutStyleValue,
    pub right: LayoutStyleValue,
    pub bottom: LayoutStyleValue,
    pub left: LayoutStyleValue,
    pub start: LayoutStyleValue,
    pub end: LayoutStyleValue,
}
impl StyleEdges {
    pub fn set(&mut self, edge: Edge, value: LayoutStyleValue) {
        match edge {
            Edge::All => {
                self.top = value;
                self.right = value;
                self.bottom = value;
                self.left = value;
                self.start = LayoutStyleValue::Auto;
                self.end = LayoutStyleValue::Auto;
            }
            Edge::Horizontal => {
                self.right = value;
                self.left = value;
                self.start = LayoutStyleValue::Auto;
                self.end = LayoutStyleValue::Auto;
            }
            Edge::Vertical => {
                self.top = value;
                self.bottom = value;
            }
            Edge::Top => self.top = value,
            Edge::Bottom => self.bottom = value,
            Edge::Left => self.left = value,
            Edge::Right => self.right = value,
            Edge::Start => self.start = value,
            Edge::End => self.end = value,
        }
    }
    /// Physical `(left, right)` for `direction`.
    fn horizontal(self, direction: Direction) -> (LayoutStyleValue, LayoutStyleValue) {
        let (leading, trailing) = if direction == Direction::Rtl {
            (self.end, self.start)
        } else {
            (self.start, self.end)
        };
        let pick = |logical: LayoutStyleValue, physical| {
            if logical.is_auto() { physical } else { logical }
        };
        (pick(leading, self.left), pick(trailing, self.right))
    }
    fn map<T>(
        self,
        direction: Direction,
        convert: impl Fn(LayoutStyleValue) -> T,
    ) -> taffy::Rect<T> {
        let (left, right) = self.horizontal(direction);
        taffy::Rect {
            left: convert(left),
            right: convert(right),
            top: convert(self.top),
            bottom: convert(self.bottom),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StyleBorder {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
impl StyleBorder {
    pub(super) fn set(&mut self, edge: Edge, value: f32) {
        match edge {
            Edge::All => {
                self.top = value;
                self.right = value;
                self.bottom = value;
                self.left = value;
            }
            Edge::Horizontal => {
                self.right = value;
                self.left = value;
            }
            Edge::Vertical => {
                self.top = value;
                self.bottom = value;
            }
            Edge::Top => self.top = value,
            Edge::Bottom => self.bottom = value,
            Edge::Left | Edge::Start => self.left = value,
            Edge::Right | Edge::End => self.right = value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutStyle {
    pub direction: Direction,
    pub flex_direction: FlexDirection,
    pub align_items: Align,
    pub align_content: Align,
    pub justify_content: Justify,
    pub flex_wrap: Wrap,
    pub overflow: Overflow,
    pub display: Display,
    pub align_self: Align,
    pub position_type: PositionType,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: LayoutStyleValue,
    pub width: LayoutStyleValue,
    pub height: LayoutStyleValue,
    pub min_width: LayoutStyleValue,
    pub max_width: LayoutStyleValue,
    pub min_height: LayoutStyleValue,
    pub max_height: LayoutStyleValue,
    pub position: StyleEdges,
    pub margin: StyleEdges,
    pub padding: StyleEdges,
    pub border: StyleBorder,
    pub gap_row: LayoutStyleValue,
    pub gap_column: LayoutStyleValue,
    pub box_sizing: BoxSizing,
    pub grid: GridStyle,
    /// Width over height applied when one dimension is auto.
    pub aspect_ratio: Option<f32>,
}
impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            direction: Direction::Inherit,
            flex_direction: FlexDirection::Column,
            align_items: Align::Stretch,
            align_content: Align::FlexStart,
            justify_content: Justify::FlexStart,
            flex_wrap: Wrap::NoWrap,
            overflow: Overflow::Visible,
            display: Display::Flex,
            align_self: Align::Auto,
            position_type: PositionType::Relative,
            flex_grow: 0.0,
            flex_shrink: 0.0,
            flex_basis: LayoutStyleValue::Auto,
            width: LayoutStyleValue::Auto,
            height: LayoutStyleValue::Auto,
            min_width: LayoutStyleValue::Auto,
            max_width: LayoutStyleValue::Auto,
            min_height: LayoutStyleValue::Auto,
            max_height: LayoutStyleValue::Auto,
            position: StyleEdges::default(),
            margin: StyleEdges::default(),
            padding: StyleEdges::default(),
            border: StyleBorder::default(),
            gap_row: LayoutStyleValue::Auto,
            gap_column: LayoutStyleValue::Auto,
            box_sizing: BoxSizing::BorderBox,
            grid: GridStyle::default(),
            aspect_ratio: None,
        }
    }
}

impl LayoutStyle {
    /// The Taffy style for this node under the resolved (`Ltr`/`Rtl`)
    /// `direction`.
    pub(super) fn to_taffy(
        &self,
        direction: Direction,
        debug_padding: Option<super::LayoutInsets>,
        floor: Option<Size>,
        calc: &CalcStorage,
    ) -> taffy::Style {
        let minimum = |value: LayoutStyleValue, floor: Option<f32>| match (value, floor) {
            (LayoutStyleValue::Auto, floor) => LengthPercentageAuto::length(floor.unwrap_or(0.0)),
            (LayoutStyleValue::Points(value), floor) => {
                LengthPercentageAuto::length(value.max(floor.unwrap_or(0.0)))
            }
            (value, Some(floor)) => {
                LengthPercentageAuto::calc(calc.insert(value.as_calc().with_minimum(floor)))
            }
            (value, None) => value.length_percentage_auto(calc),
        };
        let maximum = |value: LayoutStyleValue| value.length_percentage_auto(calc);
        let padding = match debug_padding {
            Some(insets) => taffy::Rect {
                left: LengthPercentage::length(insets.left),
                right: LengthPercentage::length(insets.right),
                top: LengthPercentage::length(insets.top),
                bottom: LengthPercentage::length(insets.bottom),
            },
            None => self
                .padding
                .map(direction, |value| value.length_percentage(calc)),
        };
        let gap = |value: LayoutStyleValue| match value {
            LayoutStyleValue::Auto => LengthPercentage::length(0.0),
            value => value.length_percentage(calc),
        };
        let mut result = taffy::Style {
            display: match self.display {
                Display::None => taffy::Display::None,
                Display::Grid => taffy::Display::Grid,
                Display::Flex | Display::Contents => taffy::Display::Flex,
            },
            box_sizing: match self.box_sizing {
                BoxSizing::BorderBox => taffy::BoxSizing::BorderBox,
                BoxSizing::ContentBox => taffy::BoxSizing::ContentBox,
            },
            direction: if direction == Direction::Rtl {
                taffy::Direction::Rtl
            } else {
                taffy::Direction::Ltr
            },
            overflow: {
                let overflow = match self.overflow {
                    Overflow::Visible => taffy::Overflow::Visible,
                    Overflow::Hidden => taffy::Overflow::Hidden,
                };
                taffy::Point {
                    x: overflow,
                    y: overflow,
                }
            },
            position: match self.position_type {
                PositionType::Static => taffy::Position::Static,
                PositionType::Relative => taffy::Position::Relative,
                PositionType::Absolute => taffy::Position::Absolute,
            },
            inset: self
                .position
                .map(direction, |value| value.length_percentage_auto(calc)),
            size: taffy::Size {
                width: self.width.dimension(calc),
                height: self.height.dimension(calc),
            },
            aspect_ratio: self
                .aspect_ratio
                .filter(|ratio| ratio.is_finite() && *ratio > 0.0),
            min_size: taffy::Size {
                width: minimum(self.min_width, floor.map(|floor| floor.width)),
                height: minimum(self.min_height, floor.map(|floor| floor.height)),
            },
            max_size: taffy::Size {
                width: maximum(self.max_width),
                height: maximum(self.max_height),
            },
            margin: self.margin.map(direction, |value| value.margin(calc)),
            padding,
            border: taffy::Rect {
                left: LengthPercentage::length(self.border.left),
                right: LengthPercentage::length(self.border.right),
                top: LengthPercentage::length(self.border.top),
                bottom: LengthPercentage::length(self.border.bottom),
            },
            align_items: align_items(self.align_items),
            align_self: match self.align_self {
                Align::Auto => None,
                align => Some(align_items(align)),
            },
            align_content: align_content(self.align_content),
            justify_content: match self.justify_content {
                Justify::FlexStart => JustifyContent::FLEX_START,
                Justify::Center => JustifyContent::CENTER,
                Justify::FlexEnd => JustifyContent::FLEX_END,
                Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
                Justify::SpaceAround => JustifyContent::SPACE_AROUND,
                Justify::SpaceEvenly => JustifyContent::SPACE_EVENLY,
            },
            gap: taffy::Size {
                width: gap(self.gap_column),
                height: gap(self.gap_row),
            },
            flex_direction: match self.flex_direction {
                FlexDirection::Row => taffy::FlexDirection::Row,
                FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
                FlexDirection::Column => taffy::FlexDirection::Column,
                FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
            },
            flex_wrap: match self.flex_wrap {
                Wrap::NoWrap => taffy::FlexWrap::NoWrap,
                Wrap::Wrap => taffy::FlexWrap::Wrap,
                Wrap::WrapReverse => taffy::FlexWrap::WrapReverse,
            },
            flex_basis: self.flex_basis.dimension(calc),
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            ..taffy::Style::DEFAULT
        };
        self.grid.apply(&mut result, calc);
        result
    }
}

pub(super) fn align_items(align: Align) -> AlignItems {
    match align {
        Align::Auto | Align::Stretch => AlignItems::STRETCH,
        Align::FlexStart => AlignItems::FLEX_START,
        Align::Center => AlignItems::CENTER,
        Align::FlexEnd => AlignItems::FLEX_END,
        Align::Baseline => AlignItems::BASELINE,
        Align::SpaceBetween | Align::SpaceAround | Align::SpaceEvenly => AlignItems::FLEX_START,
    }
}

fn align_content(align: Align) -> AlignContent {
    match align {
        Align::Auto | Align::FlexStart | Align::Baseline => AlignContent::FLEX_START,
        Align::Center => AlignContent::CENTER,
        Align::FlexEnd => AlignContent::FLEX_END,
        Align::Stretch => AlignContent::STRETCH,
        Align::SpaceBetween => AlignContent::SPACE_BETWEEN,
        Align::SpaceAround => AlignContent::SPACE_AROUND,
        Align::SpaceEvenly => AlignContent::SPACE_EVENLY,
    }
}
