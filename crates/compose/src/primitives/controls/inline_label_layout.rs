//! Translation of Primitives/Controls/InlineLabelLayout.swift.
use crate::{Layout, LayoutSubview, ProposedViewSize};
use cove_core::scene::{Direction, Point, Rect, Size};
use std::rc::Rc;

/// Three retained slots: leading glyph, flexible title, trailing controls.
/// Only the title yields or takes width: in a wider frame it absorbs the
/// slack, so trailing controls (a chevron, a close button) sit at the edge. The measured chrome minimum belongs to the
/// control, so an enclosing maximum can never shrink its chrome.
#[derive(Clone)]
pub struct InlineLabelLayout {
    pub spacing: f32,
    pub on_minimum_width: Rc<dyn Fn(f32)>,
}

impl InlineLabelLayout {
    pub fn new(spacing: f32, on_minimum_width: impl Fn(f32) + 'static) -> Self {
        Self {
            spacing,
            on_minimum_width: Rc::new(on_minimum_width),
        }
    }

    fn arrangement(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Plan {
        if subviews.len() != 3 {
            return Plan {
                slots: Vec::new(),
                minimum_width: 0.0,
                size: Size::default(),
            };
        }
        let leading = subviews[0].size_that_fits(ProposedViewSize::UNSPECIFIED);
        let trailing = subviews[2].size_that_fits(ProposedViewSize::UNSPECIFIED);
        let minimum = leading.width
            + trailing.width
            + if leading.width > 0.0 {
                self.spacing
            } else {
                0.0
            }
            + if trailing.width > 0.0 {
                self.spacing
            } else {
                0.0
            };
        let title = subviews[1].size_that_fits(ProposedViewSize::new(
            proposal.width.map(|width| (width - minimum).max(0.0)),
            None,
        ));
        Plan {
            slots: vec![leading, title, trailing],
            minimum_width: minimum,
            size: Size {
                width: minimum + title.width,
                height: leading.height.max(title.height).max(trailing.height),
            },
        }
    }
}

impl Layout for InlineLabelLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        self.arrangement(proposal, subviews).size
    }

    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        if subviews.len() != 3 {
            return;
        }
        let plan = self.arrangement(ProposedViewSize::new(Some(bounds.width), None), subviews);
        let rtl = subviews
            .first()
            .map(|subview| subview.layout_direction == Direction::Rtl)
            .unwrap_or(false);
        let mut cursor = if rtl { bounds.max_x() } else { bounds.x };
        for index in 0..subviews.len() {
            let mut size = plan.slots[index];
            if index == 1 {
                size.width = size.width.max(bounds.width - plan.minimum_width);
            }
            let x = if rtl { cursor - size.width } else { cursor };
            subviews[index].place(
                Point::new(x, bounds.y + (bounds.height - size.height) / 2.0),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
            let gap = if index == 0 {
                if size.width > 0.0 { self.spacing } else { 0.0 }
            } else if index == 1 && plan.slots[2].width > 0.0 {
                self.spacing
            } else {
                0.0
            };
            cursor += if rtl { -1.0 } else { 1.0 } * (size.width + gap);
        }
        (self.on_minimum_width)(plan.minimum_width);
    }
}

struct Plan {
    slots: Vec<Size>,
    minimum_width: f32,
    size: Size,
}
