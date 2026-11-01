//! Translation of CoveUIScene/FlexProps.swift and Layout/LayoutDirection.swift.
//!
//! Swift forwards these to Yoga (`YGFlexDirection`, `YGAlign`, …). The Rust
//! layout engine consumes the same vocabulary directly.

/// Flex layout direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}
impl FlexDirection {
    pub fn is_row(self) -> bool {
        matches!(self, Self::Row | Self::RowReverse)
    }
    pub fn is_reverse(self) -> bool {
        matches!(self, Self::RowReverse | Self::ColumnReverse)
    }
}

/// Cross-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    #[default]
    Auto,
    FlexStart,
    Center,
    FlexEnd,
    Stretch,
    Baseline,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Main-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Justify {
    #[default]
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Box edge selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    Left,
    Top,
    Right,
    Bottom,
    Start,
    End,
    Horizontal,
    Vertical,
    All,
}

/// text/layout direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    #[default]
    Inherit,
    Ltr,
    Rtl,
}

/// Position type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PositionType {
    Static,
    #[default]
    Relative,
    Absolute,
}

/// Flex wrap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Overflow behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
}

/// Display type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    None,
    Contents,
}

/// Gutter (gap) axis selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gutter {
    Column,
    Row,
    All,
}

/// Box sizing model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoxSizing {
    #[default]
    BorderBox,
    ContentBox,
}

/// Effective writing direction from the nearest explicit `layoutDirection`.
///
/// Direction can be applied after a primitive's update pass, so input and
/// drawing resolve it lazily rather than snapshotting a stale value.
pub fn resolves_right_to_left(directions: impl Iterator<Item = Direction>) -> bool {
    for direction in directions {
        match direction {
            Direction::Rtl => return true,
            Direction::Ltr => return false,
            Direction::Inherit => {}
        }
    }
    false
}

impl super::Node {
    /// Effective writing direction from the nearest explicit `layoutDirection`.
    ///
    /// Direction can be applied after a primitive's update pass, so input and
    /// drawing resolve it lazily rather than snapshotting a stale value.
    pub fn resolves_right_to_left(&self) -> bool {
        let mut layout = self.layout_node();
        while let Some(current) = layout {
            if current.direction() != Direction::Inherit {
                return current.direction() == Direction::Rtl;
            }
            layout = current.parent();
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_explicit_direction_wins() {
        assert!(resolves_right_to_left(
            [Direction::Inherit, Direction::Rtl, Direction::Ltr].into_iter()
        ));
        assert!(!resolves_right_to_left(
            [Direction::Inherit, Direction::Inherit].into_iter()
        ));
    }
    #[test]
    fn axis_helpers() {
        assert!(FlexDirection::RowReverse.is_row());
        assert!(FlexDirection::ColumnReverse.is_reverse());
        assert!(!FlexDirection::Row.is_reverse());
    }
}
