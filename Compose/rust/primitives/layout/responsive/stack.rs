//! Translation of Primitives/Layout/Responsive/Stack.swift.
//!
//! A natural-size stack algorithm for custom and candidate layouts. Children
//! receive the offered cross-axis size and an intrinsic main axis; use the Box
//! containers when flex growth or compression is required.
use cove_core::{Point, Rect, Size};
use crate::{Alignment, HorizontalAlignment, Layout, LayoutSubview, ProposedViewSize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StackAxis {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackLayout {
    pub axis: StackAxis,
    pub spacing: f32,
    pub alignment: Alignment,
}

impl Default for StackLayout {
    fn default() -> Self {
        Self { axis: StackAxis::Vertical, spacing: 0.0, alignment: Alignment::TopLeading }
    }
}

impl StackLayout {
    pub fn new(axis: StackAxis, spacing: f32, alignment: Alignment) -> Self {
        Self { axis, spacing, alignment }
    }

    fn resolved_spacing(&self) -> f32 {
        if self.spacing.is_finite() { self.spacing.max(0.0) } else { 0.0 }
    }

    fn proposal_for_child(&self, proposal: ProposedViewSize) -> ProposedViewSize {
        match self.axis {
            StackAxis::Horizontal => ProposedViewSize::new(None, proposal.height),
            StackAxis::Vertical => ProposedViewSize::new(proposal.width, None),
        }
    }

    fn arrangement(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> (Vec<Size>, Size) {
        let child_proposal = self.proposal_for_child(proposal);
        let sizes: Vec<Size> = subviews.iter().map(|subview| subview.size_that_fits(child_proposal)).collect();
        let mut main = self.resolved_spacing() * sizes.len().saturating_sub(1) as f32;
        let mut cross: f32 = 0.0;
        for size in &sizes {
            match self.axis {
                StackAxis::Horizontal => {
                    main += size.width;
                    cross = cross.max(size.height);
                }
                StackAxis::Vertical => {
                    main += size.height;
                    cross = cross.max(size.width);
                }
            }
        }
        let total = match self.axis {
            StackAxis::Horizontal => Size { width: main, height: cross },
            StackAxis::Vertical => Size { width: cross, height: main },
        };
        (sizes, total)
    }

    fn horizontal_offset(&self, available: f32, rtl: bool) -> f32 {
        let (horizontal, _) = self.alignment.components();
        match horizontal {
            HorizontalAlignment::Leading => if rtl { available } else { 0.0 },
            HorizontalAlignment::Center => available / 2.0,
            HorizontalAlignment::Trailing => if rtl { 0.0 } else { available },
        }
    }

    fn vertical_offset(&self, available: f32) -> f32 {
        let (_, vertical) = self.alignment.components();
        match vertical {
            crate::VerticalAlignment::Top => 0.0,
            crate::VerticalAlignment::Center => available / 2.0,
            crate::VerticalAlignment::Bottom => available,
        }
    }
}

impl Layout for StackLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.arrangement(proposal, subviews).1
    }

    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        let offered = ProposedViewSize::new(Some(bounds.width), Some(bounds.height));
        let (sizes, total) = self.arrangement(offered, subviews);
        let child_proposal = self.proposal_for_child(offered);
        let rtl = subviews.first().is_some_and(|subview| subview.layout_direction == cove_core::scene::Direction::Rtl);
        let start_x = bounds.x + self.horizontal_offset(bounds.width - total.width, rtl);
        let start_y = bounds.y + self.vertical_offset(bounds.height - total.height);
        let mut cursor = 0.0_f32;
        for (index, subview) in subviews.iter().enumerate() {
            let size = sizes[index];
            let point = match self.axis {
                StackAxis::Horizontal => {
                    let x = if rtl {
                        start_x + total.width - cursor - size.width
                    } else {
                        start_x + cursor
                    };
                    let y = bounds.y + self.vertical_offset(bounds.height - size.height);
                    cursor += size.width + self.resolved_spacing();
                    Point::new(x, y)
                }
                StackAxis::Vertical => {
                    let x = bounds.x + self.horizontal_offset(bounds.width - size.width, rtl);
                    cursor += size.height + self.resolved_spacing();
                    Point::new(x, start_y + cursor - size.height - self.resolved_spacing())
                }
            };
            // A measured size can be rounded by the layout engine. Treating that
            // result as a new, tighter constraint can wrap the final character
            // and change the child's height during placement.
            subview.place(point, child_proposal, true);
        }
    }
}
