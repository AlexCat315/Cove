//! Responsive layout for action controls that stack when their natural widths do not fit.
use cove_core::scene::Direction;
use cove_core::{Point, Rect, Size};
use crate::{HorizontalAlignment, Layout, LayoutSubview, ProposedViewSize};

pub struct ActionGroupLayout {
    pub stacks: bool,
    pub alignment: HorizontalAlignment,
    pub spacing: f32,
}

impl ActionGroupLayout {
    pub fn new(stacks: bool) -> Self {
        Self { stacks, alignment: HorizontalAlignment::Trailing, spacing: 8.0 }
    }

    fn vertical(&self, width: Option<f32>, subviews: &[LayoutSubview]) -> bool {
        self.stacks || width.is_some_and(|available| {
            let natural: f32 = subviews.iter()
                .map(|subview| subview.size_that_fits(ProposedViewSize::UNSPECIFIED).width)
                .sum();
            natural + self.spacing * subviews.len().saturating_sub(1) as f32 > available
        })
    }

    fn horizontal_offset(&self, available: f32, used: f32, rtl: bool) -> f32 {
        let free = (available - used).max(0.0);
        match self.alignment {
            HorizontalAlignment::Leading => if rtl { free } else { 0.0 },
            HorizontalAlignment::Center => free / 2.0,
            HorizontalAlignment::Trailing => if rtl { 0.0 } else { free },
        }
    }
}

impl Layout for ActionGroupLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        if subviews.is_empty() { return Size::default(); }
        let column = self.vertical(proposal.width, subviews);
        let proposal = if column { ProposedViewSize::new(proposal.width, None) } else { ProposedViewSize::UNSPECIFIED };
        let sizes: Vec<_> = subviews.iter().map(|subview| subview.size_that_fits(proposal)).collect();
        let gaps = self.spacing * sizes.len().saturating_sub(1) as f32;
        let width = if column {
            sizes.iter().map(|size| size.width).fold(0.0_f32, f32::max)
        } else {
            sizes.iter().map(|size| size.width).sum::<f32>() + gaps
        };
        let height = if column {
            sizes.iter().map(|size| size.height).sum::<f32>() + gaps
        } else {
            sizes.iter().map(|size| size.height).fold(0.0_f32, f32::max)
        };
        Size { width: proposal.width.unwrap_or(width), height }
    }

    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        let column = self.vertical(Some(bounds.width), subviews);
        let child_proposal = if column {
            ProposedViewSize::new(Some(bounds.width), None)
        } else {
            ProposedViewSize::UNSPECIFIED
        };
        let sizes: Vec<_> = subviews.iter().map(|subview| subview.size_that_fits(child_proposal)).collect();
        let gaps = self.spacing * sizes.len().saturating_sub(1) as f32;
        let used = sizes.iter().map(|size| size.width).sum::<f32>() + gaps;
        let rtl = subviews.first().is_some_and(|subview| subview.layout_direction == Direction::Rtl);
        let start = self.horizontal_offset(bounds.width, used, rtl);
        let mut offset = 0.0_f32;
        for (subview, size) in subviews.iter().zip(sizes) {
            let x = if column {
                self.horizontal_offset(bounds.width, size.width, rtl)
            } else if rtl {
                start + used - offset - size.width
            } else {
                start + offset
            };
            subview.place(
                Point::new(bounds.x + x, bounds.y + if column { offset } else { (bounds.height - size.height) / 2.0 }),
                ProposedViewSize::new(if column { Some(bounds.width) } else { None }, Some(size.height)),
                true,
            );
            let advance = if column { size.height } else { size.width };
            offset += advance + self.spacing;
        }
    }
}
