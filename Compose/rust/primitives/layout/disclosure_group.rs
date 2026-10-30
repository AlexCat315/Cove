//! Translation of Primitives/Layout/DisclosureGroup.swift.
//!
//! A keyboard-focusable disclosure header with naturally sized content.
//! Expansion belongs to the caller, so it survives panel reconstruction and can
//! also be controlled by search or an Expand All command.
use cove_core::scene::{
    AccessibilityRole, AccessibilityState, Align, Edge, FlexDirection, Justify, LayoutNode, Node,
    Wrap,
};
use cove_core::Binding;

use crate::animation::content::AnimatedVisibility;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::media::ui_icon_catalog::navigation;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View};

/// A disclosure header with content that only participates in layout when open.
pub struct DisclosureGroup {
    pub is_expanded: Binding<bool>,
    pub is_enabled: bool,
    label: AnyView,
    content: AnyView,
}

impl DisclosureGroup {
    pub fn new(
        is_expanded: Binding<bool>,
        label: impl View + 'static,
        content: impl View + 'static,
    ) -> Self {
        Self {
            is_expanded,
            is_enabled: true,
            label: AnyView::new(label),
            content: AnyView::new(content),
        }
    }

    /// Title-only header: the label truncates rather than wrapping the row.
    pub fn title(
        is_expanded: Binding<bool>,
        title: impl Into<String>,
        content: impl View + 'static,
    ) -> Self {
        let label = AnyView::new(
            Text::new(title)
                .truncated()
                .semantic_font(SemanticFontRef::label())
                .semantic_foreground(SemanticColorRef::on_surface_variant())
                .frame_min_width(0.0),
        );
        Self::new(is_expanded, label, content)
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn label(mut self, label: impl View + 'static) -> Self {
        self.label = AnyView::new(label);
        self
    }

    pub fn content(mut self, content: impl View + 'static) -> Self {
        self.content = AnyView::new(content);
        self
    }
}

impl View for DisclosureGroup {
    fn body(&self) -> AnyView {
        let is_expanded = self.is_expanded.get();
        let toggle_binding = self.is_expanded.clone();
        let is_enabled = self.is_enabled;

        let chevron = AnyView::new(
            ButtonIcon::new(
                ButtonIconSource::resource(if is_expanded {
                    navigation::chevron_down()
                } else {
                    navigation::chevron_right()
                }),
                Some(12.0),
                None,
            )
            .accessibility_hidden(true),
        );
        let header_row = AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                5.0,
                AnyView::new(TupleView::new(vec![
                    chevron,
                    AnyView::new(
                        AnyView::new(self.label.clone())
                            .flex_with(1.0, 1.0, None)
                            .frame_min_width(0.0),
                    ),
                ])),
            )
            .padding_edges(2.0, 0.0)
            .frame_min_width(0.0)
            .flex_with(1.0, 1.0, None),
        );
        let header = AnyView::new(
            Button::with_options(
                header_row,
                move || toggle_binding.set(!toggle_binding.get()),
                ButtonRole::Normal,
                is_enabled,
                false,
                false,
                None,
            )
            .button_style(GhostButtonStyle)
            .accessibility(move |semantics| {
                semantics.role = AccessibilityRole::Button;
                semantics.state = AccessibilityState {
                    is_enabled,
                    is_expanded: Some(is_expanded),
                    ..Default::default()
                };
            })
            .flex_with(1.0, 1.0, None),
        );

        let body = AnyView::new(AnimatedVisibility::collapsing(
            is_expanded,
            true,
            None,
            DisclosureContent { content: self.content.clone() },
        ));

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![header, body])),
            )
            .frame_min_width(0.0),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Applies the content's own leading gutter without an extra composition layer.
pub struct DisclosureContent {
    pub content: AnyView,
}

impl PrimitiveView for DisclosureContent {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_padding(6.0, Edge::Top);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_padding(6.0, Edge::Top);
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(DisclosureContent);
