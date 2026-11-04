//! Shared, responsive building blocks for interactive control examples.
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{SemanticColorRef, SemanticFontRef};
use cove_ui_compose::{AnyView, BoxView, TupleView, View};

pub(super) fn stack(items: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        12.0,
        AnyView::new(TupleView::new(items)),
    ))
}

pub(super) fn row(items: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::FlexStart,
        Wrap::Wrap,
        12.0,
        AnyView::new(TupleView::new(items)),
    ))
}

pub(super) fn bounded(content: impl View + 'static, maximum: f32) -> AnyView {
    AnyView::new(AnyView::new(content).frame(None, None, Some(0.0), None, Some(maximum), None))
}

pub(super) fn note(text: impl Into<String>) -> AnyView {
    AnyView::new(
        Text::new(text.into())
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted()),
    )
}
