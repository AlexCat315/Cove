//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/HorizontalScrollOrigin.swift.
use cove_core::scene::{Direction, Point, Rect, Size};
use crate::{Layout, LayoutSubview, ProposedViewSize};

/// Anchors its single child at the leading scroll origin, growing to the
/// proposal's width so horizontal overflow can be revealed.
#[derive(Clone, Copy, Debug, Default)]
pub struct HorizontalScrollOrigin;
impl Layout for HorizontalScrollOrigin {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        let Some(child) = subviews.first() else { return Size::default() };
        let size = child.size_that_fits(ProposedViewSize::new(proposal.width, None));
        Size { width: proposal.width.unwrap_or(size.width), height: size.height }
    }
    fn place_subviews(&self, bounds: Rect, _proposal: ProposedViewSize, subviews: &[LayoutSubview]) {
        let Some(child) = subviews.first() else { return };
        let size = child.size_that_fits(ProposedViewSize::new(Some(bounds.width), None));
        let inset = if child.layout_direction == Direction::Rtl { (bounds.width - size.width).max(0.0) } else { 0.0 };
        child.place(Point::new(bounds.x + inset, bounds.y), ProposedViewSize::new(Some(size.width), Some(size.height)), true);
    }
}
