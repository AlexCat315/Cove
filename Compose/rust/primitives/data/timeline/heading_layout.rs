//! Translation of `Primitives/Data/Timeline/HeadingLayout.swift`.
//!
//! Repositions the same title and metadata nodes using their actual allocation.
//! Neither text is truncated or scaled down to make the heading fit.

use cove_core::{Point, Rect, Size};
use crate::{Layout, LayoutSubview, ProposedViewSize};

/// Lays a title beside its trailing metadata, dropping the metadata onto its
/// own line when both will not fit.
pub struct TimelineHeadingLayout {
    pub spacing: f32,
}

impl TimelineHeadingLayout {
    pub fn new(spacing: f32) -> Self {
        Self { spacing: if spacing.is_finite() { spacing.max(0.0) } else { 0.0 } }
    }

    fn plan(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Plan {
        if subviews.is_empty() {
            return Plan { slots: Vec::new(), size: Size::default(), stacked: false };
        }
        let natural: Vec<Size> = subviews
            .iter()
            .map(|subview| subview.size_that_fits(ProposedViewSize::UNSPECIFIED))
            .collect();
        let gap = if subviews.len() > 1 { self.spacing } else { 0.0 };
        let natural_total: f32 = natural.iter().map(|size| size.width).sum::<f32>() + gap;
        let width = proposal.width.unwrap_or(natural_total).max(0.0);
        let stacked = subviews.len() > 1 && (width < 600.0 || natural_total > width);
        if stacked {
            let slots: Vec<Size> = subviews
                .iter()
                .map(|subview| subview.size_that_fits(ProposedViewSize::new(Some(width), None)))
                .collect();
            let height: f32 = slots.iter().map(|size| size.height).sum::<f32>() + 4.0;
            return Plan { slots, size: Size { width, height }, stacked: true };
        }
        let trailing_width: f32 = natural.iter().skip(1).map(|size| size.width).sum();
        let title = subviews[0].size_that_fits(ProposedViewSize::new(
            Some((width - trailing_width - gap).max(0.0)),
            None,
        ));
        let mut slots = vec![title];
        slots.extend(natural.iter().skip(1).copied());
        let height = slots.iter().map(|size| size.height).fold(0.0_f32, f32::max);
        Plan { slots, size: Size { width, height }, stacked: false }
    }
}

impl Layout for TimelineHeadingLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.plan(proposal, subviews).size
    }

    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        let plan = self.plan(ProposedViewSize::new(Some(bounds.width), None), subviews);
        let rtl = subviews
            .first()
            .map(|subview| subview.layout_direction == cove_core::scene::Direction::Rtl)
            .unwrap_or(false);
        for (index, subview) in subviews.iter().enumerate() {
            let Some(size) = plan.slots.get(index).copied() else { continue };
            let (x, y) = if plan.stacked {
                let x = if rtl { bounds.x + bounds.width - size.width } else { bounds.x };
                let y = bounds.y
                    + if index == 0 {
                        0.0
                    } else {
                        plan.slots.first().map(|slot| slot.height).unwrap_or(0.0) + 4.0
                    };
                (x, y)
            } else {
                let trailing = index > 0;
                let x = if trailing != rtl {
                    bounds.x + bounds.width - size.width
                } else {
                    bounds.x
                };
                let y = bounds.y + (bounds.height - size.height) / 2.0;
                (x, y)
            };
            subview.place(
                Point::new(x, y),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
        }
    }
}

struct Plan {
    slots: Vec<Size>,
    size: Size,
    stacked: bool,
}
