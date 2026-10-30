//! Translation of Primitives/Layout/Sidebar/Rail.swift.
//!
//! Narrow rail presentation: one glyph per destination, optional labels, and no
//! trailing accessories. The rail keeps the same destination nodes as the
//! expanded sidebar, so switching presentation preserves selection and drafts.
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::text::Text;
use crate::theme::{theme_for_node, SemanticFontRef};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};

use super::items::SidebarItem;

/// Vertical or horizontal rail container for destination glyphs.
pub struct SidebarRail {
    pub items: Vec<SidebarItem<u64>>,
    pub shows_labels: bool,
    pub width: Option<f32>,
    pub is_vertical: bool,
    pub on_activate: Option<std::rc::Rc<dyn Fn(u64)>>,
}

impl SidebarRail {
    pub fn new(items: Vec<SidebarItem<u64>>) -> Self {
        Self { items, shows_labels: false, width: None, is_vertical: true, on_activate: None }
    }

    pub fn shows_labels(mut self, shows_labels: bool) -> Self {
        self.shows_labels = shows_labels;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.is_vertical = false;
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_activate = Some(std::rc::Rc::new(handler));
        self
    }
}

/// Applies the rail's own gutter so the destinations do not touch the edge.
struct RailPadded {
    content: AnyView,
}

impl PrimitiveView for RailPadded {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_padding(4.0, cove_core::scene::Edge::All);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_padding(4.0, cove_core::scene::Edge::All);
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(RailPadded);

impl View for SidebarRail {
    fn body(&self) -> AnyView {
        let mut children: Vec<AnyView> = Vec::new();
        for item in &self.items {
            let enabled = item.is_enabled;
            let label = item.title.clone();
            let icon = item.icon.clone();
            let shows_labels = self.shows_labels;
            let on_activate = self.on_activate.clone();
            let id = item.id;
            children.push(AnyView::new(RailDestination {
                id,
                label,
                icon,
                is_enabled: enabled,
                shows_labels,
                on_activate,
            }));
        }
        let content = AnyView::new(
            BoxView::new(
                if self.is_vertical { FlexDirection::Column } else { FlexDirection::Row },
                if self.is_vertical { Align::Center } else { Align::Center },
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(children)),
            ),
        );
        let content = match self.width {
            Some(width) => AnyView::new(content.frame_width(width)),
            None => content,
        };
        AnyView::new(RailPadded { content })
    }
}

/// One rail destination: glyph plus optional label.
pub struct RailDestination {
    pub id: u64,
    pub label: String,
    pub icon: Option<cove_imaging::BundleImageResource>,
    pub is_enabled: bool,
    pub shows_labels: bool,
    pub on_activate: Option<std::rc::Rc<dyn Fn(u64)>>,
}

impl PrimitiveView for RailDestination {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let tint = if self.is_enabled {
            theme.colors.on_surface
        } else {
            theme.colors.on_surface_muted
        };
        let glyph = match self.icon.clone() {
            Some(icon) => AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(icon),
                Some(20.0),
                Some(tint),
            )),
            None => AnyView::new(crate::EmptyView),
        };
        let mut column: Vec<AnyView> = vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row, Align::Center, Justify::Center, Wrap::NoWrap, 0.0, glyph,
            )
            .frame_size(24.0, 24.0),
        )];
        if self.shows_labels {
            column.push(AnyView::new(
                Text::new(self.label.clone())
                    .truncated()
                    .semantic_font(SemanticFontRef::caption())
                    .foreground_color(tint),
            ));
        }
        let _ = self.on_activate.as_ref().map(|handler| handler(self.id));
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Center, Justify::Center, Wrap::NoWrap, 2.0,
                AnyView::new(TupleView::new(column)),
            )
            .frame(None, None, Some(44.0), Some(44.0), None, None)
            .flex_with(0.0, 0.0, None),
        )]
    }
}
crate::impl_primitive_view!(RailDestination);
