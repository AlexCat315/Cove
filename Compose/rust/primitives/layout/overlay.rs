//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/Overlay.swift.
use crate::{
    Alignment, AnyView, HorizontalAlignment, Layout, LayoutContainer, LayoutSubview,
    ProposedViewSize, VerticalAlignment, View,
};
use cove_core::scene::{Direction, Point, Rect, Size};

/// Places overlay content on top of the base view without changing the base's
/// measured size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayLayout {
    pub alignment: Alignment,
}
impl OverlayLayout {
    pub fn new(alignment: Alignment) -> Self {
        Self { alignment }
    }
    fn horizontal_offset(alignment: HorizontalAlignment, available: f32, rtl: bool) -> f32 {
        match alignment {
            HorizontalAlignment::Leading => {
                if rtl {
                    available
                } else {
                    0.0
                }
            }
            HorizontalAlignment::Center => available / 2.0,
            HorizontalAlignment::Trailing => {
                if rtl {
                    0.0
                } else {
                    available
                }
            }
        }
    }
    fn vertical_offset(alignment: VerticalAlignment, available: f32) -> f32 {
        match alignment {
            VerticalAlignment::Top => 0.0,
            VerticalAlignment::Center => available / 2.0,
            VerticalAlignment::Bottom => available,
        }
    }
}
impl Default for OverlayLayout {
    fn default() -> Self {
        Self::new(Alignment::Center)
    }
}
impl Layout for OverlayLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        subviews
            .first()
            .map(|child| child.size_that_fits(proposal))
            .unwrap_or_default()
    }
    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let Some(base) = subviews.first() else { return };
        let bounds_proposal = ProposedViewSize::new(Some(bounds.width), Some(bounds.height));
        base.place(Point::new(bounds.x, bounds.y), bounds_proposal, true);
        let Some(overlay) = subviews.get(1) else {
            return;
        };
        let size = overlay.size_that_fits(bounds_proposal);
        let (horizontal, vertical) = self.alignment.components();
        let rtl = overlay.layout_direction == Direction::Rtl;
        let x = bounds.x + Self::horizontal_offset(horizontal, bounds.width - size.width, rtl);
        let y = bounds.y + Self::vertical_offset(vertical, bounds.height - size.height);
        overlay.place(
            Point::new(x, y),
            ProposedViewSize::new(Some(size.width), Some(size.height)),
            true,
        );
    }
}

/// Base view plus one overlay child. The base owns the measured size.
#[derive(Clone)]
pub struct Overlay {
    pub base: AnyView,
    pub overlay: AnyView,
    pub alignment: Alignment,
}
impl Overlay {
    pub fn new<Base: View + 'static, Content: View + 'static>(
        base: Base,
        overlay: Content,
        alignment: Alignment,
    ) -> Self {
        Self {
            base: AnyView::new(base),
            overlay: AnyView::new(overlay),
            alignment,
        }
    }
    pub fn with_erased_content(base: AnyView, overlay: AnyView, alignment: Alignment) -> Self {
        Self {
            base,
            overlay,
            alignment,
        }
    }
}
impl View for Overlay {
    fn body(&self) -> AnyView {
        let pair = crate::TupleView::new(vec![self.base.clone(), self.overlay.clone()]);
        AnyView::new(
            LayoutContainer::<OverlayLayout, AnyView>::with_erased_content(
                OverlayLayout::new(self.alignment),
                AnyView::new(pair),
            ),
        )
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Attach content to the bounds of this view without changing its measured size.
pub trait OverlayViewExt: View + Sized + 'static {
    fn overlay<Content: View + 'static>(self, alignment: Alignment, content: Content) -> Overlay {
        Overlay::new(self, content, alignment)
    }
}
impl<T: View + Sized + 'static> OverlayViewExt for T {}
