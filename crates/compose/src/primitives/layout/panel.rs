//! Translation of Primitives/Layout/Panel.swift.
//!
//! Surface container with a labelled header bar. Visual chrome is delegated to
//! the active `PanelStyle`; call sites only supply title, optional accessory and
//! content.
use crate::Scope;
use crate::modifiers::layout::FlexExt;
use crate::theme::styles::components::{PanelStyleConfiguration, PanelStyleEnvironment};
use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{Align, FlexDirection, LayoutNode, Node};

/// Surface container with a labelled header bar.
pub struct Panel {
    pub title: String,
    pub is_active: bool,
    accessory: AnyView,
    content: AnyView,
}

impl Panel {
    pub fn new(title: impl Into<String>, content: impl View + 'static) -> Self {
        Self {
            title: title.into(),
            is_active: false,
            accessory: AnyView::new(crate::EmptyView),
            content: AnyView::new(content),
        }
    }

    pub fn accessory(mut self, accessory: impl View + 'static) -> Self {
        self.accessory = AnyView::new(accessory);
        self
    }

    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }
}

impl View for Panel {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(PanelHost {
            title: self.title.clone(),
            is_active: self.is_active,
            accessory: self.accessory.clone(),
            content: self.content.clone(),
        })
    }
}

struct PanelHost {
    title: String,
    is_active: bool,
    accessory: AnyView,
    content: AnyView,
}

impl PrimitiveView for PanelHost {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn update_node(&self, _node: &Node) {}

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let style = node.composition_value(&PanelStyleEnvironment::key());
        let configuration = PanelStyleConfiguration {
            title: self.title.clone(),
            accessory: self.accessory.clone(),
            content: self.content.clone(),
            is_active: self.is_active,
            theme: theme_for_node(node),
        };
        vec![AnyView::new(style.make_body(configuration).flex(1.0))]
    }
}
impl_primitive_view!(PanelHost);
