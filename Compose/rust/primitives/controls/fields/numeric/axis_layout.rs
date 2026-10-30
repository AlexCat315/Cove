//! Translation of Primitives/Controls/Fields/Numeric/AxisLayout.swift.
//!
//! Equal-width axis fields in one row while each keeps its minimum width;
//! otherwise every axis gets a full-width row. Never splits two-and-one, which
//! reads as a broken grid. Proposals are at-most, so axes fill them with a
//! 100% width.
use cove_core::{Point, Rect, Size};
use crate::{Layout, LayoutSubview, ProposedViewSize};

/// Equal-width axis fields while they fit; stacked rows otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisLayout {
    pub spacing: f32,
    pub minimum_width: f32,
    /// Per-axis width when the container proposes none.
    pub ideal_width: f32,
}

impl AxisLayout {
    pub fn new(spacing: f32, minimum_width: f32, ideal_width: f32) -> Self {
        Self {
            spacing: if spacing.is_finite() { spacing.max(0.0) } else { 4.0 },
            minimum_width: if minimum_width.is_finite() { minimum_width.max(0.0) } else { 0.0 },
            ideal_width: if ideal_width.is_finite() { ideal_width.max(0.0) } else { 0.0 },
        }
    }

    /// The frames for `count` axes within `width`, plus the resolved size.
    pub fn arrangement(&self, width: Option<f32>, subviews: &[LayoutSubview]) -> (Vec<Rect>, Size) {
        if subviews.is_empty() {
            return (Vec::new(), Size::default());
        }
        let count = subviews.len() as f32;
        let gaps = self.spacing * (count - 1.0);
        let available = width.unwrap_or(self.ideal_width * count + gaps);

        if available >= self.minimum_width * count + gaps {
            let column = (available - gaps) / count;
            let heights: Vec<f32> = subviews
                .iter()
                .map(|subview| subview.size_that_fits(ProposedViewSize::new(Some(column), None)).height)
                .collect();
            let height = heights.iter().copied().fold(0.0_f32, f32::max);
            let frames = subviews
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    Rect::new(index as f32 * (column + self.spacing), 0.0, column, height)
                })
                .collect();
            return (frames, Size { width: available, height });
        }

        let mut y = 0.0;
        let frames = subviews
            .iter()
            .map(|subview| {
                let height = subview.size_that_fits(ProposedViewSize::new(Some(available), None)).height;
                let frame = Rect::new(0.0, y, available, height);
                y += height + self.spacing;
                frame
            })
            .collect();
        (frames, Size { width: available, height: (y - self.spacing).max(0.0) })
    }
}

impl Layout for AxisLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.arrangement(proposal.width, subviews).1
    }

    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        let (frames, _) = self.arrangement(Some(bounds.width), subviews);
        for (index, subview) in subviews.iter().enumerate() {
            let Some(frame) = frames.get(index) else { continue };
            subview.place(
                Point::new(bounds.x + frame.x, bounds.y + frame.y),
                ProposedViewSize::new(Some(frame.width), Some(frame.height)),
                true,
            );
        }
    }
}
