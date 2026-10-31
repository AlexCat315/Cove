//! Translation of Primitives/Overlays/Anchored/Placement.swift.
//!
//! The edge and alignment of a bubble relative to its trigger. Leading and
//! trailing follow layout direction; explicit sides are retained by default.
use cove_core::{Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopoverPlacement {
    TopLeading,
    Top,
    TopTrailing,
    LeadingTop,
    Leading,
    LeadingBottom,
    TrailingTop,
    Trailing,
    TrailingBottom,
    BottomLeading,
    Bottom,
    BottomTrailing,
}

impl PopoverPlacement {
    pub const ALL: [Self; 12] = [
        Self::TopLeading,
        Self::Top,
        Self::TopTrailing,
        Self::LeadingTop,
        Self::Leading,
        Self::LeadingBottom,
        Self::TrailingTop,
        Self::Trailing,
        Self::TrailingBottom,
        Self::BottomLeading,
        Self::Bottom,
        Self::BottomTrailing,
    ];

    pub fn edge(self) -> PopoverEdge {
        match self {
            Self::TopLeading | Self::Top | Self::TopTrailing => PopoverEdge::Top,
            Self::BottomLeading | Self::Bottom | Self::BottomTrailing => PopoverEdge::Bottom,
            Self::LeadingTop | Self::Leading | Self::LeadingBottom => PopoverEdge::Left,
            Self::TrailingTop | Self::Trailing | Self::TrailingBottom => PopoverEdge::Right,
        }
    }

    pub fn alignment(self) -> i32 {
        match self {
            Self::TopLeading | Self::BottomLeading | Self::LeadingTop | Self::TrailingTop => -1,
            Self::TopTrailing
            | Self::BottomTrailing
            | Self::LeadingBottom
            | Self::TrailingBottom => 1,
            _ => 0,
        }
    }
}

impl Default for PopoverPlacement {
    fn default() -> Self {
        Self::BottomLeading
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopoverOverflow {
    /// Keep the requested side and scroll content within its available space.
    Constrain,
    /// Allow adaptive menus to use the opposite side when the panel fits there.
    Flip,
}

impl Default for PopoverOverflow {
    fn default() -> Self {
        Self::Flip
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopoverSurface {
    /// A shared opaque card with a pointer, border and shadow.
    Bubble,
    /// The content already owns its surface, as menus and pickers do.
    Content,
}

impl Default for PopoverSurface {
    fn default() -> Self {
        Self::Content
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopoverEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl PopoverEdge {
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// Resolved frame and edge for one anchor/size proposal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverGeometry {
    pub frame: Rect,
    pub edge: PopoverEdge,
    pub maximum_size: Size,
}

impl PopoverGeometry {
    pub const MARGIN: f32 = 8.0;
    pub const GAP: f32 = 4.0;
    pub const INSET: f32 = 10.0;

    pub fn new(
        anchor: Rect,
        proposed: Size,
        window: Rect,
        placement: PopoverPlacement,
        rtl: bool,
        overflow: PopoverOverflow,
        margin: f32,
        gap: f32,
    ) -> Self {
        let bounds = Rect::new(
            window.x + margin.min(window.width / 2.0),
            window.y + margin.min(window.height / 2.0),
            (window.width - 2.0 * margin.min(window.width / 2.0)).max(0.0),
            (window.height - 2.0 * margin.min(window.height / 2.0)).max(0.0),
        );
        let mut size = Size {
            width: proposed.width.min(bounds.width.max(0.0)),
            height: proposed.height.min(bounds.height.max(0.0)),
        };
        let mut edge = placement.edge();
        if rtl && !edge.is_vertical() {
            edge = edge.opposite();
        }
        let space = |edge: PopoverEdge| -> f32 {
            match edge {
                PopoverEdge::Top => anchor.y - bounds.y - gap,
                PopoverEdge::Bottom => bounds.y + bounds.height - (anchor.y + anchor.height) - gap,
                PopoverEdge::Left => anchor.x - bounds.x - gap,
                PopoverEdge::Right => bounds.x + bounds.width - (anchor.x + anchor.width) - gap,
            }
        };
        let required = if edge.is_vertical() {
            size.height
        } else {
            size.width
        };
        if overflow == PopoverOverflow::Flip
            && space(edge) < required
            && space(edge.opposite()) >= required
        {
            edge = edge.opposite();
        }
        let maximum_size = Size {
            width: if !edge.is_vertical() && overflow == PopoverOverflow::Constrain {
                space(edge).max(0.0)
            } else {
                bounds.width.max(0.0)
            },
            height: if edge.is_vertical() && overflow == PopoverOverflow::Constrain {
                space(edge).max(0.0)
            } else {
                bounds.height.max(0.0)
            },
        };
        size.width = size.width.min(maximum_size.width);
        size.height = size.height.min(maximum_size.height);
        let mut alignment = placement.alignment();
        if edge.is_vertical() && rtl {
            alignment = -alignment;
        }
        let aligned = |minimum: f32, maximum: f32, extent: f32| -> f32 {
            if alignment < 0 {
                minimum
            } else if alignment > 0 {
                maximum - extent
            } else {
                (minimum + maximum - extent) / 2.0
            }
        };
        let (x, y) = if edge.is_vertical() {
            (
                aligned(anchor.x, anchor.x + anchor.width, size.width),
                if edge == PopoverEdge::Top {
                    anchor.y - gap - size.height
                } else {
                    anchor.y + anchor.height + gap
                },
            )
        } else {
            (
                if edge == PopoverEdge::Left {
                    anchor.x - gap - size.width
                } else {
                    anchor.x + anchor.width + gap
                },
                aligned(anchor.y, anchor.y + anchor.height, size.height),
            )
        };
        let frame = Rect::new(
            bounds.x.max(x.min(bounds.x + bounds.width - size.width)),
            bounds.y.max(y.min(bounds.y + bounds.height - size.height)),
            size.width,
            size.height,
        );
        Self {
            frame,
            edge,
            maximum_size,
        }
    }
}
