//! Translation of Compose/Sources/CoveUICompose/Primitives/Layout/ZStack.swift.
use crate::{
    Alignment, AnyView, HorizontalAlignment, Layout, LayoutContainer, LayoutSubview,
    ProposedViewSize, VerticalAlignment, View,
};
use cove_core::scene::{Direction, Point, Rect, Size};

/// Layered container. Children are measured against the available proposal and
/// placed on top of each other in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZStackLayout {
    pub alignment: Alignment,
}
impl ZStackLayout {
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
impl Default for ZStackLayout {
    fn default() -> Self {
        Self::new(Alignment::Center)
    }
}
impl Layout for ZStackLayout {
    fn size_that_fits(&self, proposal: ProposedViewSize, subviews: &[LayoutSubview]) -> Size {
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for subview in subviews {
            let size = subview.size_that_fits(proposal);
            width = width.max(size.width);
            height = height.max(size.height);
        }
        Size { width, height }
    }
    fn place_subviews(
        &self,
        bounds: Rect,
        _proposal: ProposedViewSize,
        subviews: &[LayoutSubview],
    ) {
        let child_proposal = ProposedViewSize::new(Some(bounds.width), Some(bounds.height));
        let (horizontal, vertical) = self.alignment.components();
        for subview in subviews {
            let size = subview.size_that_fits(child_proposal);
            let rtl = subview.layout_direction == Direction::Rtl;
            let x = bounds.x + Self::horizontal_offset(horizontal, bounds.width - size.width, rtl);
            let y = bounds.y + Self::vertical_offset(vertical, bounds.height - size.height);
            subview.place(
                Point::new(x, y),
                ProposedViewSize::new(Some(size.width), Some(size.height)),
                true,
            );
        }
    }
}

/// Layer children in declaration order. The last child is painted and hit
/// first; earlier children remain mounted and keep their state.
#[derive(Clone)]
pub struct ZStack {
    pub alignment: Alignment,
    pub content: AnyView,
}
impl ZStack {
    pub fn new(alignment: Alignment, content: AnyView) -> Self {
        Self { alignment, content }
    }
}
impl View for ZStack {
    fn body(&self) -> AnyView {
        AnyView::new(
            LayoutContainer::<ZStackLayout, AnyView>::with_erased_content(
                ZStackLayout::new(self.alignment),
                self.content.clone(),
            ),
        )
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
